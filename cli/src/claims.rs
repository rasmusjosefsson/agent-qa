//! scenario/2 `check`-step Claim evaluator.
//!
//! Claim = `{ subject, predicate, value?, tolerance? }`. Subject arms:
//!
//!   - `element` — `{ element: Locator, attribute?, ofKind? }`
//!   - `url`     — `{ url: true }`
//!   - `network` — `{ network: NetworkMatcher, ofKind?, path? }`
//!   - `data`    — `{ data: <savedName>, path? }`
//!   - `flag`    — `{ flag: <name> }`
//!   - `var`     — `{ kind: 'var', name: <savedName>, path? }`
//!
//! Scope:
//!   - `element`: `isVisible` / `exists` (locator resolves) and
//!     `isHidden` / `notExists` (locator stops resolving) with timeout
//!     polling. Other element predicates raise not-yet-implemented.
//!   - `url`: `equals`, `matches`, `contains`, `startsWith`, `endsWith`
//!     against both `location.href` and `location.pathname` with
//!     polling (either matches).
//!   - `data` / `var`: `exists`, `notExists`, `equals`, `contains`,
//!     `matches`, `startsWith`, `endsWith`.
//!   - `network` / `flag`: structured not-yet-implemented boundary.

use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use regex::Regex;
use serde_json::Value as Json;

use crate::browser::{self, RoleAct};
use crate::scenario::{Claim, ClaimSubject, Locator, NameMatch, Predicate, RawLocatorKind};
use crate::value::{select_json_path, substitute_scenario_vars, value_to_string, ValueScope};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const MAX_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct CheckContext<'a> {
    pub session: &'a str,
}

pub fn dispatch_check(
    claim: &Claim,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Option<Duration>,
) -> Result<()> {
    let timeout = timeout.unwrap_or(DEFAULT_TIMEOUT).min(MAX_TIMEOUT);
    match &claim.subject {
        ClaimSubject::Element {
            element,
            attribute,
            of_kind,
        } => {
            if attribute.is_some() || of_kind.is_some() {
                bail!("element claim with attribute/ofKind is not yet supported");
            }
            check_element(element, &claim.predicate, ctx, scope, timeout)
        }
        ClaimSubject::Url { url: _ } => {
            check_url(&claim.predicate, claim.value.as_ref(), ctx, scope, timeout)
        }
        ClaimSubject::Data { data, path } => {
            let actual = read_saved(scope, data, path.as_deref())?;
            check_value(&actual, &claim.predicate, claim.value.as_ref(), scope)
        }
        ClaimSubject::Var { kind, name, path } => {
            if kind != "var" {
                bail!("var subject requires kind='var', got {kind:?}");
            }
            let actual = read_saved(scope, name, path.as_deref())?;
            check_value(&actual, &claim.predicate, claim.value.as_ref(), scope)
        }
        ClaimSubject::Network { .. } => {
            bail!("network claim subject is not yet implemented")
        }
        ClaimSubject::Flag { flag } => {
            check_flag(flag, &claim.predicate, claim.value.as_ref(), ctx)
        }
    }
}

// ---------- flag ----------

