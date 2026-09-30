//! Robust in-page activation of interactive controls.
//!
//! agent-browser's coordinate click (`find role … click`, `find text … click`,
//! `click <selector>`) dispatches at the element's screen point. Two failure
//! modes bite real apps constantly:
//!
//!   1. **Overlay interception** — the accessible name lives on a text/label
//!      node that sits *under* the real control (e.g. a "Select a license"
//!      button whose label is a separate `div`), so the coordinate click lands
//!      on the covering element and the app's handler never fires.
//!   2. **mousedown-bound handlers** — many custom select and menu components
//!      open on `mousedown`, not `click`, so a synthetic click is silently
//!      swallowed even when it lands on the right node.
//!
//! This module builds JS that resolves the target by ARIA role + accessible
//! name (or by visible text), scrolls it into view, and dispatches the full
//! `pointerdown → mousedown → pointerup → mouseup → click` chain **on the node
//! itself** — no coordinates, nothing to intercept. Submit buttons route
//! through `form.requestSubmit()` so the form's submit path runs.
//!
//! The builders return JS expression strings (unit-testable); the `activate_*`
//! wrappers eval them against a session. Shared by the recorder (`smart_click`)
//! and replay (`verbs`) so both paths click the same robust way.

use crate::browser;

/// ARIA roles we can activate as interactive controls. Anything outside this
/// set (e.g. `heading`, `text`, `img`) has no meaningful DOM activation, so
/// callers fall back to agent-browser's role finder for those.
pub fn is_interactive_role(role: &str) -> bool {
    matches!(
        role,
        "button"
            | "link"
            | "combobox"
            | "option"
            | "menuitem"
            | "menuitemcheckbox"
            | "menuitemradio"
            | "tab"
            | "checkbox"
            | "radio"
            | "switch"
            | "treeitem"
            | "listbox"
    )
}

/// Roles that, when activated, are expected to reveal a popup (listbox / menu /
/// option set). Used to decide whether to verify a popup opened and escalate to
/// the keyboard opener contract.
pub fn is_popup_opener_role(role: &str) -> bool {
    matches!(role, "combobox" | "listbox")
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// ARIA role → candidate-selector map, shared by the activation builder and
/// the auto-heal name collector. Single source of truth so both resolve the
/// same candidate set agent-browser's ARIA view would.
fn role_candidate_map_js() -> &'static str {
    r#"{
    button: ['button','input[type=button]','input[type=submit]','input[type=reset]','[role=button]'],
    link: ['a[href]','a','[role=link]'],
    combobox: ['[role=combobox]','select','[aria-haspopup]','[aria-expanded]','button'],
    listbox: ['[role=listbox]','select'],
    option: ['[role=option]','option'],
    menuitem: ['[role=menuitem]','[role=menuitemcheckbox]','[role=menuitemradio]'],
    menuitemcheckbox: ['[role=menuitemcheckbox]','[role=menuitem]'],
    menuitemradio: ['[role=menuitemradio]','[role=menuitem]'],
    tab: ['[role=tab]'],
    checkbox: ['[role=checkbox]','input[type=checkbox]'],
    radio: ['[role=radio]','input[type=radio]'],
    switch: ['[role=switch]','[role=checkbox]'],
    treeitem: ['[role=treeitem]'],
  }"#
}

/// Shared JS: the candidate-selector map + name/visibility helpers + the
/// activation chain. Emitted once and reused by the role and text builders.
/// Defines `__aqPick(el)` (activate a resolved element, returns bool) and
/// `__aqName(n)` / `__aqVisible(n)` helpers in the IIFE scope.
fn activation_prelude() -> &'static str {
    r#"
  const __aqText = (s) => (s || '').trim().toLowerCase();
  // Digit-normalize: collapse each run of digits to '#' so names that differ
  // only by a volatile count/id still compare equal.
  const __aqND = (s) => (s || '').replace(/\d+/g, '#').replace(/\s+/g, ' ').trim();
  // Accessible-name candidates. Crucially includes aria-labelledby (many
  // controls — e.g. a "Select a license" combobox button — carry only an icon
  // and take their name from a separate label element), so text/name matching
  // resolves the SAME element agent-browser's ARIA engine would, not just ones
  // whose own textContent happens to contain the label.
  const __aqName = (n) => {
    const out = [
      n.getAttribute && n.getAttribute('aria-label'),
      n.innerText,
      n.textContent,
      n.value,
      n.getAttribute && n.getAttribute('title'),
      n.getAttribute && n.getAttribute('name'),
      n.getAttribute && n.getAttribute('placeholder'),
    ];
    const lb = n.getAttribute && n.getAttribute('aria-labelledby');
    if (lb) {
      out.push(lb.split(/\s+/).map((id) => {
        const e = document.getElementById(id);
        return e ? (e.innerText || e.textContent || '') : '';
      }).join(' '));
    }
    return out;
  };
  const __aqVisible = (n) => {
    try { return n.getClientRects().length > 0; } catch (e) { return true; }
  };
  const __aqRealRoles = 'a[href],button,input,select,textarea,[role=button],[role=link],[role=combobox],[role=option],[role=menuitem],[role=menuitemcheckbox],[role=menuitemradio],[role=tab],[role=checkbox],[role=radio],[role=switch],[role=treeitem]';
  // A genuinely interactive control — a real interactive tag/role, or an
  // element with onclick / a focusable tabindex. Explicitly EXCLUDES
  // role=presentation / role=none (modal backdrops and layout wrappers often
  // carry onclick + tabindex=-1; clicking one dismisses the very dialog we are
  // trying to act inside).
  const __aqIsInteractive = (n) => {
    if (!n || !n.matches) return false;
    const role = (n.getAttribute('role') || '').toLowerCase();
    if (role === 'presentation' || role === 'none') return false;
    if (n.matches(__aqRealRoles)) return true;
    const ti = n.getAttribute('tabindex');
    if (n.hasAttribute('onclick')) return true;
    return ti !== null && parseInt(ti, 10) >= 0;
  };
  const __aqInteractiveSel = __aqRealRoles;
  // When a modal/menu/listbox is open, an interaction almost always targets
  // inside the topmost one — the same control name (e.g. "Add user") often
  // exists both on the page behind and inside the dialog. Prefer the open
  // surface so we don't click the wrong twin. Falls back to the whole document
  // when nothing in scope matches.
  const __aqScopeRoot = () => {
    const surfaces = Array.from(document.querySelectorAll('[role=dialog],[role=alertdialog],dialog[open],[role=listbox],[role=menu]')).filter(__aqVisible);
    return surfaces.length ? surfaces[surfaces.length - 1] : document;
  };
  const __aqPrefer = (arr, root) => {
    if (root === document) return arr;
    const scoped = arr.filter((n) => root.contains(n));
    return scoped.length ? scoped : arr;
  };
  const __aqPick = (el) => {
    if (!el) return false;
    try { el.scrollIntoView({ block: 'center', inline: 'center' }); } catch (e) {}
    // A real mouse click focuses the element; synthetic events don't.
    try { el.focus(); } catch (e) {}
    const type = (el.getAttribute && (el.getAttribute('type') || '') || '').toLowerCase();
    const isSubmit = (el.tagName === 'BUTTON' && type !== 'button' && type !== 'reset')
      || (el.tagName === 'INPUT' && type === 'submit');
    const form = el.form || (el.closest ? el.closest('form') : null);
    // Defer the activation chain past the eval's response window: a handler
    // can open a native dialog (alert/confirm/prompt/beforeunload), which
    // blocks the page's JS thread and stops the daemon delivering the eval
    // result (~30s internal timeout). A ~150ms timer lets the response land
    // first — the dialog then stays pending for the next `dialog` step.
    setTimeout(() => {
      try {
        if (isSubmit && form && typeof form.requestSubmit === 'function') {
          form.requestSubmit(el);
          return;
        }
        const opts = { bubbles: true, cancelable: true, view: window };
        try { el.dispatchEvent(new PointerEvent('pointerdown', opts)); } catch (e) {}
        el.dispatchEvent(new MouseEvent('mousedown', opts));
        try { el.dispatchEvent(new PointerEvent('pointerup', opts)); } catch (e) {}
        el.dispatchEvent(new MouseEvent('mouseup', opts));
        if (typeof el.click === 'function') { el.click(); } else { el.dispatchEvent(new MouseEvent('click', opts)); }
      } catch (e) {}
    }, 150);
    return true;
  };
