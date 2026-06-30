//! scenario/2 `env.open[*]` / `env.close[*]` op dispatcher.
//!
//! Five kinds today (matching the schema and the typed [`EnvOp`]):
//!
//!   - `nav`          — `browser::open(url)`
//!   - `localStorage` — eval `localStorage.setItem(key, value)`
//!   - `flag`         — merge into `localStorage["devtools-flag-overrides"]` JSON map
//!   - `cookie`       — eval `document.cookie = "name=value[; domain=…; path=…]"`
//!                      (best-effort; HttpOnly cookies cannot be set this way —
//!                      plugins land later for richer cookie management)
//!   - `gql`          — fetch the live tab's `<url>` with the query/variables
//!                      via in-page eval so the request carries the session
//!                      cookies the rest of the scenario is operating against.
//!                      `forEach` iterates the resolved Value list, calling
//!                      the body for each item. `saveAs` records the parsed
//!                      JSON response into `ValueScope.saved_steps`.
//!
//! Each op accepts an optional `policy` (alwaysRun/onAbort/onFailure)
//! per the schema; honour the `onFailure` axis here — `continue` lets
//! the next op proceed; default `abort` propagates the error.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value as Json;

use crate::browser;
use crate::scenario::{EnvOp, EnvOpPolicy, OnFailureContinue, Value};
use crate::value::{resolve_value, substitute_scenario_vars, ValueScope};

pub fn run_phase(phase: &str, ops: &[EnvOp], session: &str, scope: &mut ValueScope) -> Result<()> {
    for (idx, op) in ops.iter().enumerate() {
        let policy = policy_of(op);
        let result = run_one(phase, idx, op, session, scope);
        if let Err(e) = result {
            if matches!(
                policy.and_then(|p| p.on_failure.as_ref()),
                Some(OnFailureContinue::Continue)
            ) {
                eprintln!("[v2-replay] {phase}#{idx} failed (continue per policy): {e}");
                continue;
            }
            return Err(e);
        }
    }
    Ok(())
}

fn policy_of(op: &EnvOp) -> Option<&EnvOpPolicy> {
    match op {
        EnvOp::Fresh { policy, .. }
        | EnvOp::UseProfile { policy, .. }
        | EnvOp::Nav { policy, .. }
        | EnvOp::Cookie { policy, .. }
        | EnvOp::LocalStorage { policy, .. }
        | EnvOp::Gql { policy, .. }
        | EnvOp::Flag { policy, .. } => policy.as_ref(),
    }
}