/// Check a flag value stored in `localStorage['devtools-flag-overrides']`
/// (the same key written by [`crate::env_ops::EnvOp::Flag`]). The map
/// shape matches: `{ "<flag>": true|false, … }`.
///
/// Predicates supported:
///   `equals`              expected `value` is true / false
///   `exists`              flag key is present (boolean true OR false)
///   `notExists`           flag key absent
///   `isVisible`           alias for `equals true`
///   `isHidden`            alias for `equals false`
fn check_flag(
    flag: &str,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
) -> Result<()> {
    let raw = browser::eval_expression(
        ctx.session,
        "(() => { try { return JSON.parse(localStorage.getItem('devtools-flag-overrides') || '{}'); } catch { return {}; } })()",
    )?;
    // agent-browser eval double-encodes; the IIFE returns a real object
    // so we get one level of JSON.
    let parsed: Json =
        serde_json::from_str(raw.trim()).unwrap_or(Json::Object(serde_json::Map::new()));
    let entry = parsed.get(flag).cloned();
    match predicate {
        Predicate::Exists => {
            if entry.is_none() {
                bail!("expected flag {flag:?} to exist in localStorage devtools-flag-overrides");
            }
            Ok(())
        }
        Predicate::NotExists => {
            if entry.is_some() {
                bail!("expected flag {flag:?} to be absent, got {:?}", entry);
            }
            Ok(())
        }
        Predicate::IsVisible | Predicate::IsHidden | Predicate::Equals => {
            let want_true = match predicate {
                Predicate::IsVisible => true,
                Predicate::IsHidden => false,
                Predicate::Equals => match expected {
                    Some(Json::Bool(b)) => *b,
                    other => bail!(
                        "flag claim with predicate equals requires boolean value, got {other:?}"
                    ),
                },
                _ => unreachable!(),
            };
            let actual = entry
                .as_ref()
                .and_then(|v| v.as_bool())
                .ok_or_else(|| {
                    anyhow!(
                        "flag {flag:?} not present (or non-boolean) in localStorage devtools-flag-overrides"
                    )
                })?;
            if actual != want_true {
                bail!("expected flag {flag:?} = {want_true}, got {actual}");
            }
            Ok(())
        }
        other => bail!("flag subject does not support predicate '{other:?}'"),
    }
}

// ---------- element ----------

fn check_element(
    loc: &Locator,
    predicate: &Predicate,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    match predicate {
        Predicate::IsVisible | Predicate::Exists => {
            poll_until(timeout, |_| locator_resolves(ctx.session, loc, scope))
        }
        Predicate::IsHidden | Predicate::NotExists => {
            // Inverse: poll until the locator no longer resolves.
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                match locator_resolves(ctx.session, loc, scope) {
                    Ok(()) => thread::sleep(POLL_INTERVAL),
                    Err(_) => return Ok(()),
                }
            }
            bail!("expected {predicate:?}, but element still present");
        }
        other => bail!("element subject does not yet support predicate '{other:?}'"),
    }
}