"#
}

/// Build JS that resolves an element by ARIA `role` + accessible `name` among a
/// role-appropriate candidate set, then activates it. Exact name match wins over
/// substring; visible candidates win over hidden. Returns `"true"`/`"false"`.
///
/// `role` empty → search across every interactive role (name-only match).
pub fn build_role_name_click(role: &str, name: &str) -> String {
    format!(
        r#"(() => {{{prelude}
  const want = __aqText({name_lit});
  const role = {role_lit};
  const map = {map};
  const sels = (role && map[role]) ? map[role]
    : (role ? ['[role="' + role + '"]'] : Object.keys(map).reduce((a, k) => a.concat(map[k]), []));
  const cands = Array.from(document.querySelectorAll(sels.join(',')));
  const exact = (n) => __aqName(n).some((c) => __aqText(c) === want);
  const partial = (n) => want.length >= 3 && __aqName(n).some((c) => __aqText(c).includes(want));
  // Digit-tolerant: volatile counts drift between record and replay (e.g.
  // "Optimize 9986 licenses available" → "…9985…", "Row 41" → "Row 42").
  // Collapse digit runs to '#' so the stable text still matches.
  const wantND = __aqND(want);
  const digitTol = wantND.replace(/#/g, '').trim().length >= 3
    && ((n) => __aqName(n).some((c) => __aqND(__aqText(c)) === wantND));
  const root = __aqScopeRoot();
  const vis = cands.filter(__aqVisible);
  const el = __aqPrefer(vis.filter(exact), root)[0]
    || __aqPrefer(cands.filter(exact), root)[0]
    || __aqPrefer(vis.filter(partial), root)[0]
    || __aqPrefer(cands.filter(partial), root)[0]
    || (digitTol && __aqPrefer(vis.filter(digitTol), root)[0])
    || (digitTol && __aqPrefer(cands.filter(digitTol), root)[0]);
  return __aqPick(el);
}})()"#,
        prelude = activation_prelude(),
        name_lit = json_str(name),
        role_lit = json_str(role),
        map = role_candidate_map_js(),
    )
}

/// JS collecting the page's live accessible-name candidates for a role —
/// the input the auto-heal strategy ladder matches the recorded name
/// against. Uses the same candidate set, visibility filter, and popup-scope
/// preference as `build_role_name_click` so a strategy match is dispatchable
/// by definition. Returns `JSON.stringify({names: [...]})`; the
/// `__aqCollectNames` marker lets test doubles distinguish this probe.
pub fn build_collect_role_names(role: &str) -> String {
    format!(
        r#"(() => {{ const __aqCollectNames = true;{prelude}
  const role = {role_lit};
  const map = {map};
  const sels = (role && map[role]) ? map[role]
    : (role ? ['[role="' + role + '"]'] : Object.keys(map).reduce((a, k) => a.concat(map[k]), []));
  const cands = Array.from(document.querySelectorAll(sels.join(',')));
  const root = __aqScopeRoot();
  const out = [];
  const seen = new Set();
  // Every __aqName candidate counts — activation matches against all of
  // them, so collecting only the first would miss names living in a later
  // field (aria-label vs innerText vs labelledby).
  for (const n of __aqPrefer(cands.filter(__aqVisible), root)) {{
    for (const nm of (__aqName(n) || []).map(__aqText)) {{
      if (nm && !seen.has(nm)) {{ seen.add(nm); out.push(nm); }}
    }}
  }}
  return JSON.stringify({{ names: out }});
}})()"#,
        prelude = activation_prelude(),
        role_lit = json_str(role),
        map = role_candidate_map_js(),
    )
}

/// One entry of a resolved `locator.scope` chain, pre-baked to JS literals so
/// the builder stays free of scenario semantics (vars/i18n are resolved by the
/// caller).
#[derive(Debug)]
pub enum ScopeStep {
    /// `container.querySelector(<css>)`
    Css(String),
    /// `document.evaluate(<xpath>, container, …).singleNodeValue`
    Xpath(String),
    /// First node inside the container whose text matches `<text>` — prefers
    /// the nearest interactive element, mirroring `build_text_click`.
    Text(String),
    /// Role (+ optional resolved name) matched inside the container.
    Role { role: String, name: String },
}

/// JS narrowing `let __aqOuter` through a locator scope chain. Emits one
/// narrowing IIFE per entry; on a miss it `return "scope-miss:<index>"`s so the
/// caller can name the level that failed. Requires the prelude + `__aqScopedFind`
/// to be in scope (see [`build_scoped_role_act`]).
pub fn build_scope_chain(scope: &[ScopeStep]) -> String {
    let mut out = String::from("let __aqOuter = document;\n");
    for (i, step) in scope.iter().enumerate() {
        let inner = match step {
            ScopeStep::Css(sel) => format!(
                "const els = Array.from(r.querySelectorAll({})); return els.find(__aqVisible) || els[0] || null;",
                json_str(sel)
            ),
            ScopeStep::Xpath(xp) => format!(
                "const res = document.evaluate({}, r, null, 9, null); return res && res.singleNodeValue;",
                json_str(xp)
            ),
            ScopeStep::Text(t) => format!(
                "const want = __aqText({q}); const hits = Array.from(r.querySelectorAll('*')).filter((n) => __aqVisible(n) && __aqName(n).some((c) => __aqText(c).includes(want))); const inter = hits.filter(__aqIsInteractive).sort((a, b) => (a.textContent || '').length - (b.textContent || '').length); const anc = hits.map((n) => (n.closest ? n.closest(__aqInteractiveSel) : null)).filter(Boolean); const bare = hits.slice().sort((a, b) => (a.textContent || '').length - (b.textContent || '').length); return inter[0] || anc[0] || bare[0] || null;",
                q = json_str(t)
            ),
            ScopeStep::Role { role, name } => format!(
                "return __aqScopedFind({}, {}, r);",
                json_str(role),
                json_str(name)
            ),
        };
        out.push_str(&format!(
            "__aqOuter = (() => {{ const r = __aqOuter; if (!r || !r.querySelectorAll) return null; {inner} }})();\nif (!__aqOuter) return \"scope-miss:{i}\";\n"
        ));
    }
    out
}

/// Definition of `__aqScopedFind(roleLit, nameLit, root)` — a strict-in-root
/// role+name matcher (an explicit scope means "only here": no document-wide
/// fallback, no popup-surface preference). Empty `nameLit` resolves to the
/// first visible candidate of the role.
fn scoped_find_helper_js() -> String {
    r#"const __aqScopedFind = (roleLit, nameLit, root) => {
  const want = __aqText(nameLit);
  const map = %MAP%;
  const sels = (roleLit && map[roleLit]) ? map[roleLit]
    : (roleLit ? ['[role="' + roleLit + '"]'] : Object.keys(map).reduce((a, k) => a.concat(map[k]), []));
  const cands = Array.from(root.querySelectorAll(sels.join(',')));
  if (!want) return cands.filter(__aqVisible)[0] || cands[0] || null;
  const exact = (n) => __aqName(n).some((c) => __aqText(c) === want);
  const partial = (n) => want.length >= 3 && __aqName(n).some((c) => __aqText(c).includes(want));
  const wantND = __aqND(want);
  const digitTol = wantND.replace(/#/g, '').trim().length >= 3
    && ((n) => __aqName(n).some((c) => __aqND(__aqText(c)) === wantND));
  const vis = cands.filter(__aqVisible);
  return vis.filter(exact)[0] || cands.filter(exact)[0]
    || vis.filter(partial)[0] || cands.filter(partial)[0]
    || (digitTol && (vis.filter(digitTol)[0] || cands.filter(digitTol)[0]))
    || null;
};
"#
    .replace("%MAP%", role_candidate_map_js())
}

