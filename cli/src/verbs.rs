//! `do`-verb dispatcher for scenario/2 replay.
//!
//! Mirrors `replay/verbs.ts`. Each variant of [`crate::scenario::Verb`]
//! maps to a sequence of agent-browser CLI calls, with the verb-shape
//! rules in [`crate::verb_shape`] enforcing the do-step shape first.
//!
//! Locator resolution: for `Locator::Role { role, name, … }` we use
//! agent-browser's one-shot `find role <r> <act> --name <n>` form. For
//! `Locator::Raw { raw: { kind, value }, … }` we use native selector verbs
//! for css/testId and `find xpath` for XPath.
//! Tolerance / nested scope / heal land later.
//!
//! Implemented verbs:
//!   - implemented: goto, reload, back, forward, click, type, clear,
//!     press, hover, focus, blur, check, uncheck, wait
//!   - not yet implemented: (none of the scenario/2 do verbs)
//!     loop, group, useTemplate
//!
//! The runner consumes [`dispatch_do`] from this module — verb-shape
//! assertion happens inside.

use anyhow::{anyhow, bail, Result};
use serde_json::Value as Json;

use crate::browser::{self, RoleAct};
use crate::scenario::{Locator, NameMatch, RawLocatorKind, Step, Value, Verb};
use crate::value::{resolve_value, value_to_string, ValueScope};
use crate::verb_shape::assert_verb_shape;

#[derive(Debug, Clone)]
pub struct DoContext<'a> {
    pub session: &'a str,
}

/// Dispatch a single `do` step. Returns `Ok(Some(saved))` if the verb
/// produces a binding to record via `saveAs`; `Ok(None)` otherwise.
pub fn dispatch_do(step: &Step, ctx: &DoContext, scope: &mut ValueScope) -> Result<Option<Json>> {
    assert_verb_shape(step)?;
    let (id, verb, on, value, params) = match step {
        Step::Do {
            id,
            verb,
            on,
            value,
            params,
            ..
        } => (
            id.as_str(),
            verb,
            on.as_ref(),
            value.as_ref(),
            params.as_ref(),
        ),
        Step::Check { id, .. } => bail!("dispatch_do: step '{id}' is a check, not a do"),
    };
    match verb {
        Verb::Goto => {
            let url = resolve_literal_string(value, scope, "goto.value")?;
            browser::open(ctx.session, &url)?;
            Ok(None)
        }
        Verb::Reload => {
            browser::eval_expression(ctx.session, "(() => { location.reload(); })()")?;
            Ok(None)
        }
        Verb::Back => {
            browser::eval_expression(ctx.session, "(() => { history.back(); })()")?;
            Ok(None)
        }
        Verb::Forward => {
            browser::eval_expression(ctx.session, "(() => { history.forward(); })()")?;
            Ok(None)
        }
        Verb::Click => {
            act_on_locator(ctx.session, on.unwrap(), scope, RoleAct::Click, None)?;
            Ok(None)
        }
        Verb::Check | Verb::Uncheck => {
            // Checkbox toggle: agent-browser doesn't expose set-state
            // directly, so we click. Real state-aware behaviour lands
            // when a check claim follows this in a scenario.
            act_on_locator(ctx.session, on.unwrap(), scope, RoleAct::Click, None)?;
            Ok(None)
        }
        Verb::Hover => {
            act_on_locator(ctx.session, on.unwrap(), scope, RoleAct::Hover, None)?;
            Ok(None)
        }
        Verb::Focus => {
            act_on_locator(ctx.session, on.unwrap(), scope, RoleAct::Focus, None)?;
            Ok(None)
        }
        Verb::Blur => {
            browser::eval_expression(
                ctx.session,
                "(() => { if (document.activeElement) document.activeElement.blur(); })()",
            )?;
            Ok(None)
        }
        Verb::Type => {
            let v = resolve_value(value.unwrap(), scope)?;
            act_on_locator(
                ctx.session,
                on.unwrap(),
                scope,
                RoleAct::Fill,
                Some(&value_to_string(&v)),
            )?;
            Ok(None)
        }
        Verb::Clear => {
            act_on_locator(ctx.session, on.unwrap(), scope, RoleAct::Fill, Some(""))?;
            Ok(None)
        }
        Verb::Press => {
            let key = resolve_literal_string(value, scope, "press.value")?;
            browser::press_key(ctx.session, &key)?;
            Ok(None)
        }
        Verb::Wait => {
            // `params.ms` → wait by ms; `params.until` → wait --load <state>;
            // neither → soft wait for networkidle.
            let ms = params.and_then(|p| p.get("ms")).and_then(|v| v.as_u64());
            let until = params.and_then(|p| p.get("until")).and_then(|v| v.as_str());
            match (ms, until) {
                (Some(ms), _) => browser::wait_ms(ctx.session, ms)?,
                (_, Some(state)) => browser::wait_for_load(ctx.session, state)?,
                _ => browser::wait_for_load(ctx.session, "networkidle")?,
            }
            Ok(None)
        }
        Verb::CallGql => {
            let p = params.ok_or_else(|| anyhow!("step '{id}' callGql: params required"))?;
            let url = p
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("step '{id}' callGql: params.url is required"))?;
            let query = p
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("step '{id}' callGql: params.query is required"))?;
            let url = crate::value::substitute_scenario_vars(url, scope);
            let query = crate::value::substitute_scenario_vars(query, scope);
            let mut vars = serde_json::Map::new();
            if let Some(v) = p.get("variables").and_then(|v| v.as_object()) {
                for (k, val) in v {
                    vars.insert(k.clone(), val.clone());
                }
            }
            let response =
                crate::env_ops::post_gql_via_eval(ctx.session, &url, &query, &Json::Object(vars))?;
            // Bind into scope under the saveAs name when the step
            // declared one. dispatch_do's caller in runner.rs handles
            // this via the returned Some(value).
            Ok(Some(response))
        }
        Verb::ScrollTo => {
            scroll_to(ctx.session, on, scope)?;
            Ok(None)
        }
        Verb::Read => {
            let value = read_text(ctx.session, on.unwrap(), scope)?;
            Ok(Some(serde_json::Value::String(value)))
        }
        Verb::Select => {
            let v = resolve_value(value.unwrap(), scope)?;
            select_option(ctx.session, on.unwrap(), &value_to_string(&v), scope)?;
            Ok(None)
        }
        Verb::Group => {
            bail!(
                "step '{id}' verb=group should be flattened by the runner before dispatch_do is called"
            );
        }
        Verb::UseTemplate => {
            bail!(
                "step '{id}' verb=useTemplate should be flattened by the runner before dispatch_do is called"
            );
        }
        Verb::Loop => {
            bail!(
                "step '{id}' verb=loop should be flattened by the runner before dispatch_do is called"
            );
        }
        Verb::Upload => {
            let v = resolve_value(value.unwrap(), scope)?;
            let files =
                upload_files_from_value(&v).map_err(|e| anyhow!("step '{id}' upload: {e}"))?;
            let selector = upload_selector(on.unwrap(), scope)
                .map_err(|e| anyhow!("step '{id}' upload: {e}"))?;
            browser::upload(ctx.session, &selector, &files)?;
            Ok(None)
        }
    }
}