fn locator_resolves(session: &str, loc: &Locator, scope: &mut ValueScope) -> Result<()> {
    // Re-use the same locator → CLI mapping as dispatch_do uses for
    // its act calls; here we use Focus (the cheapest no-op-ish act
    // agent-browser exposes) just to confirm the element resolves.
    // If/when agent-browser grows a dedicated `find ... exists` or
    // `query` subverb, switch to that.
    match loc {
        Locator::Role(role) => {
            let name = match &role.name {
                Some(NameMatch::Plain(s)) => Some(substitute_scenario_vars(s, scope)),
                Some(NameMatch::Pattern { pattern, .. }) => {
                    Some(substitute_scenario_vars(pattern, scope))
                }
                Some(NameMatch::I18n { i18n_key }) => {
                    bail!("locator.name.i18nKey ({i18n_key:?}) is not yet supported")
                }
                None => None,
            };
            let name_str = name.as_deref().unwrap_or("");
            // Probe role+name quietly so a recovering miss doesn't print a
            // misleading `✗ Element not found` line.
            match browser::find_role_act_quiet(session, &role.role, name_str, RoleAct::Focus, None)
            {
                Ok(()) => Ok(()),
                Err(e) if !name_str.is_empty() => {
                    // Same parity hack as verbs.rs::act_on_locator: agent-browser's
                    // role+name engine misses elements inside Radix portals etc.
                    // that the snapshot reports with role+name. We fall back to
                    // a DOM-eval probe by visible text — throws if no element
                    // contains the recorded name (or any of its space-split
                    // chunks of length ≥ 3, to handle the snapshot-stitches-text
                    // -nodes case). If text also misses, parse the snapshot for
                    // `<role> "<name>" [ref=eN]` as the final rung; presence in
                    // the snapshot proves the element exists even if neither
                    // role-engine nor text-engine can locate it.
                    if text_presence_probe(session, name_str).is_ok() {
                        eprintln!(
                            "[v2-replay] role+name miss for role='{}' name='{}' recovered via text presence probe",
                            role.role, name_str
                        );
                        Ok(())
                    } else if snapshot_presence_probe(session, &role.role, name_str).is_ok() {
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
            let v = substitute_scenario_vars(&raw.raw.value, scope);
            match &raw.raw.kind {
                RawLocatorKind::Css => {
                    browser::find_css_act(session, &v, RoleAct::Focus, None)?;
                }
                RawLocatorKind::Xpath => {
                    browser::find_xpath_act(session, &v, RoleAct::Focus, None)?;
                }
                RawLocatorKind::TestId => {
                    let css = format!("[data-testid=\"{}\"]", v.replace('"', "\\\""));
                    browser::find_css_act(session, &css, RoleAct::Focus, None)?;
                }
                RawLocatorKind::Text => {
                    // `find text` in agent-browser doesn't support focus.
                    // Synthesise a presence probe via in-page eval: throws
                    // when no element with the text exists, so the
                    // surrounding poll_until reads the same signal as a
                    // failed find.
                    let expr = format!(
                        "(() => {{ const want = {q}; const hit = [...document.querySelectorAll('*')].find(el => (el.innerText || el.textContent || '').includes(want)); if (!hit) throw new Error('text not found: ' + want); }})()",
                        q = serde_json::to_string(&v).expect("string serializes")
                    );
                    browser::eval_expression(session, &expr)?;
                }
            }
            Ok(())
        }
    }
}

/// Presence probe by visible text. Tries the full name first, then each
/// whitespace-separated chunk of length ≥ 3. Throws on no match. Used as
/// a fallback when agent-browser's role+name engine misses an element
/// that snapshot reports with role+name (Radix portals etc.).
fn text_presence_probe(session: &str, name: &str) -> Result<()> {
    if probe_text(session, name).is_ok() {
        return Ok(());
    }
    for chunk in name.split_whitespace() {
        if chunk.len() < 3 {
            continue;
        }
        if probe_text(session, chunk).is_ok() {
            return Ok(());
        }
    }
    bail!("no element matched text '{name}' (or any of its chunks)")
}

fn probe_text(session: &str, want: &str) -> Result<()> {
    let expr = format!(
        "(() => {{ const want = {q}; const hit = [...document.querySelectorAll('*')].find(el => (el.innerText || el.textContent || '').includes(want)); if (!hit) throw new Error('text not found: ' + want); }})()",
        q = serde_json::to_string(want).expect("string serializes")
    );
    browser::eval_expression(session, &expr)?;
    Ok(())
}

/// Snapshot-based presence probe: parse the ARIA snapshot for
/// `<role> "<name>" [ref=...]`. Returns Ok if the line exists — the
/// element is present in the page's a11y tree even if the role and
/// text engines can't locate it. Used as the final fallback rung for
/// presence asserts.
fn snapshot_presence_probe(session: &str, role: &str, name: &str) -> Result<()> {
    let snap = browser::snapshot_full(session)
        .map_err(|e| anyhow!("snapshot for ref-based presence probe: {e}"))?;
    if browser::find_ref_in_snapshot(&snap, role, name).is_some() {
        Ok(())
    } else {
        bail!("no `{role} \"{name}\" [ref=...]` line in snapshot")
    }
}

fn poll_until(
    timeout: Duration,
    mut probe: impl FnMut(&mut ValueScope) -> Result<()>,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut scope = ValueScope::default(); // unused but matches signature
    let mut last: Option<anyhow::Error> = None;
    while Instant::now() < deadline {
        match probe(&mut scope) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                thread::sleep(POLL_INTERVAL);
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("predicate timed out")))
}

// ---------- url ----------

fn check_url(
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    if matches!(predicate, Predicate::Exists | Predicate::NotExists) {
        // No expected value required — for url we always 'exist'.
        return Ok(());
    }
    let expected = expected
        .ok_or_else(|| anyhow!("url claim with predicate '{predicate:?}' requires 'value'"))?;
    let need = value_to_string(expected);
    let need = substitute_scenario_vars(&need, scope);

    let deadline = Instant::now() + timeout;
    let mut last_err: Option<anyhow::Error> = None;
    let mut last_href = String::new();
    while Instant::now() < deadline {
        let href =
            decode_json_string(browser::eval_expression(ctx.session, "location.href")?.trim());
        let path =
            decode_json_string(browser::eval_expression(ctx.session, "location.pathname")?.trim());
        last_href = href.clone();
        match compare_string(predicate, &href, &need).or_else(|err_href| {
            compare_string(predicate, &path, &need).map_err(|err_path| {
                last_err = Some(err_path);
                err_href
            })
        }) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err.get_or_insert(e);
                thread::sleep(POLL_INTERVAL);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("url claim timed out; last seen url={last_href:?}")))
}

fn decode_json_string(raw: &str) -> String {
    serde_json::from_str::<Json>(raw)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| raw.to_string())
}