/// Build JS that resolves `role`+`name` strictly inside the `scope` chain and
/// activates it. Returns `"true"` / `"false"` (no inner match) /
/// `"scope-miss:<i>"` (a scope level matched nothing) on stdout.
pub fn build_scoped_role_act(
    role: &str,
    name: &str,
    scope: &[ScopeStep],
    act: browser::RoleAct,
    value: Option<&str>,
) -> String {
    let act_js = match act {
        browser::RoleAct::Click => "return __aqPick(el) ? \"true\" : \"false\";".to_string(),
        browser::RoleAct::Focus => {
            "el.scrollIntoView({block:'center',inline:'nearest'}); el.focus(); return \"true\";"
                .to_string()
        }
        browser::RoleAct::Hover => {
            "el.scrollIntoView({block:'center',inline:'nearest'}); const o = {bubbles:true,cancelable:true,view:window}; el.dispatchEvent(new PointerEvent('pointerover', o)); el.dispatchEvent(new MouseEvent('mouseover', o)); el.dispatchEvent(new MouseEvent('mousemove', o)); return \"true\";"
                .to_string()
        }
        browser::RoleAct::Fill => format!(
            "el.scrollIntoView({{block:'center',inline:'nearest'}}); el.focus(); el.value = {}; el.dispatchEvent(new Event('input', {{ bubbles: true }})); el.dispatchEvent(new Event('change', {{ bubbles: true }})); return \"true\";",
            json_str(value.unwrap_or(""))
        ),
        // Presence-probe act: touching textContent reads the element
        // without mutating it.
        browser::RoleAct::Text => "el.textContent; return \"true\";".to_string(),
    };
    format!(
        "(() => {{{prelude}\n{find}\n{chain}{act}\n}})()",
        prelude = activation_prelude(),
        // The helper is emitted before the chain because a Role scope step
        // resolves through __aqScopedFind too.
        find = scoped_find_helper_js(),
        chain = build_scope_chain(scope),
        act = scoped_target_js(role, name, &act_js),
    )
}

fn scoped_target_js(role: &str, name: &str, act_js: &str) -> String {
    format!(
        "const el = __aqScopedFind({}, {}, __aqOuter);\nif (!el) return \"false\";\n{}",
        json_str(role),
        json_str(name),
        act_js
    )
}

/// One resolved endpoint of a `drag` step — a locator already lowered to
/// JS literals (names/i18n/vars resolved by the caller).
#[derive(Debug)]
pub enum DragEndpoint {
    /// ARIA role (+ resolved accessible name) inside an optional scope chain.
    Role {
        role: String,
        name: String,
        scope: Vec<ScopeStep>,
    },
    /// `container.querySelector(<css>)` — also how testId locators arrive.
    Css(String),
    /// `document.evaluate(<xpath>, …).singleNodeValue`.
    Xpath(String),
    /// First visible in-root node whose text contains `<text>`.
    Text(String),
}

fn drag_endpoint_json(ep: &DragEndpoint) -> String {
    match ep {
        DragEndpoint::Role { role, name, scope } => {
            let steps = scope
                .iter()
                .map(|s| match s {
                    ScopeStep::Css(v) => format!("{{\"css\":{}}}", json_str(v)),
                    ScopeStep::Xpath(v) => format!("{{\"xpath\":{}}}", json_str(v)),
                    ScopeStep::Text(v) => format!("{{\"text\":{}}}", json_str(v)),
                    ScopeStep::Role { role, name } => {
                        format!(
                            "{{\"role\":{},\"name\":{}}}",
                            json_str(role),
                            json_str(name)
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"kind\":\"role\",\"role\":{},\"name\":{},\"scope\":[{}]}}",
                json_str(role),
                json_str(name),
                steps
            )
        }
        DragEndpoint::Css(v) => format!("{{\"kind\":\"css\",\"value\":{}}}", json_str(v)),
        DragEndpoint::Xpath(v) => format!("{{\"kind\":\"xpath\",\"value\":{}}}", json_str(v)),
        DragEndpoint::Text(v) => format!("{{\"kind\":\"text\",\"value\":{}}}", json_str(v)),
    }
}

/// JS that resolves the drag `src`/`dst` endpoints and dispatches a full
/// drag gesture **on the nodes themselves** — the same overlay-immune,
/// coordinate-free approach as the click activation:
///
///   pointerdown + mousedown on src
///   → dragstart(src, shared DataTransfer)
///   → mousemove + pointermove at dst
///   → dragenter + dragover + drop on dst (shared DataTransfer)
///   → pointerup + mouseup on dst → dragend(src)
///
/// Covers both HTML5 `draggable` dnd (the DragEvent chain) and
/// pointer/mouse-driven sortable libraries in one gesture.
/// Returns `"true"`, `"src-miss"`, or `"dst-miss"` on stdout.
/// `__aqDrag` marker for test doubles.
pub fn build_drag_js(src: &DragEndpoint, dst: &DragEndpoint) -> String {
    format!(
        r#"(() => {{ const __aqDrag = true;
{prelude}
{find}
{finder}
  const src = __aqDragFind({src});
  if (!src) return "src-miss";
  const dst = __aqDragFind({dst});
  if (!dst) return "dst-miss";
  try {{ src.scrollIntoView({{ block: 'center', inline: 'center' }}); }} catch (e) {{}}
  try {{ dst.scrollIntoView({{ block: 'center', inline: 'center' }}); }} catch (e) {{}}
  const ctr = (el) => {{ const r = el.getBoundingClientRect(); return {{ x: r.left + r.width / 2, y: r.top + r.height / 2 }}; }};
  const s = ctr(src), dp = ctr(dst);
  const o = (x, y, up) => ({{ bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y, button: 0, buttons: up ? 0 : 1 }});
  src.dispatchEvent(new PointerEvent('pointerdown', o(s.x, s.y)));
  src.dispatchEvent(new MouseEvent('mousedown', o(s.x, s.y)));
  try {{
    const dt = new DataTransfer();
    src.dispatchEvent(new DragEvent('dragstart', Object.assign(o(s.x, s.y), {{ dataTransfer: dt }})));
    document.dispatchEvent(new MouseEvent('mousemove', o(dp.x, dp.y)));
    dst.dispatchEvent(new PointerEvent('pointermove', o(dp.x, dp.y)));
    dst.dispatchEvent(new DragEvent('dragenter', Object.assign(o(dp.x, dp.y), {{ dataTransfer: dt }})));
    dst.dispatchEvent(new DragEvent('dragover', Object.assign(o(dp.x, dp.y), {{ dataTransfer: dt }})));
    dst.dispatchEvent(new DragEvent('drop', Object.assign(o(dp.x, dp.y), {{ dataTransfer: dt }})));
    src.dispatchEvent(new DragEvent('dragend', Object.assign(o(dp.x, dp.y), {{ dataTransfer: dt }})));
  }} catch (e) {{
    document.dispatchEvent(new MouseEvent('mousemove', o(dp.x, dp.y)));
    dst.dispatchEvent(new PointerEvent('pointermove', o(dp.x, dp.y)));
  }}
  dst.dispatchEvent(new PointerEvent('pointerup', o(dp.x, dp.y, true)));
  dst.dispatchEvent(new MouseEvent('mouseup', o(dp.x, dp.y, true)));
  return "true";
}})()"#,
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        finder = endpoint_finder_js(),
        src = drag_endpoint_json(src),
        dst = drag_endpoint_json(dst),
    )
}