// ---------- helpers ----------

/// Set a `<select>` element's value via raw locator + dispatch a change
/// event so React / framework state stays in sync.
///
/// - Raw css / xpath / testId locators → native DOM mutation.
/// - Role + name → not yet (combobox-role flows want a snapshot-aware
///   click-then-pick interaction).
fn select_option(
    session: &str,
    loc: &Locator,
    value: &str,
    scope: &mut ValueScope,
) -> anyhow::Result<()> {
    use anyhow::bail;
    let value_lit = json_str(value);
    let body = |selector_lit: String| -> String {
        format!(
            "(() => {{ const el = document.querySelector({sel}); if (!el) return; el.value = {val}; el.dispatchEvent(new Event('input', {{ bubbles: true }})); el.dispatchEvent(new Event('change', {{ bubbles: true }})); }})()",
            sel = selector_lit,
            val = value_lit
        )
    };
    let expr = match loc {
        Locator::Raw(raw) => {
            let v = crate::value::substitute_scenario_vars(&raw.raw.value, scope);
            match raw.raw.kind {
                RawLocatorKind::Css => body(json_str(&v)),
                RawLocatorKind::TestId => {
                    let css = format!("[data-testid=\"{}\"]", v.replace('"', "\\\""));
                    body(json_str(&css))
                }
                RawLocatorKind::Xpath => format!(
                    "(() => {{ const r = document.evaluate({}, document, null, 9, null); const el = r && r.singleNodeValue; if (!el) return; el.value = {val}; el.dispatchEvent(new Event('input', {{ bubbles: true }})); el.dispatchEvent(new Event('change', {{ bubbles: true }})); }})()",
                    json_str(&v),
                    val = value_lit
                ),
                RawLocatorKind::Text => format!(
                    "(() => {{ const want = {q}; const el = [...document.querySelectorAll('input,select,textarea,[contenteditable=\"true\"]')].find(e => (e.labels && [...e.labels].some(l => (l.innerText||'').includes(want))) || (e.placeholder||'').includes(want) || (e.getAttribute('aria-label')||'').includes(want)); if (!el) return; el.value = {val}; el.dispatchEvent(new Event('input', {{ bubbles: true }})); el.dispatchEvent(new Event('change', {{ bubbles: true }})); }})()",
                    q = json_str(&v),
                    val = value_lit
                ),
            }
        }
        Locator::Role(_) => {
            bail!(
                "select with role+name locators is not yet supported (use raw.css or raw.xpath; combobox-role flows want a snapshot-aware click-then-pick interaction)"
            );
        }
    };
    browser::eval_expression(session, &expr)?;
    Ok(())
}

// ---------- end helpers ----------

/// Read element text via agent-browser eval.
///
/// - Raw css / xpath / testId locators → native DOM query, return
///   `(el.textContent ?? "").trim()`.
/// - Role + name → not yet supported (no ARIA-aware DOM query primitive
///   in the browser without a snapshot walker).
fn read_text(session: &str, loc: &Locator, scope: &mut ValueScope) -> anyhow::Result<String> {
    use anyhow::bail;
    let expr = match loc {
        Locator::Raw(raw) => {
            let v = crate::value::substitute_scenario_vars(&raw.raw.value, scope);
            match raw.raw.kind {
                RawLocatorKind::Css => format!(
                    "(() => {{ const el = document.querySelector({}); return el ? (el.textContent || '').trim() : ''; }})()",
                    json_str(&v)
                ),
                RawLocatorKind::TestId => {
                    let css = format!("[data-testid=\"{}\"]", v.replace('"', "\\\""));
                    format!(
                        "(() => {{ const el = document.querySelector({}); return el ? (el.textContent || '').trim() : ''; }})()",
                        json_str(&css)
                    )
                }
                RawLocatorKind::Xpath => format!(
                    "(() => {{ const r = document.evaluate({}, document, null, 9, null); const el = r && r.singleNodeValue; return el ? (el.textContent || '').trim() : ''; }})()",
                    json_str(&v)
                ),
                RawLocatorKind::Text => format!(
                    "(() => {{ const want = {q}; const el = [...document.querySelectorAll('*')].find(e => (e.innerText || e.textContent || '').includes(want)); return el ? (el.textContent || '').trim() : ''; }})()",
                    q = json_str(&v)
                ),
            }
        }
        Locator::Role(_) => {
            bail!("read with role+name locators is not yet supported (use raw.css or raw.xpath)");
        }
    };
    let raw = browser::eval_expression(session, &expr)?;
    // agent-browser eval double-encodes string returns; unwrap once.
    let trimmed = raw.trim();
    let unwrapped: String = serde_json::from_str(trimmed).unwrap_or_else(|_| trimmed.to_string());
    Ok(unwrapped)
}