// ---------- data / var ----------

fn read_saved(scope: &mut ValueScope, name: &str, path: Option<&str>) -> Result<Json> {
    let base = scope
        .saved_steps
        .get(name)
        .cloned()
        .or_else(|| scope.inputs.get(name).cloned())
        .ok_or_else(|| anyhow!("no saved binding or input named '{name}'"))?;
    match path {
        Some(p) => select_json_path(&base, p),
        None => Ok(base),
    }
}

fn check_value(
    actual: &Json,
    predicate: &Predicate,
    expected: Option<&Json>,
    scope: &mut ValueScope,
) -> Result<()> {
    match predicate {
        Predicate::Exists => {
            if actual.is_null() {
                bail!("expected value to exist");
            }
            Ok(())
        }
        Predicate::NotExists => {
            if !actual.is_null() {
                bail!("expected value not to exist");
            }
            Ok(())
        }
        _ => {
            let expected =
                expected.ok_or_else(|| anyhow!("predicate '{predicate:?}' requires 'value'"))?;
            let need = substitute_scenario_vars(&value_to_string(expected), scope);
            compare_string(predicate, &value_to_string(actual), &need)
        }
    }
}

// ---------- predicates ----------

fn compare_string(predicate: &Predicate, actual: &str, need: &str) -> Result<()> {
    match predicate {
        Predicate::Equals => {
            if actual != need {
                bail!("expected equals {need:?}, got {actual:?}");
            }
            Ok(())
        }
        Predicate::Contains => {
            if !actual.contains(need) {
                bail!("expected to contain {need:?}, got {actual:?}");
            }
            Ok(())
        }
        Predicate::Matches => {
            let re = Regex::new(need).map_err(|e| anyhow!("invalid regex {need:?}: {e}"))?;
            if !re.is_match(actual) {
                bail!("expected to match /{need}/, got {actual:?}");
            }
            Ok(())
        }
        Predicate::StartsWith => {
            if !actual.starts_with(need) {
                bail!("expected to start with {need:?}, got {actual:?}");
            }
            Ok(())
        }
        Predicate::EndsWith => {
            if !actual.ends_with(need) {
                bail!("expected to end with {need:?}, got {actual:?}");
            }
            Ok(())
        }
        other => bail!("predicate '{other:?}' not supported for this subject"),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serde_json::json;

    fn pred_from_json(j: serde_json::Value) -> Predicate {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn compare_equals() {
        compare_string(&pred_from_json(json!("equals")), "x", "x").unwrap();
        compare_string(&pred_from_json(json!("equals")), "x", "y").unwrap_err();
    }

    #[test]
    fn compare_contains_matches_starts_ends() {
        compare_string(&pred_from_json(json!("contains")), "hello world", "lo wo").unwrap();
        compare_string(&pred_from_json(json!("matches")), "abc123", r"\d{3}$").unwrap();
        compare_string(&pred_from_json(json!("startsWith")), "abc", "ab").unwrap();
        compare_string(&pred_from_json(json!("endsWith")), "abc", "bc").unwrap();
    }

    #[test]
    fn check_value_exists_and_not_exists() {
        let mut scope = ValueScope::default();
        check_value(
            &json!("x"),
            &pred_from_json(json!("exists")),
            None,
            &mut scope,
        )
        .unwrap();
        check_value(
            &json!(null),
            &pred_from_json(json!("exists")),
            None,
            &mut scope,
        )
        .unwrap_err();
        check_value(
            &json!(null),
            &pred_from_json(json!("notExists")),
            None,
            &mut scope,
        )
        .unwrap();
    }

    #[test]
    fn read_saved_walks_path() {
        let mut scope = ValueScope::default();
        scope
            .saved_steps
            .insert("user".into(), json!({ "profile": { "id": "u1" } }));
        let v = read_saved(&mut scope, "user", Some("$.profile.id")).unwrap();
        assert_eq!(v, json!("u1"));
    }

    #[test]
    fn decode_json_string_unwraps_quoted_strings() {
        assert_eq!(decode_json_string("\"hello\""), "hello");
        assert_eq!(decode_json_string("hello"), "hello");
        assert_eq!(decode_json_string("123"), "123");
    }

    #[test]
    fn dispatch_data_subject_equals_via_saved_step() {
        let mut scope = ValueScope::default();
        scope.saved_steps.insert("greet".into(), json!("hello"));
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "data": "greet" },
            "predicate": "equals",
            "value": "hello"
        }))
        .unwrap();
        let ctx = CheckContext { session: "s" };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn dispatch_var_subject_with_path() {
        let mut scope = ValueScope::default();
        scope
            .saved_steps
            .insert("u".into(), json!({ "n": "alice" }));
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "kind": "var", "name": "u", "path": "$.n" },
            "predicate": "matches",
            "value": "^al"
        }))
        .unwrap();
        let ctx = CheckContext { session: "s" };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn dispatch_network_subject_not_implemented() {
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "network": { "urlMatches": "x" } },
            "predicate": "exists"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext { session: "s" };
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    mod flag_subject {
        use super::*;
        use crate::browser as ab;
        use crate::test_util::lock_env;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;
        use tempfile::TempDir;

        fn install_fake_eval(dir: &Path, payload: &str) {
            // Echo the IIFE return verbatim. agent-browser's eval bridge
            // would normally double-encode strings, but our IIFE returns
            // an object directly so the wire is a single layer of JSON.
            let resp_path = dir.join("resp.txt");
            fs::write(&resp_path, payload).unwrap();
            let body = format!(
                "#!/bin/sh\nif [ \"$3\" = eval ]; then cat '{}'; exit 0; fi\nexit 0\n",
                resp_path.display()
            );
            let bin = dir.join("agent-browser");
            fs::write(&bin, body).unwrap();
            let mut perm = fs::metadata(&bin).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&bin, perm).unwrap();
            std::env::set_var(ab::BIN_ENV, &bin);
            ab::_reset_bin_cache_for_tests();
        }

        fn clear() {
            std::env::remove_var(ab::BIN_ENV);
            ab::_reset_bin_cache_for_tests();
        }

        #[test]
        fn equals_true_passes_when_flag_true() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{"my-flag":true}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "my-flag" },
                "predicate": "equals",
                "value": true
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext { session: "s" };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn equals_true_fails_when_flag_false() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{"my-flag":false}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "my-flag" },
                "predicate": "equals",
                "value": true
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext { session: "s" };
            let err = dispatch_check(&claim, &ctx, &mut scope, None)
                .unwrap_err()
                .to_string();
            assert!(err.contains("= true, got false"));
            clear();
        }

        #[test]
        fn exists_passes_when_present_either_value() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{"my-flag":false}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "my-flag" },
                "predicate": "exists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext { session: "s" };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn not_exists_passes_when_absent() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "unset" },
                "predicate": "notExists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext { session: "s" };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }
    }
}