/// The endpoint resolver shared by `drag`, `contextmenu`, and friends:
/// `__aqDragFind({kind, value?|role?|…, scope})` → the node or null. Pair it
/// with `activation_prelude()` + `scoped_find_helper_js()`.
fn endpoint_finder_js() -> String {
    r#"  const __aqDragFind = (d) => {
    let root = document;
    for (const s of (d.scope || [])) {
      if (!root || !root.querySelectorAll) return null;
      if (s.css != null) { const els = Array.from(root.querySelectorAll(s.css)); root = els.find(__aqVisible) || els[0] || null; }
      else if (s.xpath != null) { const r = document.evaluate(s.xpath, root, null, 9, null); root = r && r.singleNodeValue; }
      else if (s.role != null) root = __aqScopedFind(s.role, s.name || '', root);
      else if (s.text != null) { const want = __aqText(s.text); const hits = Array.from(root.querySelectorAll('*')).filter((n) => __aqVisible(n) && __aqName(n).some((c) => __aqText(c).includes(want))); const inter = hits.filter(__aqIsInteractive); root = inter[0] || hits.sort((a, b) => (a.textContent || '').length - (b.textContent || '').length)[0] || null; }
      if (!root) return null;
    }
    if (d.kind === 'role') return __aqScopedFind(d.role, d.name || '', root);
    if (d.kind === 'css') { const els = Array.from(root.querySelectorAll(d.value)); return els.find(__aqVisible) || els[0] || null; }
    if (d.kind === 'xpath') { const r = document.evaluate(d.value, root, null, 9, null); return r && r.singleNodeValue; }
    if (d.kind === 'text') { const want = __aqText(d.value); const hits = Array.from(root.querySelectorAll('*')).filter((n) => __aqVisible(n) && __aqName(n).some((c) => __aqText(c).includes(want))); const inter = hits.filter(__aqIsInteractive); return inter[0] || hits.sort((a, b) => (a.textContent || '').length - (b.textContent || '').length)[0] || null; }
    return null;
  };"#
        .to_string()
}

/// Resolve one endpoint and report where it stands in the DOM: `"detached"`
/// when nothing resolves, `"visible"`, or `"hidden"` (in the DOM but with no
/// layout box or `display:none`/`visibility:hidden`). The `wait` verb polls
/// this to implement `params.locator` + `params.state`.
/// `__aqWaitState` marker for test doubles.
pub fn build_element_state_js(ep: &DragEndpoint) -> String {
    format!(
        r#"(() => {{ const __aqWaitState = true;
{prelude}
{find}
{finder}
  const el = __aqDragFind({ep});
  if (!el) return "detached";
  try {{
    const cs = getComputedStyle(el);
    if (cs.display === "none" || cs.visibility === "hidden" || cs.visibility === "collapse") return "hidden";
  }} catch (e) {{}}
  return __aqVisible(el) ? "visible" : "hidden";
}})()"#,
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        finder = endpoint_finder_js(),
        ep = drag_endpoint_json(ep),
    )
}

/// JS resolving one endpoint and dispatching a secondary-button click on it:
/// `pointerdown{button:2} → mousedown{button:2} → pointerup{button:2} →
/// mouseup{button:2} → contextmenu{button:2}` — no `click` (a real right
/// click never fires one). Returns `"true"` / `"miss"`.
/// `__aqCtx` marker for test doubles.
pub fn build_contextmenu_js(ep: &DragEndpoint) -> String {
    format!(
        r#"(() => {{ const __aqCtx = true;
{prelude}
{find}
{finder}
  const el = __aqDragFind({ep});
  if (!el) return "miss";
  try {{ el.scrollIntoView({{ block: 'center', inline: 'center' }}); }} catch (e) {{}}
  const r = el.getBoundingClientRect();
  const x = r.left + r.width / 2, y = r.top + r.height / 2;
  const dn = {{ bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y, button: 2, buttons: 2 }};
  const up = {{ bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y, button: 2, buttons: 0 }};
  // Defer past the eval's response window — a contextmenu handler can open a
  // native dialog which blocks the JS thread and eats the eval result.
  setTimeout(() => {{
    try {{
      el.dispatchEvent(new PointerEvent('pointerdown', dn));
      el.dispatchEvent(new MouseEvent('mousedown', dn));
      el.dispatchEvent(new PointerEvent('pointerup', up));
      el.dispatchEvent(new MouseEvent('mouseup', up));
      el.dispatchEvent(new MouseEvent('contextmenu', dn));
    }} catch (e) {{}}
  }}, 150);
  return "true";
}})()"#,
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        finder = endpoint_finder_js(),
        ep = drag_endpoint_json(ep),
    )
}

/// JS returning `"true"` when `role`+`name` resolves inside `scope` — the
/// claims presence probe. Same miss encoding as [`build_scoped_role_act`].
pub fn build_scoped_role_probe(role: &str, name: &str, scope: &[ScopeStep]) -> String {
    format!(
        "(() => {{{prelude}\n{find}\n{chain}return el ? \"true\" : \"false\";\n}})()",
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        chain = build_scope_chain(scope)
            + "const el = __aqScopedFind("
            + &json_str(role)
            + ", "
            + &json_str(name)
            + ", __aqOuter);\n",
    )
}

/// JS returning up to 4 field-level rejection evidence strings — inputs
/// that failed constraint validation (`:invalid`, `aria-invalid="true"`)
/// plus the step's own element when `css_hint` resolves. Complements the
/// global alert probe: form validation usually surfaces per-field via
/// `validationMessage`/`aria-describedby` and never raises a banner.
/// `__aqFieldProbe` marker for test doubles.
pub fn build_field_rejection_probe(css_hint: Option<&str>) -> String {
    let hint_expr = css_hint
        .map(|s| format!("document.querySelector({})", json_str(s)))
        .unwrap_or_else(|| "null".to_string());
    format!(
        r#"(() => {{ const __aqFieldProbe = true;
  const vis = (n) => {{ try {{ return n.getClientRects().length > 0; }} catch (e) {{ return false; }} }};
  const evidence = (el) => {{
    try {{
      const name = el.getAttribute('aria-label') || el.getAttribute('placeholder') ||
        el.getAttribute('name') || el.id || el.tagName.toLowerCase();
      const desc = (el.getAttribute('aria-describedby') || '').split(/\s+/)
        .map((i) => {{ const d = document.getElementById(i); return d ? (d.innerText || '').trim() : ''; }})
        .filter(Boolean).join(' ');
      const msg = el.validationMessage || desc || 'invalid';
      return (name + ': ' + msg).slice(0, 160);
    }} catch (e) {{ return null; }}
  }};
  const hits = [];
  const seen = new Set();
  const push = (el, prefix) => {{
    if (!el || seen.has(el)) return;
    seen.add(el);
    const e = evidence(el);
    if (e) hits.push(prefix + e);
  }};
  const hinted = {hint_expr};
  if (hinted && (hinted.matches(':invalid') || hinted.getAttribute('aria-invalid') === 'true')) {{
    push(hinted, 'field: ');
  }}
  const active = document.activeElement;
  if (active && active.matches && (active.matches(':invalid') || active.getAttribute('aria-invalid') === 'true')) {{
    push(active, 'field(active): ');
  }}
  for (const el of document.querySelectorAll('[aria-invalid="true"], input:invalid, select:invalid, textarea:invalid')) {{
    if (hits.length >= 4) break;
    if (vis(el)) push(el, 'field: ');
  }}
  return JSON.stringify(hits);
}})()"#
    )
}

/// JS returning up to 3 visible alert/banner/toast texts — evidence that a
/// failed step was a value rejection (the app refused the submitted input)
/// rather than a locator miss. `__aqRejectProbe` marker for test doubles.
pub fn build_rejection_probe() -> String {
    r#"(() => { const __aqRejectProbe = true;
  const vis = (n) => { try { return n.getClientRects().length > 0; } catch (e) { return false; } };
  const hits = Array.from(document.querySelectorAll(
    '[role=alert],[aria-live],[role=status],[class*="error"],[class*="Error"],[class*="toast"],[class*="Toast"],[class*="banner"],[class*="Banner"]'
  ))
    .filter(vis)
    .map((n) => (n.innerText || n.textContent || '').trim())
    .filter(Boolean)
    .slice(0, 3);
  return JSON.stringify(hits);
})()"#
        .to_string()
}

/// Build JS that resolves an element by visible text and activates it. Prefers
/// an interactive element (or the nearest interactive ancestor of a matching
/// text/label node — the "covered label" case), else the shortest-text match.
/// Returns `"true"`/`"false"`.
pub fn build_text_click(text: &str) -> String {
    format!(
        r#"(() => {{{prelude}
  const want = __aqText({text_lit});
  if (!want) return false;
  const wantND = __aqND(want);
  const digitTolOk = wantND.replace(/#/g, '').trim().length >= 3;
  const nameHit = (n) => __aqName(n).some((c) => __aqText(c).includes(want) || (digitTolOk && __aqND(__aqText(c)) === wantND));
  const all = Array.from(document.querySelectorAll('*'));
  const matches = all.filter((n) => __aqVisible(n) && nameHit(n));
  const root = __aqScopeRoot();
  const byLen = (a, b) => (a.textContent || '').length - (b.textContent || '').length;
  // 1: an interactive match, smallest text (most specific).
  const interactive = __aqPrefer(matches.filter(__aqIsInteractive), root).sort(byLen);
  // 2: the nearest interactive ancestor of a matching (possibly covered) node.
  const ancestors = __aqPrefer(
    matches.map((n) => (n.closest ? n.closest(__aqInteractiveSel) : null)).filter(Boolean),
    root,
  );
  // 3: last resort, the smallest matching node itself.
  const bare = __aqPrefer(matches.slice(), root).sort(byLen);
  const el = interactive[0] || ancestors[0] || bare[0];
  return __aqPick(el);
}})()"#,
        prelude = activation_prelude(),
        text_lit = json_str(text),
    )
}