// ---------- end helpers ----------

/// Scroll to a given target.
///
/// - No `on` → `window.scrollTo(0, 0)` (top of page).
/// - `on` Raw css → `document.querySelector(…).scrollIntoView()`.
/// - `on` Raw xpath → `document.evaluate(…).singleNodeValue.scrollIntoView()`.
/// - `on` Raw testId → synthesised `[data-testid=…]` CSS.
/// - `on` Role → not yet supported (needs ARIA-aware DOM traversal).
fn scroll_to(session: &str, on: Option<&Locator>, scope: &mut ValueScope) -> anyhow::Result<()> {
    use anyhow::bail;
    let expr = match on {
        None => "(() => { window.scrollTo(0, 0); })()".to_string(),
        Some(Locator::Raw(raw)) => {
            let v = crate::value::substitute_scenario_vars(&raw.raw.value, scope);
            match raw.raw.kind {
                RawLocatorKind::Css => format!(
                    "(() => {{ const el = document.querySelector({}); if (el) el.scrollIntoView(); }})()",
                    json_str(&v)
                ),
                RawLocatorKind::TestId => {
                    let css = format!("[data-testid=\"{}\"]", v.replace('"', "\\\""));
                    format!(
                        "(() => {{ const el = document.querySelector({}); if (el) el.scrollIntoView(); }})()",
                        json_str(&css)
                    )
                }
                RawLocatorKind::Xpath => format!(
                    "(() => {{ const r = document.evaluate({}, document, null, 9, null); if (r && r.singleNodeValue) r.singleNodeValue.scrollIntoView(); }})()",
                    json_str(&v)
                ),
                RawLocatorKind::Text => format!(
                    "(() => {{ const want = {q}; const el = [...document.querySelectorAll('*')].find(e => (e.innerText || e.textContent || '').includes(want)); if (el) el.scrollIntoView(); }})()",
                    q = json_str(&v)
                ),
            }
        }
        Some(Locator::Role(_)) => {
            bail!(
                "scrollTo with role+name locators is not yet supported (use raw.css or raw.xpath)"
            );
        }
    };
    browser::eval_expression(session, &expr)?;
    Ok(())
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("string serializes")
}

// ---------- end helpers ----------

fn resolve_literal_string(
    value: Option<&Value>,
    scope: &mut ValueScope,
    label: &str,
) -> Result<String> {
    let v = value.ok_or_else(|| anyhow!("{label}: missing"))?;
    let resolved = resolve_value(v, scope)?;
    Ok(value_to_string(&resolved))
}

/// Resolve an upload step's locator to a CSS-shaped selector that
/// `agent-browser upload` accepts. Role locators and xpath locators
/// are rejected — agent-browser's upload primitive only takes a CSS
/// selector or a snapshot `@ref`, neither of which we can synthesise
/// from those shapes here.
fn upload_selector(loc: &Locator, scope: &mut ValueScope) -> Result<String> {
    match loc {
        Locator::Role(_) => bail!(
            "role+name locators is not supported (agent-browser upload requires a css selector or @ref)"
        ),
        Locator::Raw(raw) => {
            let v = crate::value::substitute_scenario_vars(&raw.raw.value, scope);
            match &raw.raw.kind {
                RawLocatorKind::Css => Ok(v),
                RawLocatorKind::TestId => {
                    Ok(format!("[data-testid=\"{}\"]", v.replace('"', "\\\"")))
                }
                RawLocatorKind::Xpath => bail!(
                    "xpath locators are not supported by agent-browser upload (use css or testId)"
                ),
                RawLocatorKind::Text => bail!(
                    "text locators are not supported by agent-browser upload (use css or testId)"
                ),
            }
        }
    }
}

/// Coerce a resolved Value to a list of file paths.
/// - JSON string → single-element list
/// - JSON array of strings → list as-is
/// - empty string / empty array / other shapes → error
fn upload_files_from_value(v: &serde_json::Value) -> Result<Vec<String>> {
    let files: Vec<String> = match v {
        serde_json::Value::String(s) => {
            if s.is_empty() {
                vec![]
            } else {
                vec![s.clone()]
            }
        }
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, it) in items.iter().enumerate() {
                match it {
                    serde_json::Value::String(s) if !s.is_empty() => out.push(s.clone()),
                    serde_json::Value::String(_) => bail!("file path at index {i} is empty"),
                    other => bail!(
                        "file path at index {i} must be a string, got {}",
                        json_kind(other)
                    ),
                }
            }
            out
        }
        other => bail!(
            "value must be a string or string[] (got {})",
            json_kind(other)
        ),
    };
    if files.is_empty() {
        bail!("at least one file path is required");
    }
    Ok(files)
}

