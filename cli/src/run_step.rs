//! `run-step` verb — dispatch ONE step against the live session.
//!
//! This is the author-time counterpart to `record-step`: both take the
//! exact same friendly trigger payload (`navigation` / `action` / `wait`
//! / `assert`), but where `record-step` *appends* the step to the buffer,
//! `run-step` *executes* it against the live agent-browser session for
//! immediate feedback — without recording anything.
//!
//! It is the "run one step" primitive the authoring editor needs: it
//! reuses the same translation (`recorder_shape::map_row`) and the same
//! dispatchers (`verbs::dispatch_do` / `claims::dispatch_check`) the
//! replay runner uses, so a step that passes here replays identically.
//!
//! CLI shape:
//!
//!   agent-qa run-step <navigation|action|wait|assert> <payload-json>
//!                     [--session <name>]
//!
//! Output is a single JSON line on stdout describing the outcome:
//!
//!   {"ok":true,"stepId":"s0","kind":"do","verb":"click","intent":"…"}
//!   {"ok":false,"stepId":"s0","kind":"do","verb":"click","intent":"…",
//!    "error":"Element not found: …"}
//!
//! Exit code is 0 whenever the step actually *ran* (pass or fail) so the
//! editor gets structured feedback; only argument / payload problems bail
//! with a non-zero exit.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::claims::{dispatch_check, CheckContext};
use crate::paths;
use crate::recorder_shape::{map_row, validate_trigger, TriggerKind};
use crate::scenario::Step;
use crate::value::ValueScope;
use crate::verbs::{dispatch_do, DoContext};

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let outcome = run_step(&opts)?;
    println!("{}", serde_json::to_string(&outcome.report)?);
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa run-step — dispatch ONE step against the live session

Usage:
  agent-qa run-step <navigation|action|wait|assert> <payload-json>
                    [--session <name>]

Takes the same trigger payload as `record-step`, but EXECUTES the step
against the live agent-browser session instead of recording it. Use it
for fast author-time feedback while building a scenario in the editor.

Payload shapes are identical to `record-step` (see `record-step --help`).

Output: one JSON line on stdout —
  {{\"ok\":true,\"stepId\":\"s0\",\"kind\":\"do\",\"verb\":\"click\",\"intent\":\"…\"}}
  {{\"ok\":false,…,\"error\":\"<dispatch error>\"}}

Exit code is 0 whenever the step ran (pass or fail). Only a malformed
payload / unknown kind exits non-zero.

run-step does NOT record anything and does NOT drive navigation baseline
(env.open); it dispatches exactly the one step you pass."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    kind: TriggerKind,
    payload: Json,
    session: Option<String>,
}

struct Outcome {
    report: Json,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        print_help();
        std::process::exit(0);
    }
    let mut positional: Vec<String> = Vec::new();
    let mut session: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => positional.push(other.to_string()),
        }
    }
    if positional.len() < 2 {
        bail!("usage: run-step <navigation|action|wait|assert> <payload-json> [--session <name>]");
    }
    let kind = TriggerKind::parse(&positional[0])?;
    let payload: Json = serde_json::from_str(&positional[1])
        .with_context(|| format!("parse payload JSON: {:?}", positional[1]))?;
    validate_trigger(kind, &payload)
        .with_context(|| format!("run-step {}: invalid payload", kind.as_str()))?;
    Ok(Opts {
        kind,
        payload,
        session,
    })
}

/// Resolve the session name: explicit `--session` wins, else the SESSION
/// line from the active recording env, else the ambient
/// `AGENT_BROWSER_SESSION` (the host's per-chat browser binding), else
/// `default`.
fn resolve_session(explicit: Option<&str>) -> String {
    if let Some(s) = explicit {
        return s.to_string();
    }
    if let Ok(body) = fs::read_to_string(paths::record_env_file()) {
        if let Some(s) = extract(&body, "SESSION") {
            if !s.is_empty() {
                return s;
            }
        }
    }
    if let Ok(s) = std::env::var("AGENT_BROWSER_SESSION") {
        let s = s.trim().to_string();
        if !s.is_empty() {
            return s;
        }
    }
    "default".to_string()
}