/// JS returning the count of currently-open popup surfaces (open listboxes,
/// rendered options, menus, and expanded openers). A grow between before/after
/// an opener activation is the signal the popup actually opened. Returns a
/// bare integer on stdout.
pub fn build_popup_probe() -> &'static str {
    r#"(() => {
  const vis = (n) => { try { return n.getClientRects().length > 0; } catch (e) { return false; } };
  const count = (sel) => Array.from(document.querySelectorAll(sel)).filter(vis).length;
  return (
    count('[role=listbox]') + count('[role=option]') + count('[role=menu]') +
    count('[role=menuitem]') + count('[aria-expanded="true"]')
  );
})()"#
}

/// Whether an agent-browser eval return represents JS boolean `true`. eval
/// serializes a bare boolean as `true`, but a stray quoting layer would make it
/// `"true"` — accept both so a serialization quirk can't silently disable
/// activation.
fn eval_is_true(out: &str) -> bool {
    let t = out.trim().trim_matches('"');
    t == "true"
}

/// How many times to re-probe when an activation finds no match, and the gap
/// between tries. Popups/options often render a frame or two after the opener
/// click returns; a bounded poll rides that out without a hard-coded wait.
/// Tests set the count to 1 (no sleeping) via the env var.
fn retry_attempts() -> u32 {
    if std::env::var_os("AGENT_QA_DOM_ACTIVATE_NO_RETRY").is_some() {
        1
    } else {
        4
    }
}
const RETRY_GAP_MS: u64 = 200;

/// Eval an activation expression, retrying while it reports "no match" — a
/// no-op find is safe to repeat, and a `true` return (something was clicked)
/// stops immediately so we never double-activate.
fn activate_with_retry(session: &str, expr: &str) -> anyhow::Result<bool> {
    let attempts = retry_attempts();
    for i in 0..attempts {
        if eval_is_true(&browser::eval_expression(session, expr)?) {
            return Ok(true);
        }
        if i + 1 < attempts {
            std::thread::sleep(std::time::Duration::from_millis(RETRY_GAP_MS));
        }
    }
    Ok(false)
}

/// Outcome of a scoped-locator activation. `Done` = the element inside the
/// scope chain was acted on; `Miss` = the chain resolved but no role+name
/// matched within it; `ScopeMiss(i)` = scope level `i` matched no element.
#[derive(Debug, PartialEq, Eq)]
pub enum ScopedOutcome {
    Done,
    Miss,
    ScopeMiss(usize),
}

fn parse_scoped_outcome(out: &str) -> ScopedOutcome {
    let t = out.trim().trim_matches('"');
    if t == "true" {
        ScopedOutcome::Done
    } else if let Some(idx) = t.strip_prefix("scope-miss:") {
        ScopedOutcome::ScopeMiss(idx.parse().unwrap_or(0))
    } else {
        ScopedOutcome::Miss
    }
}

/// Activate a role+name element strictly inside a `scope` chain, retrying
/// while either level reports a miss — late-mounted panels ride out the poll
/// without a hard-coded wait. Never falls back to document-wide matching.
pub fn act_scoped(
    session: &str,
    role: &str,
    name: &str,
    scope: &[ScopeStep],
    act: browser::RoleAct,
    value: Option<&str>,
) -> anyhow::Result<ScopedOutcome> {
    let expr = build_scoped_role_act(role, name, scope, act, value);
    let attempts = retry_attempts();
    let mut last = ScopedOutcome::Miss;
    for i in 0..attempts {
        let out = browser::eval_expression(session, &expr)?;
        last = parse_scoped_outcome(&out);
        if matches!(last, ScopedOutcome::Done) || i + 1 == attempts {
            return Ok(last);
        }
        std::thread::sleep(std::time::Duration::from_millis(RETRY_GAP_MS));
    }
    Ok(last)
}

/// Presence probe for a scoped role locator — `true` when the role+name
/// resolves inside the chain.
pub fn probe_scoped(
    session: &str,
    role: &str,
    name: &str,
    scope: &[ScopeStep],
) -> anyhow::Result<bool> {
    let expr = build_scoped_role_probe(role, name, scope);
    let attempts = retry_attempts();
    for i in 0..attempts {
        let out = browser::eval_expression(session, &expr)?;
        match parse_scoped_outcome(&out) {
            ScopedOutcome::Done => return Ok(true),
            _ if i + 1 == attempts => return Ok(false),
            _ => std::thread::sleep(std::time::Duration::from_millis(RETRY_GAP_MS)),
        }
    }
    Ok(false)
}

/// Activate an element by role + name in-page. `Ok(true)` if a node matched and
/// was activated, `Ok(false)` if nothing matched (caller falls back).
pub fn activate_role_name(session: &str, role: &str, name: &str) -> anyhow::Result<bool> {
    activate_with_retry(session, &build_role_name_click(role, name))
}

/// Activate an element by visible text in-page (with covered-label recovery).
pub fn activate_text(session: &str, text: &str) -> anyhow::Result<bool> {
    activate_with_retry(session, &build_text_click(text))
}

/// Count open popup surfaces right now. Returns 0 on any probe error so callers
/// treating "did it grow?" as a soft signal never hard-fail on the probe.
pub fn popup_count(session: &str) -> u32 {
    match browser::eval_expression(session, build_popup_probe()) {
        Ok(s) => s.trim().trim_matches('"').parse().unwrap_or(0),
        Err(_) => 0,
    }
}

/// Shared element finder for the gesture verbs — same descriptor-driven
/// resolution as [`build_drag_js`] (css/xpath/text/role+scope).
/// `__aqGestureFind` marker for test doubles.
fn gesture_finder_js() -> &'static str {
    r#"
  const __aqGestureFind = (d) => {
    let root = document;
    for (const s of (d.scope || [])) {
      if (!root || !root.querySelectorAll) return null;
      if (s.css != null) root = root.querySelector(s.css);
      else if (s.xpath != null) { const r = document.evaluate(s.xpath, root, null, 9, null); root = r && r.singleNodeValue; }
      else if (s.role != null) root = __aqScopedFind(s.role, s.name || '', root);
      else if (s.text != null) { const want = __aqText(s.text); const hits = Array.from(root.querySelectorAll('*')).filter((n) => __aqVisible(n) && __aqName(n).some((c) => __aqText(c).includes(want))); const inter = hits.filter(__aqIsInteractive); root = inter[0] || hits.sort((a, b) => (a.textContent || '').length - (b.textContent || '').length)[0] || null; }
      if (!root) return null;
    }
    if (d.kind === 'role') return __aqScopedFind(d.role, d.name || '', root);
    if (d.kind === 'css') return root.querySelector(d.value);
    if (d.kind === 'xpath') { const r = document.evaluate(d.value, root, null, 9, null); return r && r.singleNodeValue; }
    if (d.kind === 'text') { const want = __aqText(d.value); const hits = Array.from(root.querySelectorAll('*')).filter((n) => __aqVisible(n) && __aqName(n).some((c) => __aqText(c).includes(want))); const inter = hits.filter(__aqIsInteractive); return inter[0] || hits.sort((a, b) => (a.textContent || '').length - (b.textContent || '').length)[0] || null; }
    return null;
  };
"#
}