fn json_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Whether the role+name fallback to `find text` makes sense for this
/// action. `Focus` is excluded — agent-browser's text locator doesn't
/// support focus, and a presence assertion that fell through to text
/// would mask the original failure.
fn role_act_supports_text_fallback(act: RoleAct) -> bool {
    matches!(act, RoleAct::Click | RoleAct::Hover | RoleAct::Fill)
}

/// Try `agent-browser find text` with the recorded name verbatim, then
/// (if that misses) each whitespace-separated chunk of length ≥ 3. The
/// chunked retry covers the common snapshot-stitches-text-nodes-with-a
/// space case (e.g. snapshot prints `"Docs Vercel documentation"` for an
/// element whose accessible name is actually `"DocsVercel documentation"`,
/// where the second word `"Vercel"` or `"documentation"` is unique).
///
/// Returns true on first success.
fn try_text_fallback(session: &str, name: &str, act: RoleAct, value: Option<&str>) -> bool {
    if browser::find_text_act_quiet(session, name, act, value).is_ok() {
        return true;
    }
    for chunk in name.split_whitespace() {
        if chunk.len() < 3 {
            continue;
        }
        if browser::find_text_act_quiet(session, chunk, act, value).is_ok() {
            return true;
        }
    }
    false
}

/// Textboxes are commonly exposed in snapshots by placeholder or implicit
/// label, while agent-browser's role finder may only see explicit ARIA names.
/// When role=textbox+fill misses, recover through DOM fields whose visible
/// label/placeholder/aria/name/id matches the recorded name.
fn try_named_control_fill(session: &str, name: &str, value: &str) -> anyhow::Result<bool> {
    let expr = format!(
        r#"(() => {{
  const want = {name_lit}.toLowerCase();
  const value = {value_lit};
  const text = (s) => (s || '').trim().toLowerCase();
  const labelText = (el) => [
    ...(el.labels ? Array.from(el.labels).map(l => l.innerText || l.textContent || '') : []),
    el.id ? (document.querySelector(`label[for="${{CSS.escape(el.id)}}"]`)?.innerText || '') : '',
  ].join(' ');
  const fields = Array.from(document.querySelectorAll('input,textarea,[contenteditable="true"]'));
  const el = fields.find((field) => [
    labelText(field),
    field.getAttribute('placeholder'),
    field.getAttribute('aria-label'),
    field.getAttribute('name'),
    field.id,
  ].some((candidate) => text(candidate).includes(want)));
  if (!el) return false;
  if (el.isContentEditable) {{
    el.textContent = value;
  }} else {{
    const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
    if (setter) setter.call(el, value); else el.value = value;
  }}
  el.dispatchEvent(new InputEvent('input', {{ bubbles: true, inputType: 'insertText', data: value }}));
  el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  return true;
}})()"#,
        name_lit = json_str(name),
        value_lit = json_str(value)
    );
    let out = browser::eval_expression(session, &expr)?;
    Ok(out.trim() == "true")
}

/// Named buttons/links can appear in snapshots by input value or implicit
/// button text even when the role finder misses. Recover by matching common
/// name-bearing attributes and invoking native click/submit behaviour.
fn try_named_control_click(session: &str, role: &str, name: &str) -> anyhow::Result<bool> {
    if role != "button" && role != "link" {
        return Ok(false);
    }
    let selector = if role == "link" {
        "a,[role=link]"
    } else {
        "button,input[type=button],input[type=submit],input[type=reset],[role=button]"
    };
    let expr = format!(
        r#"(() => {{
  const want = {name_lit}.toLowerCase();
  const text = (s) => (s || '').trim().toLowerCase();
  const candidates = Array.from(document.querySelectorAll({selector_lit}));
  const primary = (node) => [
    node.innerText,
    node.textContent,
    node.value,
    node.getAttribute('aria-label'),
    node.getAttribute('name'),
  ].some((candidate) => text(candidate) === want);
  const secondary = (node) => want.length >= 3 && [
    node.innerText,
    node.textContent,
    node.value,
    node.getAttribute('aria-label'),
    node.getAttribute('name'),
    node.id,
    node.getAttribute('data-test'),
    node.getAttribute('data-testid'),
  ].some((candidate) => text(candidate).includes(want));
  const el = candidates.find(primary) || candidates.find(secondary);
  if (!el) return false;
  const form = el.form || el.closest('form');
  const isSubmit = el.tagName === 'BUTTON' || (el.tagName === 'INPUT' && ['submit', 'button', 'reset'].includes((el.type || '').toLowerCase()));
  if (form && isSubmit && typeof form.requestSubmit === 'function') {{
    form.requestSubmit(el);
  }} else {{
    el.dispatchEvent(new MouseEvent('mousedown', {{ bubbles: true, cancelable: true, view: window }}));
    el.dispatchEvent(new MouseEvent('mouseup', {{ bubbles: true, cancelable: true, view: window }}));
    el.click();
  }}
  return true;
}})()"#,
        name_lit = json_str(name),
        selector_lit = json_str(selector)
    );
    let out = browser::eval_expression(session, &expr)?;
    Ok(out.trim() == "true")
}

