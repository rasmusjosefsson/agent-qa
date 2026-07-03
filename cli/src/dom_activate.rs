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
//!   2. **mousedown-bound handlers** — MUI Select / React-Select / many menu
//!      components open on `mousedown`, not `click`, so a synthetic click is
//!      silently swallowed even when it lands on the right node.
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
    const type = (el.getAttribute && (el.getAttribute('type') || '') || '').toLowerCase();
    const isSubmit = (el.tagName === 'BUTTON' && type !== 'button' && type !== 'reset')
      || (el.tagName === 'INPUT' && type === 'submit');
    const form = el.form || (el.closest ? el.closest('form') : null);
    if (isSubmit && form && typeof form.requestSubmit === 'function') {
      form.requestSubmit(el);
      return true;
    }
    const opts = { bubbles: true, cancelable: true, view: window };
    try { el.dispatchEvent(new PointerEvent('pointerdown', opts)); } catch (e) {}
    el.dispatchEvent(new MouseEvent('mousedown', opts));
    try { el.dispatchEvent(new PointerEvent('pointerup', opts)); } catch (e) {}
    el.dispatchEvent(new MouseEvent('mouseup', opts));
    if (typeof el.click === 'function') { el.click(); } else { el.dispatchEvent(new MouseEvent('click', opts)); }
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
  const map = {{
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
  }};
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
    )
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
    fn popup_probe_counts_surfaces() {
        let js = build_popup_probe();
        assert!(js.contains("role=listbox"));
        assert!(js.contains("role=option"));
        assert!(js.contains("aria-expanded=\\\"true\\\"") || js.contains("aria-expanded=\"true\""));
    }
}