/// Hold an element pressed: dispatches pointerdown + mousedown at the
/// element's center and leaves the button held — the release is a second
/// eval ([`build_hold_up_js`]) after the caller's sleep. Element handles
/// that arm on `pointerdown`/`mousedown` (long-press menus, press-and-hold
/// buttons) see a real gesture. Returns `"true"`/`"el-miss"`.
/// `__aqHold` marker for test doubles.
pub fn build_hold_down_js(ep: &DragEndpoint) -> String {
    format!(
        r#"(() => {{ const __aqHold = true;
{prelude}
{find}
{finder}
  const el = __aqGestureFind({ep});
  if (!el) return "el-miss";
  try {{ el.scrollIntoView({{ block: 'center', inline: 'center' }}); }} catch (e) {{}}
  const r = el.getBoundingClientRect();
  const o = {{ bubbles: true, cancelable: true, composed: true, view: window, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 0, buttons: 1 }};
  el.dispatchEvent(new PointerEvent('pointerdown', o));
  el.dispatchEvent(new MouseEvent('mousedown', o));
  if (typeof TouchEvent === 'function' && typeof Touch === 'function') {{
    const t = new Touch({{ identifier: 1, target: el, clientX: o.clientX, clientY: o.clientY }});
    el.dispatchEvent(new TouchEvent('touchstart', Object.assign({{}}, o, {{ touches: [t], targetTouches: [t], changedTouches: [t] }})));
  }}
  window.__aqHoldEl = el;
  return "true";
}})()"#,
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        finder = gesture_finder_js(),
        ep = drag_endpoint_json(ep),
    )
}

/// Release a [`build_hold_down_js`] hold: pointerup + mouseup + touchend
/// on the same element (and a plain `click` is deliberately NOT sent —
/// a hold is not a tap). Returns `"true"` or `"no-hold"`.
/// `__aqHoldUp` marker for test doubles.
pub fn build_hold_up_js() -> String {
    r#"(() => { const __aqHoldUp = true;
  const el = window.__aqHoldEl;
  delete window.__aqHoldEl;
  if (!el || !el.isConnected) return "no-hold";
  const r = el.getBoundingClientRect();
  const o = { bubbles: true, cancelable: true, composed: true, view: window, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 0, buttons: 0 };
  el.dispatchEvent(new PointerEvent('pointerup', o));
  el.dispatchEvent(new MouseEvent('mouseup', o));
  if (typeof TouchEvent === 'function' && typeof Touch === 'function') {
    const t = new Touch({ identifier: 1, target: el, clientX: o.clientX, clientY: o.clientY });
    el.dispatchEvent(new TouchEvent('touchend', Object.assign({}, o, { touches: [], targetTouches: [], changedTouches: [t] })));
  }
  return "true";
})()"#
        .to_string()
}

/// Swipe on an element (or the viewport when `ep` is `None`): dispatches
/// touchstart → 8 touchmove steps → touchend *and* the pointer/mouse
/// sequence pointer-driven carousels listen for. `direction` is the
/// direction the finger travels — `up` moves the point up (content
/// scrolls down). Returns `"true"`/`"el-miss"`/`"bad-dir"`.
/// `__aqSwipe` marker for test doubles.
pub fn build_swipe_js(ep: Option<&DragEndpoint>, direction: &str, distance: f64) -> String {
    let find_src = match ep {
        Some(ep) => format!(
            "  const el = __aqGestureFind({ep});\n  if (!el) return \"el-miss\";\n  try {{ el.scrollIntoView({{ block: 'center', inline: 'center' }}); }} catch (e) {{}}\n  const r = el.getBoundingClientRect();\n  const sx = r.left + r.width / 2, sy = r.top + r.height / 2;\n  const target = el;",
            ep = drag_endpoint_json(ep)
        ),
        None => "  const sx = innerWidth / 2, sy = innerHeight / 2;\n  const target = document.elementFromPoint(sx, sy) || document.body;".to_string(),
    };
    format!(
        r#"(() => {{ const __aqSwipe = true;
{prelude}
{find}
{finder}
{find_src}
  const D = {distance};
  let dx = 0, dy = 0;
  if ({dir} === 'up') dy = -D;
  else if ({dir} === 'down') dy = D;
  else if ({dir} === 'left') dx = -D;
  else if ({dir} === 'right') dx = D;
  else return "bad-dir";
  const ev = (x, y, buttons) => ({{ bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y, button: 0, buttons }});
  const hasTouch = typeof TouchEvent === 'function' && typeof Touch === 'function';
  const mkTouch = (x, y) => hasTouch ? new Touch({{ identifier: 7, target, clientX: x, clientY: y }}) : null;
  target.dispatchEvent(new PointerEvent('pointerdown', ev(sx, sy, 1)));
  target.dispatchEvent(new MouseEvent('mousedown', ev(sx, sy, 1)));
  if (hasTouch) {{ const t = mkTouch(sx, sy); target.dispatchEvent(new TouchEvent('touchstart', Object.assign({{}}, ev(sx, sy, 1), {{ touches: [t], targetTouches: [t], changedTouches: [t] }}))); }}
  const STEPS = 8;
  for (let i = 1; i <= STEPS; i++) {{
    const x = sx + dx * i / STEPS, y = sy + dy * i / STEPS;
    document.dispatchEvent(new PointerEvent('pointermove', ev(x, y, 1)));
    document.dispatchEvent(new MouseEvent('mousemove', ev(x, y, 1)));
    if (hasTouch) {{ const t = mkTouch(x, y); target.dispatchEvent(new TouchEvent('touchmove', Object.assign({{}}, ev(x, y, 1), {{ touches: [t], targetTouches: [t], changedTouches: [t] }}))); }}
  }}
  target.dispatchEvent(new PointerEvent('pointerup', ev(sx + dx, sy + dy, 0)));
  document.dispatchEvent(new MouseEvent('mouseup', ev(sx + dx, sy + dy, 0)));
  if (hasTouch) {{ const t = mkTouch(sx + dx, sy + dy); target.dispatchEvent(new TouchEvent('touchend', Object.assign({{}}, ev(sx + dx, sy + dy, 0), {{ touches: [], targetTouches: [], changedTouches: [t] }}))); }}
  return "true";
}})()"#,
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        finder = gesture_finder_js(),
        find_src = find_src,
        distance = distance,
        dir = json_str(direction),
    )
}

/// `build_pinch_js` — two-finger pinch/zoom via a synthesized
/// TouchEvent chain (two Touches moving toward or away from each other)
/// plus a ctrlKey WheelEvent (the desktop trackpad pinch convention).
/// `direction`: "in" = fingers travel toward each other (zoom out),
/// "out" = apart (zoom in). `distance` = per-finger travel in px.
/// `__aqPinch` marker for test doubles.
pub fn build_pinch_js(ep: Option<&DragEndpoint>, direction: &str, distance: f64) -> String {
    let find_src = match ep {
        Some(ep) => format!(
            "  const el = __aqGestureFind({ep});
  if (!el) return \"el-miss\";
  try {{ el.scrollIntoView({{ block: 'center', inline: 'center' }}); }} catch (e) {{}}
  const r = el.getBoundingClientRect();
  const sx = r.left + r.width / 2, sy = r.top + r.height / 2;
  const target = el;",
            ep = drag_endpoint_json(ep)
        ),
        None => "  const sx = innerWidth / 2, sy = innerHeight / 2;
  const target = document.elementFromPoint(sx, sy) || document.body;"
            .to_string(),
    };
    format!(
        r#"(() => {{ const __aqPinch = true;
{prelude}
{find}
{finder}
{find_src}
  const D = {distance};
  const IN = {dir} === 'in', OUT = {dir} === 'out';
  if (!IN && !OUT) return "bad-dir";
  const ev = (x, y) => ({{ bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y }});
  const hasTouch = typeof TouchEvent === 'function' && typeof Touch === 'function';
  const mkTouch = (id, x, y) => hasTouch ? new Touch({{ identifier: id, target, clientX: x, clientY: y }}) : null;
  // Two fingers a fixed radius apart, on the vertical axis through the
  // center. "in" starts spread and converges; "out" starts together and
  // spreads. Touches report their position on the shared target.
  const R0 = IN ? D : 1, R1 = IN ? 1 : D;
  if (hasTouch) {{
    const t1 = mkTouch(1, sx, sy - R0), t2 = mkTouch(2, sx, sy + R0);
    target.dispatchEvent(new TouchEvent('touchstart', Object.assign({{}}, ev(sx, sy), {{ touches: [t1, t2], targetTouches: [t1, t2], changedTouches: [t1, t2] }})));
    const STEPS = 8;
    for (let i = 1; i <= STEPS; i++) {{
      const r = R0 + (R1 - R0) * i / STEPS;
      const a = mkTouch(1, sx, sy - r), b = mkTouch(2, sx, sy + r);
      target.dispatchEvent(new TouchEvent('touchmove', Object.assign({{}}, ev(sx, sy), {{ touches: [a, b], targetTouches: [a, b], changedTouches: [a, b] }})));
    }}
    const e1 = mkTouch(1, sx, sy - R1), e2 = mkTouch(2, sx, sy + R1);
    target.dispatchEvent(new TouchEvent('touchend', Object.assign({{}}, ev(sx, sy), {{ touches: [], targetTouches: [], changedTouches: [e1, e2] }})));
  }}
  // Also fire the ctrlKey wheel so pages that map trackpad pinch to
  // wheel+zoom react even without touch support.
  target.dispatchEvent(new WheelEvent('wheel', Object.assign({{}}, ev(sx, sy), {{ deltaY: IN ? 120 : -120, ctrlKey: true }})));
  return "true";
}})()"#,
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        finder = gesture_finder_js(),
        find_src = find_src,
        distance = distance,
        dir = json_str(direction),
    )
}