/// agent-browser's top-level `click <selector>` can report success for a
/// submit button without dispatching the form's submit path. Prefer the
/// browser-native mouse/click sequence for native controls, then let the
/// caller fall back to agent-browser for ordinary DOM targets.
fn try_selector_native_click(session: &str, selector: &str) -> anyhow::Result<bool> {
    let expr = format!(
        r#"(() => {{
  const el = document.querySelector({selector_lit});
  if (!el) return false;
  const tag = el.tagName;
  const type = (el.getAttribute('type') || '').toLowerCase();
  const role = (el.getAttribute('role') || '').toLowerCase();
  const isNativeControl = tag === 'BUTTON'
    || tag === 'A'
    || (tag === 'INPUT' && ['submit', 'button', 'reset', 'checkbox', 'radio'].includes(type))
    || ['button', 'checkbox', 'radio', 'link'].includes(role);
  if (!isNativeControl) return false;
  el.dispatchEvent(new MouseEvent('mousedown', {{ bubbles: true, cancelable: true, view: window }}));
  el.dispatchEvent(new MouseEvent('mouseup', {{ bubbles: true, cancelable: true, view: window }}));
  el.click();
  return true;
}})()"#,
        selector_lit = json_str(selector)
    );
    let out = browser::eval_expression(session, &expr)?;
    Ok(out.trim() == "true")
}

/// Final replay-side fallback: snapshot the page, find
/// `<role> "<name>" [ref=eN]`, click `@eN`. Same rationale as
/// `smart_click::try_snapshot_fallback` — see that for the why.
/// Currently only wired for click (the only verb where a backend-node
/// click is meaningful without an action argument).
fn try_snapshot_fallback(session: &str, role: &str, name: &str) -> anyhow::Result<()> {
    let snap = browser::snapshot_full(session)
        .map_err(|e| anyhow!("snapshot for ref-based fallback: {e}"))?;
    let r = browser::find_ref_in_snapshot(&snap, role, name)
        .ok_or_else(|| anyhow!("snapshot has no `{role} \"{name}\" [ref=...]` line"))?;
    browser::click_ref(session, &r).map_err(|e| anyhow!("click @{r}: {e}"))?;
    Ok(())
}