fn run_one(
    phase: &str,
    idx: usize,
    op: &EnvOp,
    session: &str,
    scope: &mut ValueScope,
) -> Result<()> {
    match op {
        EnvOp::Fresh { intent, .. } => {
            eprintln!(
                "[v2-replay] {phase}#{idx} fresh: clear cookies + localStorage + sessionStorage{}",
                intent
                    .as_deref()
                    .map(|s| format!(" ({s})"))
                    .unwrap_or_default()
            );
            reset_browser_state(session)
                .with_context(|| format!("[{phase}#{idx}] fresh: reset browser state"))?;
            Ok(())
        }
        EnvOp::UseProfile { intent, name, .. } => {
            eprintln!(
                "[v2-replay] {phase}#{idx} useProfile: bootstrap {name:?}{}",
                intent
                    .as_deref()
                    .map(|s| format!(" ({s})"))
                    .unwrap_or_default()
            );
            bootstrap_profile(name, session)
                .with_context(|| format!("[{phase}#{idx}] useProfile: bootstrap {name}"))?;
            Ok(())
        }
        EnvOp::Nav { url, intent, .. } => {
            let url = url
                .as_deref()
                .ok_or_else(|| anyhow!("[{phase}#{idx}] nav requires `url`"))?;
            let resolved = substitute_scenario_vars(url, scope);
            eprintln!(
                "[v2-replay] {phase}#{idx} nav {resolved}{}",
                intent
                    .as_deref()
                    .map(|i| format!(" — {i}"))
                    .unwrap_or_default()
            );
            browser::open(session, &resolved)
                .with_context(|| format!("[{phase}#{idx}] browser open {resolved}"))
        }
        EnvOp::LocalStorage {
            key, value, intent, ..
        } => {
            let key = key
                .as_deref()
                .ok_or_else(|| anyhow!("[{phase}#{idx}] localStorage requires `key`"))?;
            let value = value
                .as_deref()
                .ok_or_else(|| anyhow!("[{phase}#{idx}] localStorage requires `value`"))?;
            let key = substitute_scenario_vars(key, scope);
            let value = substitute_scenario_vars(value, scope);
            eprintln!(
                "[v2-replay] {phase}#{idx} localStorage[{}]={}{}",
                json_str(&key),
                json_str(&value),
                intent
                    .as_deref()
                    .map(|i| format!(" — {i}"))
                    .unwrap_or_default()
            );
            let expr = format!(
                "(() => {{ localStorage.setItem({}, {}); }})()",
                json_str(&key),
                json_str(&value)
            );
            browser::eval_expression(session, &expr)?;
            Ok(())
        }
        EnvOp::Flag {
            name,
            enabled,
            intent,
            ..
        } => {
            let name = substitute_scenario_vars(name, scope);
            eprintln!(
                "[v2-replay] {phase}#{idx} flag {name}={enabled}{}",
                intent
                    .as_deref()
                    .map(|i| format!(" — {i}"))
                    .unwrap_or_default()
            );
            // Merge into devtools-flag-overrides JSON map.
            let expr = format!(
                "(() => {{ \
                    const k = 'devtools-flag-overrides'; \
                    let m; try {{ m = JSON.parse(localStorage.getItem(k) || '{{}}'); }} catch {{ m = {{}}; }} \
                    m[{}] = {}; \
                    localStorage.setItem(k, JSON.stringify(m)); \
                }})()",
                json_str(&name),
                if *enabled { "true" } else { "false" }
            );
            browser::eval_expression(session, &expr)?;
            Ok(())
        }
        EnvOp::Cookie {
            name,
            value,
            domain,
            path,
            intent,
            ..
        } => {
            let name = name
                .as_deref()
                .ok_or_else(|| anyhow!("[{phase}#{idx}] cookie requires `name`"))?;
            let value = value
                .as_deref()
                .ok_or_else(|| anyhow!("[{phase}#{idx}] cookie requires `value`"))?;
            let name = substitute_scenario_vars(name, scope);
            let value = substitute_scenario_vars(value, scope);
            let mut cookie = format!("{name}={value}");
            if let Some(d) = domain.as_deref() {
                cookie.push_str(&format!("; domain={d}"));
            }
            if let Some(p) = path.as_deref() {
                cookie.push_str(&format!("; path={p}"));
            }
            eprintln!(
                "[v2-replay] {phase}#{idx} cookie {name}=…{}",
                intent
                    .as_deref()
                    .map(|i| format!(" — {i}"))
                    .unwrap_or_default()
            );
            let expr = format!("(() => {{ document.cookie = {}; }})()", json_str(&cookie));
            browser::eval_expression(session, &expr)?;
            Ok(())
        }
        EnvOp::Gql {
            url,
            query,
            variables,
            for_each,
            save_as,
            intent,
            ..
        } => run_gql(
            phase,
            idx,
            url,
            query,
            variables.as_ref(),
            for_each.as_ref(),
            save_as.as_deref(),
            intent.as_deref(),
            session,
            scope,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_gql(
    phase: &str,
    idx: usize,
    url: &str,
    query: &str,
    variables: Option<&std::collections::BTreeMap<String, Json>>,
    for_each: Option<&Value>,
    save_as: Option<&str>,
    intent: Option<&str>,
    session: &str,
    scope: &mut ValueScope,
) -> Result<()> {
    let url_resolved = substitute_scenario_vars(url, scope);
    let query_resolved = substitute_scenario_vars(query, scope);

    let make_variables = |item: Option<&Json>| -> Json {
        let mut map = serde_json::Map::new();
        if let Some(vs) = variables {
            for (k, v) in vs {
                map.insert(k.clone(), v.clone());
            }
        }
        if let Some(it) = item {
            map.insert("item".into(), it.clone());
        }
        Json::Object(map)
    };

    eprintln!(
        "[v2-replay] {phase}#{idx} gql {url_resolved}{}",
        intent.map(|i| format!(" — {i}")).unwrap_or_default()
    );

    let mut responses: Vec<Json> = Vec::new();

    if let Some(fe) = for_each {
        let resolved = resolve_value(fe, scope)?;
        let items = match resolved {
            Json::Array(a) => a,
            other => bail!(
                "[{phase}#{idx}] gql.forEach must resolve to an array, got {}",
                json_kind(&other)
            ),
        };
        for item in items {
            let vars = make_variables(Some(&item));
            let r = post_gql_via_eval(session, &url_resolved, &query_resolved, &vars)?;
            responses.push(r);
        }
    } else {
        let vars = make_variables(None);
        let r = post_gql_via_eval(session, &url_resolved, &query_resolved, &vars)?;
        responses.push(r);
    }

    if let Some(name) = save_as {
        let payload = if for_each.is_some() {
            Json::Array(responses)
        } else {
            responses.into_iter().next().unwrap_or(Json::Null)
        };
        scope.saved_steps.insert(name.to_string(), payload);
    }
    Ok(())
}

/// POST a GraphQL request via in-page `fetch` so session cookies travel
/// with it. Returns the parsed JSON body. Throws on non-2xx or on a
/// `errors[]` array in the response (callers can opt into ignoring
/// errors by checking the saved binding themselves).
pub(crate) fn post_gql_via_eval(
    session: &str,
    url: &str,
    query: &str,
    variables: &Json,
) -> Result<Json> {
    let body = serde_json::to_string(&serde_json::json!({
        "query": query,
        "variables": variables,
    }))?;
    let expr = format!(
        "(async () => {{ \
            const r = await fetch({}, {{ \
                method: 'POST', \
                credentials: 'include', \
                headers: {{ 'content-type': 'application/json' }}, \
                body: {} \
            }}); \
            const text = await r.text(); \
            return JSON.stringify({{ status: r.status, body: text }}); \
        }})()",
        json_str(url),
        json_str(&body)
    );
    let raw = browser::eval_expression(session, &expr)?;
    // agent-browser double-encodes string returns (see unwrap pattern
    // in browser.ts:unwrapEvalString). Decode once then parse.
    let outer = raw.trim();
    let outer_str: String = serde_json::from_str(outer).unwrap_or_else(|_| outer.to_string());
    let envelope: Json = serde_json::from_str(&outer_str)
        .with_context(|| format!("parse gql eval envelope: {outer_str:?}"))?;
    let status = envelope.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let body_text = envelope.get("body").and_then(|v| v.as_str()).unwrap_or("");
    if !(200..300).contains(&status) {
        bail!("gql {url}: HTTP {status} — body: {body_text}");
    }
    let parsed: Json = serde_json::from_str(body_text)
        .with_context(|| format!("parse gql body: {body_text:?}"))?;
    if let Some(errors) = parsed.get("errors").and_then(|v| v.as_array()) {
        if !errors.is_empty() {
            bail!("gql {url}: errors[] in response: {errors:?}");
        }
    }
    Ok(parsed)
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("string serializes")
}

fn json_kind(v: &Json) -> &'static str {
    match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

/// Implementation of `EnvOp::Fresh`. Wipes the active session's
/// cookies + localStorage + sessionStorage by shelling out to
/// agent-browser. Failures from any one wipe are non-fatal
/// (sub-commands log a warning); only an aggregate spawn failure
/// surfaces. Sub-failures are usually "nothing to clear" — the
/// agent-browser CLI exits 0 in that case.
fn reset_browser_state(session: &str) -> Result<()> {
    let opts = crate::browser::RunOpts::new().lenient();
    let _ = crate::browser::run(session, ["cookies", "clear"], opts);
    let _ = crate::browser::run(session, ["storage", "local", "clear"], opts);
    let _ = crate::browser::run(session, ["storage", "session", "clear"], opts);
    Ok(())
}

/// Implementation of `EnvOp::UseProfile`. Shells out to
/// `agent-qa profile-bootstrap <name> --session <replay session>` (the same
/// verb authors run at recording time) so replay reaches the same
/// authenticated baseline. Targeting the REPLAY session (not the profile's
/// default `<name>-session`) is what makes this idempotent: the auth plugin
/// probes that session first and no-ops if it is already signed in — so a
/// replay against a session a host already authenticated (e.g. the workbench
/// after `/connect`) needs no credentials. A fresh session still triggers a
/// full `auth login`, which requires the profile's credentials in env.
fn bootstrap_profile(name: &str, session: &str) -> Result<()> {
    use std::process::Command;
    let exe = std::env::current_exe().unwrap_or_else(|_| "agent-qa".into());
    let status = Command::new(exe)
        .args(["profile-bootstrap", name, "--session", session])
        .status()
        .with_context(|| format!("spawn agent-qa profile-bootstrap {name}"))?;
    if !status.success() {
        bail!("profile-bootstrap {name} exited {status}");
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser as ab;
    use crate::test_util::lock_env;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn install_fake_logging(dir: &Path, log: &Path) -> PathBuf {
        let body = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let bin = write_exec(dir, "agent-browser", &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
        bin
    }

    #[allow(dead_code)]
    fn install_fake_gql(dir: &Path, response_body: &str) -> PathBuf {
        // For 'eval --stdin' / 'eval <expr>' shape, output a JSON-wrapped
        // string envelope.
        let body = format!(
            "#!/bin/sh\nif [ \"$3\" = 'eval' ] || [ \"$2\" = 'eval' ]; then\n  cat <<'EOF'\n\"{{\\\"status\\\":200,\\\"body\\\":\\\"{}\\\"}}\"\nEOF\n  exit 0\nfi\nexit 0\n",
            response_body.replace('"', "\\\\\\\"")
        );
        let bin = write_exec(dir, "agent-browser", &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
        bin
    }

    fn clear_fake() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    fn parse_op(j: serde_json::Value) -> EnvOp {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn nav_open_uses_browser_open() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        let op = parse_op(json!({ "kind": "nav", "url": "https://example.com/" }));
        let mut scope = ValueScope::default();
        run_phase("env.open", &[op], "s", &mut scope).unwrap();
        let lines = fs::read_to_string(&log).unwrap();
        assert!(
            lines.contains("--session s open https://example.com/"),
            "got: {lines}"
        );
        clear_fake();
    }

    #[test]
    fn localstorage_sets_via_eval() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        let op = parse_op(json!({ "kind": "localStorage", "key": "x", "value": "1" }));
        let mut scope = ValueScope::default();
        run_phase("env.open", &[op], "s", &mut scope).unwrap();
        let lines = fs::read_to_string(&log).unwrap();
        assert!(lines.contains("--session s eval"), "got: {lines}");
        assert!(lines.contains("localStorage.setItem"), "got: {lines}");
        clear_fake();
    }

    #[test]
    fn flag_writes_devtools_overrides() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        let op = parse_op(json!({ "kind": "flag", "name": "my-flag", "enabled": true }));
        let mut scope = ValueScope::default();
        run_phase("env.open", &[op], "s", &mut scope).unwrap();
        let lines = fs::read_to_string(&log).unwrap();
        assert!(lines.contains("devtools-flag-overrides"), "got: {lines}");
        assert!(lines.contains("\"my-flag\""), "got: {lines}");
        clear_fake();
    }

    #[test]
    fn cookie_writes_document_cookie() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        let op = parse_op(json!({
            "kind": "cookie", "name": "session", "value": "abc",
            "domain": ".example.com", "path": "/"
        }));
        let mut scope = ValueScope::default();
        run_phase("env.open", &[op], "s", &mut scope).unwrap();
        let lines = fs::read_to_string(&log).unwrap();
        assert!(lines.contains("document.cookie"), "got: {lines}");
        assert!(lines.contains("session=abc"), "got: {lines}");
        assert!(lines.contains(".example.com"), "got: {lines}");
        clear_fake();
    }

    #[test]
    fn on_failure_continue_swallows_error() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        // Fake binary that fails open but succeeds for everything else.
        let body = "#!/bin/sh\nif [ \"$3\" = 'open' ]; then exit 5; fi\nexit 0\n";
        let bin = write_exec(tmp.path(), "agent-browser", body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();

        let op = parse_op(json!({
            "kind": "nav", "url": "https://example.com/",
            "policy": { "onFailure": "continue" }
        }));
        let mut scope = ValueScope::default();
        run_phase("env.open", &[op], "s", &mut scope).unwrap();
        clear_fake();
    }

    #[test]
    fn on_failure_default_propagates() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let body = "#!/bin/sh\nif [ \"$3\" = 'open' ]; then exit 5; fi\nexit 0\n";
        let bin = write_exec(tmp.path(), "agent-browser", body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
        let op = parse_op(json!({ "kind": "nav", "url": "https://example.com/" }));
        let mut scope = ValueScope::default();
        run_phase("env.open", &[op], "s", &mut scope).unwrap_err();
        clear_fake();
    }
}