/// Resolve a scenario directory for `DoContext` (only the `upload` verb
/// reads it, to resolve relative file paths). Prefer the active recording
/// sid's directory; fall back to the current working directory.
fn resolve_scenario_dir() -> PathBuf {
    if let Ok(body) = fs::read_to_string(paths::record_env_file()) {
        if let Some(sid) = extract(&body, "SID") {
            if let Ok(dir) = paths::scenario_dir(&sid) {
                return dir;
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn extract(env: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    env.lines()
        .find_map(|l| l.strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

fn run_step(opts: &Opts) -> Result<Outcome> {
    // Translate the trigger payload into a scenario/2 step, exactly as
    // `flush` would, then parse it into the same `Step` the runner uses.
    let step_json = map_row(opts.kind.as_str(), &opts.payload, "s0")
        .with_context(|| "translate trigger payload to scenario/2 step")?;
    let step: Step = serde_json::from_value(step_json.clone()).context("parse translated step")?;

    let session = resolve_session(opts.session.as_deref());
    let scenario_dir = resolve_scenario_dir();
    let mut scope = ValueScope::new(HashMap::new());

    let (kind_label, verb_label) = match &step {
        Step::Do { verb, .. } => ("do", Some(format!("{verb:?}").to_ascii_lowercase())),
        Step::Check { .. } => ("check", None),
    };
    let intent = step.intent().to_string();

    let result = match &step {
        Step::Do { .. } => {
            let ctx = DoContext {
                session: &session,
                scenario_dir: &scenario_dir,
            };
            dispatch_do(&step, &ctx, &mut scope).map(|_| ())
        }
        Step::Check { claim, .. } => {
            let ctx = CheckContext { session: &session };
            dispatch_check(claim, &ctx, &mut scope, None)
        }
    };

    let mut report = json!({
        "ok": result.is_ok(),
        "stepId": "s0",
        "kind": kind_label,
        "intent": intent,
        "session": session,
    });
    if let Some(verb) = verb_label {
        report
            .as_object_mut()
            .unwrap()
            .insert("verb".into(), Json::String(verb));
    }
    if let Err(e) = &result {
        report
            .as_object_mut()
            .unwrap()
            .insert("error".into(), Json::String(format!("{e:#}")));
    }
    Ok(Outcome { report })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser;
    use crate::test_util::lock_env;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_exec(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("agent-browser");
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn install_fake(dir: &Path, body: &str) {
        let bin = write_exec(dir, body);
        std::env::set_var(browser::BIN_ENV, &bin);
        browser::_reset_bin_cache_for_tests();
    }

    fn clear_fake() {
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn parse_args_requires_kind_and_payload() {
        parse_args(&["navigation".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_rejects_unknown_kind() {
        parse_args(&["wat".into(), "{}".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_rejects_bad_json() {
        parse_args(&["navigation".into(), "not json".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_session_flag() {
        let o = parse_args(&[
            "navigation".into(),
            r#"{"route":"https://x/"}"#.into(),
            "--session".into(),
            "sx".into(),
        ])
        .unwrap();
        assert_eq!(o.session.as_deref(), Some("sx"));
    }

    #[test]
    fn run_step_navigation_dispatches_open_and_reports_ok() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("rec"));
        let log = tmp.path().join("ab.log");
        install_fake(
            tmp.path(),
            &format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display()),
        );

        let out = run_step(&Opts {
            kind: TriggerKind::Navigation,
            payload: json!({ "route": "https://example.com/" }),
            session: Some("sx".into()),
        })
        .unwrap();
        assert_eq!(out.report["ok"], true);
        assert_eq!(out.report["kind"], "do");
        assert_eq!(out.report["verb"], "goto");
        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            invocation.contains("--session sx open https://example.com/"),
            "got: {invocation}"
        );

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake();
    }

    #[test]
    fn run_step_reports_dispatch_failure_without_bailing() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("rec"));
        // agent-browser exits non-zero → dispatch returns Err → ok:false.
        install_fake(
            tmp.path(),
            "#!/bin/sh\necho 'Element not found' 1>&2\nexit 1\n",
        );

        let out = run_step(&Opts {
            kind: TriggerKind::Action,
            payload: json!({ "method": "clickRole", "args": ["button", "Login"] }),
            session: Some("sx".into()),
        })
        .unwrap();
        assert_eq!(out.report["ok"], false);
        assert_eq!(out.report["verb"], "click");
        assert!(
            out.report["error"].as_str().unwrap().contains("exited 1"),
            "got: {}",
            out.report["error"]
        );

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake();
    }

    #[test]
    fn resolve_session_prefers_explicit_then_env_then_default() {
        let _g = lock_env();
        // Ambient AGENT_BROWSER_SESSION must not leak into the file/default
        // precedence assertions below.
        std::env::remove_var("AGENT_BROWSER_SESSION");
        let tmp = TempDir::new().unwrap();
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        fs::create_dir_all(&rec).unwrap();

        // No env file → default.
        assert_eq!(resolve_session(None), "default");
        // Env file SESSION → that.
        fs::write(rec.join("scenario.env"), "SID=x\nSESSION=senv\n").unwrap();
        assert_eq!(resolve_session(None), "senv");
        // Explicit wins.
        assert_eq!(resolve_session(Some("flag")), "flag");

        std::env::remove_var(paths::RECORD_DIR_ENV);
    }
}