fn act_on_locator(
    session: &str,
    loc: &Locator,
    scope: &mut ValueScope,
    act: RoleAct,
    value: Option<&str>,
) -> Result<()> {
    match loc {
        Locator::Role(role) => {
            // Top-level scope handling is deferred; recorder typically
            // emits an empty scope. Nested `scope` arrays are dropped
            // here with a noted limitation.
            if let Some(inner) = &role.scope {
                if !inner.is_empty() {
                    eprintln!(
                        "[v2-replay] note: locator.scope is not yet honoured (got {} nested entries)",
                        inner.len()
                    );
                }
            }
            let name = match &role.name {
                Some(NameMatch::Plain(s)) => Some(crate::value::substitute_scenario_vars(s, scope)),
                Some(NameMatch::Pattern { pattern, .. }) => {
                    Some(crate::value::substitute_scenario_vars(pattern, scope))
                }
                Some(NameMatch::I18n { i18n_key }) => {
                    bail!("locator.name.i18nKey ({i18n_key:?}) is not yet supported")
                }
                None => None,
            };
            let name_str = name.as_deref().unwrap_or("");
            // agent-browser's find requires --name when matching by
            // accessible name; without a name we fall back to role-only
            // (first match).
            //
            // Probe role+name quietly so a recovering miss doesn't spam
            // the user with a `✗ Element not found` line they have to
            // mentally discard.
            if matches!(act, RoleAct::Click)
                && !name_str.is_empty()
                && try_named_control_click(session, &role.role, name_str)?
            {
                eprintln!(
                    "[v2-replay] role='{}' name='{}' activated via named control click",
                    role.role, name_str
                );
                return Ok(());
            }

            match browser::find_role_act_quiet(session, &role.role, name_str, act, value) {
                Ok(()) => Ok(()),
                Err(e) if !name_str.is_empty() && role_act_supports_text_fallback(act) => {
                    // Fallback ladder: text → chunked text → snapshot ref.
                    // See [`crate::browser::find_ref_in_snapshot`] for the
                    // snapshot rationale.
                    if matches!(act, RoleAct::Fill)
                        && role.role == "textbox"
                        && try_named_control_fill(session, name_str, value.unwrap_or(""))?
                    {
                        eprintln!(
                            "[v2-replay] role+name miss for role='{}' name='{}' recovered via named control fill",
                            role.role, name_str
                        );
                        Ok(())
                    } else if try_text_fallback(session, name_str, act, value) {
                        eprintln!(
                            "[v2-replay] role+name miss for role='{}' name='{}' recovered via text locator",
                            role.role, name_str
                        );
                        Ok(())
                    } else if matches!(act, RoleAct::Click)
                        && try_snapshot_fallback(session, &role.role, name_str).is_ok()
                    {
                        eprintln!(
                            "[v2-replay] role+name miss for role='{}' name='{}' recovered via snapshot ref",
                            role.role, name_str
                        );
                        Ok(())
                    } else {
                        Err(e.into())
                    }
                }
                Err(e) => Err(e.into()),
            }
        }
        Locator::Raw(raw) => {
            let v = crate::value::substitute_scenario_vars(&raw.raw.value, scope);
            match &raw.raw.kind {
                RawLocatorKind::Css => {
                    if matches!(act, RoleAct::Click) && try_selector_native_click(session, &v)? {
                        eprintln!(
                            "[v2-replay] css selector '{}' activated via native DOM click",
                            v
                        );
                    } else {
                        browser::selector_act(session, &v, act, value)?;
                    }
                }
                RawLocatorKind::Xpath => {
                    browser::find_xpath_act(session, &v, act, value)?;
                }
                RawLocatorKind::TestId => {
                    let css = format!("[data-testid=\"{}\"]", v.replace('"', "\\\""));
                    if matches!(act, RoleAct::Click) && try_selector_native_click(session, &css)? {
                        eprintln!("[v2-replay] testId '{}' activated via native DOM click", v);
                    } else {
                        browser::selector_act(session, &css, act, value)?;
                    }
                }
                RawLocatorKind::Text => {
                    // agent-browser's `find text` doesn't support focus; for
                    // focus we fall back to a DOM-eval click on the first
                    // text-matching element. Click/hover/fill go through the
                    // native locator which has scroll/visibility heuristics.
                    if matches!(act, RoleAct::Focus) {
                        let expr = format!(
                            "(() => {{ const want = {q}; const all = [...document.querySelectorAll('a,button,[role]')]; const hit = all.find(el => (el.innerText || el.textContent || '').includes(want)); if (hit) hit.focus(); }})()",
                            q = json_str(&v)
                        );
                        browser::eval_expression(session, &expr)?;
                    } else {
                        browser::find_text_act(session, &v, act, value)?;
                    }
                }
            }
            Ok(())
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    use crate::browser as ab;
    use crate::test_util::lock_env;

    fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn install_fake(dir: &Path, log: &Path) {
        let body = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let bin = write_exec(dir, "agent-browser", &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn install_fake_eval_true(dir: &Path, log: &Path) {
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nif [ \"$3\" = eval ]; then printf true; fi\nexit 0\n",
            log.display()
        );
        let bin = write_exec(dir, "agent-browser", &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn clear_fake() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    fn parse(j: serde_json::Value) -> Step {
        serde_json::from_value(j).unwrap()
    }

    fn run_one(s: &Step) -> String {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        install_fake(tmp.path(), &log);
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let _out = dispatch_do(s, &ctx, &mut scope).unwrap();
        let lines = fs::read_to_string(&log).unwrap();
        clear_fake();
        lines
    }

    #[test]
    fn goto_invokes_open() {
        let s = parse(json!({
            "id": "s1", "intent": "go", "kind": "do", "verb": "goto",
            "value": { "from": "literal", "literal": "https://example.com/" }
        }));
        let out = run_one(&s);
        assert!(
            out.contains("--session sess open https://example.com/"),
            "got: {out}"
        );
    }

    #[test]
    fn click_prefers_named_control_activation() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        install_fake_eval_true(tmp.path(), &log);
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "click",
            "on": { "role": "button", "name": "Save" }
        }));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        dispatch_do(&s, &ctx, &mut scope).unwrap();

        let out = fs::read_to_string(&log).unwrap();
        clear_fake();
        assert!(out.contains("--session sess eval"), "got: {out}");
        assert!(
            !out.contains("find role button click --name Save"),
            "got: {out}"
        );
    }

    #[test]
    fn click_role_button_uses_named_control_click_before_find_role() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nif [ \"$3\" = find ]; then exit 1; fi\nif [ \"$3\" = eval ]; then printf true; exit 0; fi\nexit 0\n",
            log.display()
        );
        let bin = write_exec(tmp.path(), "agent-browser", &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "click",
            "on": { "role": "button", "name": "Login" }
        }));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        dispatch_do(&s, &ctx, &mut scope).unwrap();

        let out = fs::read_to_string(&log).unwrap();
        clear_fake();
        assert!(out.contains("--session sess eval"), "got: {out}");
        assert!(out.contains("requestSubmit"), "got: {out}");
        assert!(
            !out.contains("find role button click --name Login"),
            "got: {out}"
        );
    }

    #[test]
    fn type_resolves_literal_value_and_calls_find_fill() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "type",
            "on": { "role": "textbox", "name": "Email" },
            "value": { "from": "literal", "literal": "a@b" }
        }));
        let out = run_one(&s);
        assert!(
            out.contains("--session sess find role textbox fill --name Email a@b"),
            "got: {out}"
        );
    }

    #[test]
    fn type_role_textbox_miss_falls_back_to_named_control_fill() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nif [ \"$3\" = find ]; then exit 1; fi\nif [ \"$3\" = eval ]; then printf true; exit 0; fi\nexit 0\n",
            log.display()
        );
        let bin = write_exec(tmp.path(), "agent-browser", &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "type",
            "on": { "role": "textbox", "name": "Username" },
            "value": { "from": "literal", "literal": "standard_user" }
        }));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        dispatch_do(&s, &ctx, &mut scope).unwrap();

        let out = fs::read_to_string(&log).unwrap();
        clear_fake();
        assert!(
            out.contains("--session sess find role textbox fill --name Username standard_user"),
            "got: {out}"
        );
        assert!(out.contains("--session sess eval"), "got: {out}");
        assert!(out.contains("placeholder"), "got: {out}");
    }

    #[test]
    fn press_invokes_press() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "press",
            "value": { "from": "literal", "literal": "Enter" }
        }));
        let out = run_one(&s);
        assert!(out.contains("--session sess press Enter"), "got: {out}");
    }

    #[test]
    fn back_invokes_eval_history_back() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "back" }));
        let out = run_one(&s);
        assert!(out.contains("--session sess eval"), "got: {out}");
        assert!(out.contains("history.back()"), "got: {out}");
    }

    #[test]
    fn wait_with_ms_param() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "wait",
            "params": { "ms": 250 }
        }));
        let out = run_one(&s);
        assert!(out.contains("--session sess wait 250"), "got: {out}");
    }

    #[test]
    fn wait_with_until_load_state() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "wait",
            "params": { "until": "load" }
        }));
        let out = run_one(&s);
        assert!(
            out.contains("--session sess wait --load load"),
            "got: {out}"
        );
    }

    #[test]
    fn raw_css_locator_uses_native_selector_click() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "click",
            "on": { "raw": { "kind": "css", "value": "button.save" }, "reason": "no aria role" }
        }));
        let out = run_one(&s);
        assert!(
            out.contains("--session sess click button.save"),
            "got: {out}"
        );
    }

    #[test]
    fn raw_css_click_prefers_native_dom_click_when_selector_resolves() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        install_fake_eval_true(tmp.path(), &log);
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "click",
            "on": { "raw": { "kind": "css", "value": "button[type=submit]" }, "reason": "no aria role" }
        }));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        dispatch_do(&s, &ctx, &mut scope).unwrap();

        let out = fs::read_to_string(&log).unwrap();
        clear_fake();
        assert!(out.contains("--session sess eval"), "got: {out}");
        assert!(out.contains("MouseEvent"), "got: {out}");
        assert!(!out.contains("click button[type=submit]"), "got: {out}");
    }

    #[test]
    fn raw_testid_locator_synthesises_css() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "click",
            "on": { "raw": { "kind": "testId", "value": "save-btn" }, "reason": "no aria role" }
        }));
        let out = run_one(&s);
        assert!(
            out.contains("click [data-testid=\"save-btn\"]"),
            "got: {out}"
        );
    }

    #[test]
    fn scrollto_no_locator_scrolls_to_top() {
        let s = parse(json!({ "id": "s1", "intent": "top", "kind": "do", "verb": "scrollTo" }));
        let out = run_one(&s);
        assert!(out.contains("--session sess eval"), "got: {out}");
        assert!(out.contains("window.scrollTo(0, 0)"), "got: {out}");
    }

    #[test]
    fn scrollto_raw_css_scrolls_into_view() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "scrollTo",
            "on": { "raw": { "kind": "css", "value": ".target" }, "reason": "deep DOM" }
        }));
        let out = run_one(&s);
        assert!(out.contains("document.querySelector"), "got: {out}");
        assert!(out.contains(".target"), "got: {out}");
        assert!(out.contains("scrollIntoView"), "got: {out}");
    }

    #[test]
    fn scrollto_raw_xpath_uses_evaluate() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "scrollTo",
            "on": { "raw": { "kind": "xpath", "value": "//div[@id='x']" }, "reason": "" }
        }));
        let out = run_one(&s);
        assert!(out.contains("document.evaluate"), "got: {out}");
        assert!(out.contains("//div[@id='x']"), "got: {out}");
    }

    #[test]
    fn scrollto_role_locator_errors() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "scrollTo",
            "on": { "role": "button", "name": "Save" }
        }));
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(tmp.path(), &tmp.path().join("ab.log"));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let err = dispatch_do(&s, &ctx, &mut scope).unwrap_err().to_string();
        clear_fake();
        assert!(
            err.contains("role+name locators is not yet supported"),
            "got: {err}"
        );
    }

    #[test]
    fn read_raw_css_returns_element_text() {
        // Fake browser whose eval emits a JSON-string return
        // (matches agent-browser's wire format).
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let resp_path = tmp.path().join("resp.txt");
        std::fs::write(&resp_path, serde_json::to_string("hello world").unwrap()).unwrap();
        let body = format!(
            "#!/bin/sh\nif [ \"$3\" = eval ]; then cat '{}'; exit 0; fi\nexit 0\n",
            resp_path.display()
        );
        let bin = write_exec(tmp.path(), "agent-browser", &body);
        std::env::set_var(crate::browser::BIN_ENV, &bin);
        crate::browser::_reset_bin_cache_for_tests();

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "read",
            "on": { "raw": { "kind": "css", "value": ".banner" }, "reason": "" }
        }));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let saved = dispatch_do(&s, &ctx, &mut scope).unwrap();
        assert_eq!(saved, Some(serde_json::Value::String("hello world".into())));

        std::env::remove_var(crate::browser::BIN_ENV);
        crate::browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn read_role_locator_errors() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "read",
            "on": { "role": "heading", "name": "Welcome" }
        }));
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(tmp.path(), &tmp.path().join("ab.log"));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let err = dispatch_do(&s, &ctx, &mut scope).unwrap_err().to_string();
        clear_fake();
        assert!(
            err.contains("role+name locators is not yet supported"),
            "got: {err}"
        );
    }

    #[test]
    fn callgql_posts_via_eval_and_returns_response() {
        // Fake browser whose eval emits the canonical agent-browser
        // double-encoded envelope (string-of-JSON).
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let resp_path = tmp.path().join("resp.txt");
        // Outer level is a JSON string containing the inner envelope
        // {"status":200,"body":"<gql-body-as-string>"}
        let inner_envelope = r#"{"status":200,"body":"{\"data\":{\"x\":1}}"}"#;
        let outer = serde_json::to_string(inner_envelope).unwrap();
        std::fs::write(&resp_path, outer).unwrap();
        let body = format!(
            "#!/bin/sh\nif [ \"$3\" = eval ]; then cat '{}'; exit 0; fi\nexit 0\n",
            resp_path.display()
        );
        let bin = write_exec(tmp.path(), "agent-browser", &body);
        std::env::set_var(crate::browser::BIN_ENV, &bin);
        crate::browser::_reset_bin_cache_for_tests();

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "callGql",
            "params": { "url": "https://api/graphql", "query": "query A { x }" }
        }));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let saved = dispatch_do(&s, &ctx, &mut scope).unwrap();
        // Returned value is the parsed body; data.x == 1.
        let saved = saved.expect("callGql returns Some(response)");
        assert_eq!(saved["data"]["x"], 1);

        std::env::remove_var(crate::browser::BIN_ENV);
        crate::browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn select_raw_css_sets_value_and_fires_events() {
        let s = parse(json!({
            "id": "s1", "intent": "pick", "kind": "do", "verb": "select",
            "on": { "raw": { "kind": "css", "value": "select#country" }, "reason": "native select" },
            "value": { "from": "literal", "literal": "US" }
        }));
        let out = run_one(&s);
        assert!(out.contains("select#country"), "got: {out}");
        assert!(out.contains("el.value = \"US\""), "got: {out}");
        assert!(out.contains("new Event('change'"), "got: {out}");
    }

    #[test]
    fn select_role_locator_errors() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "select",
            "on": { "role": "combobox", "name": "Country" },
            "value": { "from": "literal", "literal": "US" }
        }));
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(tmp.path(), &tmp.path().join("ab.log"));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let err = dispatch_do(&s, &ctx, &mut scope).unwrap_err().to_string();
        clear_fake();
        assert!(
            err.contains("role+name locators is not yet supported"),
            "got: {err}"
        );
    }

    #[test]
    fn upload_raw_css_dispatches_files() {
        let s = parse(json!({
            "id": "s1", "intent": "attach", "kind": "do", "verb": "upload",
            "on": { "raw": { "kind": "css", "value": "input#file" }, "reason": "" },
            "value": { "from": "literal", "literal": "/tmp/a.pdf" }
        }));
        let out = run_one(&s);
        assert!(out.contains("upload input#file /tmp/a.pdf"), "got: {out}");
    }

    #[test]
    fn upload_testid_locator_translates_to_attr_selector() {
        let s = parse(json!({
            "id": "s1", "intent": "attach", "kind": "do", "verb": "upload",
            "on": { "raw": { "kind": "testId", "value": "file-input" }, "reason": "" },
            "value": { "from": "literal", "literal": "/tmp/a.pdf" }
        }));
        let out = run_one(&s);
        assert!(
            out.contains("upload [data-testid=\"file-input\"] /tmp/a.pdf"),
            "got: {out}"
        );
    }

    #[test]
    fn upload_array_value_passes_multiple_files() {
        let s = parse(json!({
            "id": "s1", "intent": "attach", "kind": "do", "verb": "upload",
            "on": { "raw": { "kind": "css", "value": "input#file" }, "reason": "" },
            "value": { "from": "literal", "literal": ["/tmp/a.pdf", "/tmp/b.png"] }
        }));
        let out = run_one(&s);
        assert!(
            out.contains("upload input#file /tmp/a.pdf /tmp/b.png"),
            "got: {out}"
        );
    }

    #[test]
    fn upload_role_locator_errors() {
        let s = parse(json!({
            "id": "s1", "intent": "attach", "kind": "do", "verb": "upload",
            "on": { "role": "button", "name": "Upload" },
            "value": { "from": "literal", "literal": "/tmp/a.pdf" }
        }));
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(tmp.path(), &tmp.path().join("ab.log"));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let err = dispatch_do(&s, &ctx, &mut scope).unwrap_err().to_string();
        clear_fake();
        assert!(
            err.contains("role+name locators is not supported"),
            "got: {err}"
        );
    }

    #[test]
    fn upload_xpath_locator_errors() {
        let s = parse(json!({
            "id": "s1", "intent": "attach", "kind": "do", "verb": "upload",
            "on": { "raw": { "kind": "xpath", "value": "//input" }, "reason": "" },
            "value": { "from": "literal", "literal": "/tmp/a.pdf" }
        }));
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(tmp.path(), &tmp.path().join("ab.log"));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let err = dispatch_do(&s, &ctx, &mut scope).unwrap_err().to_string();
        clear_fake();
        assert!(
            err.contains("xpath locators are not supported"),
            "got: {err}"
        );
    }

    #[test]
    fn upload_empty_value_errors() {
        let s = parse(json!({
            "id": "s1", "intent": "attach", "kind": "do", "verb": "upload",
            "on": { "raw": { "kind": "css", "value": "input#file" }, "reason": "" },
            "value": { "from": "literal", "literal": [] }
        }));
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(tmp.path(), &tmp.path().join("ab.log"));
        let ctx = DoContext { session: "sess" };
        let mut scope = ValueScope::default();
        let err = dispatch_do(&s, &ctx, &mut scope).unwrap_err().to_string();
        clear_fake();
        assert!(err.contains("at least one file path"), "got: {err}");
    }

    #[test]
    fn type_substitutes_vars_unique_in_literal_template() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "type",
            "on": { "role": "textbox", "name": "Email" },
            "value": { "from": "literal", "literal": "qa-{{vars._unique}}@example.com" }
        }));
        let out = run_one(&s);
        let re =
            regex::Regex::new(r"find role textbox fill --name Email qa-[a-f0-9]{8}@example.com")
                .unwrap();
        assert!(re.is_match(&out), "got: {out}");
    }
}