/// Two-finger rotate on an element (or the viewport when `ep` is `None`):
/// two touches on a horizontal line `radius` px either side of the center,
/// sweeping the line through `degrees` — positive is clockwise (screen
/// coords put +y down, so a rising atan2 angle reads as a clockwise turn).
/// touchstart → 8 touchmove steps → touchend. There is no desktop
/// convention for rotate (ctrl-wheel is pinch), so a browser without
/// Touch/TouchEvent reports `"no-touch"` instead of faking a fallback.
/// `__aqRotate` marker for test doubles.
pub fn build_rotate_js(ep: Option<&DragEndpoint>, degrees: f64, radius: f64) -> String {
    let find_src = match ep {
        Some(ep) => format!(
            "  const el = __aqGestureFind({ep});
  if (!el) return \"el-miss\";
  try {{ el.scrollIntoView({{ block: 'center', inline: 'center' }}); }} catch (e) {{}}
  const r = el.getBoundingClientRect();
  const sx = r.left + r.width / 2, sy = r.top + r.height / 2;
  const target = el;",
            ep = drag_endpoint_json(ep)
        ),
        None => "  const sx = innerWidth / 2, sy = innerHeight / 2;
  const target = document.elementFromPoint(sx, sy) || document.body;"
            .to_string(),
    };
    format!(
        r#"(() => {{ const __aqRotate = true;
{prelude}
{find}
{finder}
{find_src}
  const R = {radius}, RAD = {degrees} * Math.PI / 180;
  const ev = (x, y) => ({{ bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y }});
  const hasTouch = typeof TouchEvent === 'function' && typeof Touch === 'function';
  if (!hasTouch) return "no-touch";
  const mkTouch = (id, x, y) => new Touch({{ identifier: id, target, clientX: x, clientY: y }});
  const t1 = (a) => mkTouch(1, sx + R * Math.cos(a), sy + R * Math.sin(a));
  const t2 = (a) => mkTouch(2, sx - R * Math.cos(a), sy - R * Math.sin(a));
  target.dispatchEvent(new TouchEvent('touchstart', Object.assign({{}}, ev(sx, sy), {{ touches: [t1(0), t2(0)], targetTouches: [t1(0), t2(0)], changedTouches: [t1(0), t2(0)] }})));
  const STEPS = 8;
  for (let i = 1; i <= STEPS; i++) {{
    const a = RAD * i / STEPS;
    target.dispatchEvent(new TouchEvent('touchmove', Object.assign({{}}, ev(sx, sy), {{ touches: [t1(a), t2(a)], targetTouches: [t1(a), t2(a)], changedTouches: [t1(a), t2(a)] }})));
  }}
  target.dispatchEvent(new TouchEvent('touchend', Object.assign({{}}, ev(sx, sy), {{ touches: [], targetTouches: [], changedTouches: [t1(RAD), t2(RAD)] }})));
  return "true";
}})()"#,
        prelude = activation_prelude(),
        find = scoped_find_helper_js(),
        finder = gesture_finder_js(),
        find_src = find_src,
        radius = radius,
        degrees = degrees,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_role_set() {
        assert!(is_interactive_role("combobox"));
        assert!(is_interactive_role("option"));
        assert!(is_interactive_role("button"));
        assert!(!is_interactive_role("heading"));
        assert!(!is_interactive_role("text"));
    }

    #[test]
    fn popup_opener_roles() {
        assert!(is_popup_opener_role("combobox"));
        assert!(!is_popup_opener_role("button"));
    }

    #[test]
    fn contextmenu_js_dispatches_secondary_button_chain() {
        let ep = DragEndpoint::Css("#hot-spot".to_string());
        let js = build_contextmenu_js(&ep);
        assert!(js.contains("__aqCtx"), "marker");
        assert!(js.contains("#hot-spot"), "selector embedded");
        assert!(js.contains("button: 2"), "secondary button");
        assert!(js.contains("contextmenu"), "contextmenu event");
        assert!(js.contains("setTimeout"), "deferred past eval window");
    }

    #[test]
    fn role_name_click_embeds_name_role_and_chain() {
        let js = build_role_name_click("combobox", "Select a license");
        assert!(js.contains("Select a license"));
        assert!(js.contains("\"combobox\""));
        // Full pointer + mouse chain and scrollIntoView present.
        assert!(js.contains("pointerdown"));
        assert!(js.contains("mousedown"));
        assert!(js.contains("pointerup"));
        assert!(js.contains("mouseup"));
        assert!(js.contains("scrollIntoView"));
        assert!(js.contains("requestSubmit"));
        // Synthetic clicks must focus like a real mouse click so keyboard
        // input lands on the element afterwards.
        assert!(js.contains("focus()"));
    }

    #[test]
    fn role_name_click_escapes_quotes() {
        let js = build_role_name_click("button", "say \"hi\"");
        // Name is JSON-escaped, so the raw unescaped form must not appear.
        assert!(js.contains("say \\\"hi\\\""));
    }

    #[test]
    fn text_click_walks_to_interactive_ancestor() {
        let js = build_text_click("Select a license");
        assert!(js.contains("Select a license"));
        assert!(js.contains("closest"));
        assert!(js.contains("scrollIntoView"));
        assert!(js.contains("pointerdown"));
    }

    #[test]
    fn role_name_click_has_digit_tolerant_and_labelledby_tiers() {
        let js = build_role_name_click("option", "Optimize 9986 licenses available");
        // Digit normalization + aria-labelledby resolution both present.
        assert!(js.contains("__aqND"));
        assert!(js.contains("aria-labelledby"));
        // Backdrop wrappers (role=presentation/none) are excluded.
        assert!(js.contains("presentation"));
    }

    #[test]
    fn scope_chain_narrows_and_marks_the_level() {
        let js = build_scope_chain(&[
            ScopeStep::Css("[data-testid=\"card-a\"]".into()),
            ScopeStep::Role {
                role: "listbox".into(),
                name: "Options".into(),
            },
            ScopeStep::Xpath("//div[@id='x']".into()),
        ]);
        assert!(
            js.contains("querySelectorAll(\"[data-testid=\\\"card-a\\\"]\")"),
            "{js}"
        );
        // Css scope steps resolve to the first *visible* match — sites that
        // render hidden duplicate controls (responsive twins, sticky bars)
        // must not absorb interactions meant for the visible one.
        assert!(js.contains("els.find(__aqVisible)"), "{js}");
        assert!(js.contains("scope-miss:0"), "{js}");
        assert!(js.contains("scope-miss:1"), "{js}");
        assert!(js.contains("scope-miss:2"), "{js}");
        // Role scope steps resolve through the strict-in-root matcher; xpath
        // evaluates against the narrowed context node.
        assert!(
            js.contains("__aqScopedFind(\"listbox\", \"Options\", r)"),
            "{js}"
        );
        assert!(js.contains("document.evaluate"), "{js}");
    }

    #[test]
    fn scoped_act_is_strictly_in_root() {
        let js = build_scoped_role_act(
            "button",
            "Select All",
            &[ScopeStep::Css("#card".into())],
            browser::RoleAct::Click,
            None,
        );
        // Candidates come from the resolved container, not the document.
        assert!(js.contains("root.querySelectorAll(sels.join(','))"), "{js}");
        assert!(js.contains("\"scope-miss:0\""), "{js}");
        assert!(js.contains("__aqPick(el)"), "{js}");
        assert!(js.contains("\"Select All\""), "{js}");
    }

    #[test]
    fn scoped_fill_emits_input_and_change() {
        let js = build_scoped_role_act(
            "textbox",
            "Email",
            &[ScopeStep::Css("#form".into())],
            browser::RoleAct::Fill,
            Some("a@b.c"),
        );
        assert!(js.contains("el.value = \"a@b.c\""), "{js}");
        assert!(js.contains("new Event('input'"), "{js}");
        assert!(js.contains("new Event('change'"), "{js}");
    }

    #[test]
    fn scoped_probe_returns_presence() {
        let js = build_scoped_role_probe("link", "Docs", &[ScopeStep::Css("nav".into())]);
        assert!(
            js.contains("const el = __aqScopedFind(\"link\", \"Docs\", __aqOuter)"),
            "{js}"
        );
        assert!(js.contains("\"scope-miss:0\""), "{js}");
    }

    #[test]
    fn popup_probe_counts_surfaces() {
        let js = build_popup_probe();
        assert!(js.contains("role=listbox"));
        assert!(js.contains("role=option"));
        assert!(js.contains("aria-expanded=\\\"true\\\"") || js.contains("aria-expanded=\"true\""));
    }

    #[test]
    fn field_probe_reads_constraint_validation_and_hint() {
        let js = build_field_rejection_probe(Some("#email"));
        assert!(js.contains("__aqFieldProbe"));
        assert!(js.contains(":invalid"));
        assert!(js.contains("aria-invalid"));
        assert!(js.contains("validationMessage"));
        assert!(js.contains("aria-describedby"));
        assert!(js.contains("document.querySelector(\"#email\")"));
        assert!(js.contains("activeElement"));
    }

    #[test]
    fn field_probe_without_hint_skips_element_lookup() {
        let js = build_field_rejection_probe(None);
        assert!(js.contains("const hinted = null;"));
    }

    #[test]
    fn drag_js_emits_finder_and_full_gesture() {
        let js = build_drag_js(
            &DragEndpoint::Role {
                role: "listitem".into(),
                name: "Card A".into(),
                scope: vec![ScopeStep::Css("#board".into())],
            },
            &DragEndpoint::Css(".dropzone".into()),
        );
        assert!(js.contains("__aqDrag"), "{js}");
        assert!(js.contains("\"listitem\""), "{js}");
        assert!(js.contains("\"Card A\""), "{js}");
        assert!(js.contains("\"#board\""), "{js}");
        assert!(js.contains("\".dropzone\""), "{js}");
        for evt in [
            "pointerdown",
            "mousedown",
            "dragstart",
            "dragenter",
            "dragover",
            "drop",
            "dragend",
            "pointerup",
            "mouseup",
        ] {
            assert!(js.contains(evt), "{evt} missing from {js}");
        }
        assert!(js.contains("new DataTransfer()"), "{js}");
        assert!(js.contains("src-miss") && js.contains("dst-miss"), "{js}");
    }

    #[test]
    fn drag_js_supports_xpath_and_text_endpoints() {
        let js = build_drag_js(
            &DragEndpoint::Xpath("//li[1]".into()),
            &DragEndpoint::Text("Archive".into()),
        );
        assert!(js.contains("\"kind\":\"xpath\""), "{js}");
        assert!(js.contains("\"kind\":\"text\""), "{js}");
        assert!(js.contains("//li[1]") && js.contains("Archive"), "{js}");
    }

    #[test]
    fn element_state_js_reports_detached_hidden_visible() {
        let js = build_element_state_js(&DragEndpoint::Css("#probe".into()));
        assert!(js.contains("\"#probe\""), "{js}");
        assert!(js.contains("__aqWaitState"), "{js}");
        assert!(
            js.contains("\"detached\"") && js.contains("\"hidden\"") && js.contains("\"visible\""),
            "{js}"
        );
        assert!(js.contains("getComputedStyle"), "{js}");
    }

    #[test]
    fn hold_js_emits_down_events_without_click() {
        let js = build_hold_down_js(&DragEndpoint::Css("#btn".into()));
        assert!(js.contains("__aqHold"), "{js}");
        assert!(js.contains("\"#btn\""), "{js}");
        for evt in ["pointerdown", "mousedown", "touchstart"] {
            assert!(js.contains(evt), "{js}");
        }
        // the shared prelude carries a click helper (__aqPick) — hold must never invoke it
        assert!(!js.contains("__aqPick("), "{js}");
        let up = build_hold_up_js();
        assert!(up.contains("__aqHoldUp"), "{up}");
        assert!(up.contains("pointerup") && up.contains("touchend"), "{up}");
        assert!(!up.contains("__aqPick("), "{up}");
    }

    #[test]
    fn swipe_js_directions_and_viewport_default() {
        let el = build_swipe_js(Some(&DragEndpoint::Css(".card".into())), "left", 150.0);
        assert!(el.contains("__aqSwipe"), "{el}");
        assert!(el.contains("\".card\""), "{el}");
        assert!(el.contains("=== 'left'"), "{el}");
        assert!(el.contains("const D = 150"), "{el}");
        for evt in [
            "touchstart",
            "touchmove",
            "touchend",
            "pointerdown",
            "pointermove",
            "pointerup",
        ] {
            assert!(el.contains(evt), "{el}");
        }
        // viewport-origin swipe when `on` is absent
        let page = build_swipe_js(None, "up", 300.0);
        assert!(page.contains("elementFromPoint"), "{page}");
        assert!(!page.contains("el-miss"), "{page}");
        assert!(build_swipe_js(None, "diagonal", 100.0).contains("bad-dir"));
    }

    #[test]
    fn pinch_js_directions_and_viewport_default() {
        let el = build_pinch_js(Some(&DragEndpoint::Css("#map".into())), "in", 200.0);
        assert!(el.contains("__aqPinch"), "{el}");
        assert!(el.contains("=== 'in'"), "{el}");
        assert!(el.contains("const D = 200"), "{el}");
        assert!(el.contains("touchstart"), "{el}");
        assert!(el.contains("touchmove"), "{el}");
        assert!(el.contains("touchend"), "{el}");
        assert!(el.contains("[t1, t2]"), "{el}");
        assert!(el.contains("ctrlKey: true"), "{el}");
        assert!(el.contains("__aqGestureFind"), "{el}");

        let page = build_pinch_js(None, "out", 150.0);
        assert!(page.contains("elementFromPoint"), "{page}");
        assert!(page.contains("=== 'out'"), "{page}");
        assert!(build_pinch_js(None, "diagonal", 100.0).contains("bad-dir"));
    }

    #[test]
    fn rotate_js_emits_two_touch_orbit() {
        let el = build_rotate_js(Some(&DragEndpoint::Css("#dial".into())), 90.0, 60.0);
        assert!(el.contains("__aqRotate"), "{el}");
        assert!(el.contains("const R = 60"), "{el}");
        assert!(el.contains("RAD = 90 * Math.PI / 180"), "{el}");
        assert!(el.contains("touchstart"), "{el}");
        assert!(el.contains("touchmove"), "{el}");
        assert!(el.contains("touchend"), "{el}");
        assert!(el.contains("__aqGestureFind"), "{el}");
        assert!(el.contains("\"#dial\""), "{el}");

        let page = build_rotate_js(None, -45.0, 120.0);
        assert!(page.contains("elementFromPoint"), "{page}");
        assert!(page.contains("RAD = -45 * Math.PI / 180"), "{page}");
        // No desktop fallback: a Touch-less browser reports no-touch.
        assert!(page.contains("\"no-touch\""), "{page}");
    }
}
