//! `run-step` dispatches one direct scenario/2 draft against a live session.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::claims::{dispatch_check, CheckContext};
use crate::record_step::{parse_draft, StepKind};
use crate::recorder_state::RecorderState;
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
        "agent-qa run-step - dispatch one scenario/2 draft

Usage:
  agent-qa run-step <do|check> <draft-json> [--session <name>]

The draft has the same direct shape as record-step and omits id and kind."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    kind: StepKind,
    payload: Json,
    session: Option<String>,
}

struct Outcome {
    report: Json,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "help"))
    {
        print_help();
        std::process::exit(0);
    }
    let mut positionals = Vec::new();
    let mut session = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--session" => session = it.next().cloned(),
            value if value.starts_with("--session=") => {
                session = Some(value["--session=".len()..].to_string())
            }
            value if value.starts_with("--") => bail!("unknown flag {value:?}"),
            value => positionals.push(value.to_string()),
        }
    }
    if positionals.len() != 2 {
        bail!("usage: run-step <do|check> <draft-json> [--session <name>]");
    }
    Ok(Opts {
        kind: StepKind::parse(&positionals[0])?,
        payload: serde_json::from_str(&positionals[1])
            .with_context(|| format!("parse draft JSON: {:?}", positionals[1]))?,
        session,
    })
}

fn active_state() -> Result<Option<RecorderState>> {
    RecorderState::try_load_active()
}

fn run_step(opts: &Opts) -> Result<Outcome> {
    let step = parse_draft(opts.kind, &opts.payload, "s0")?;
    let state = active_state()?;
    let session = opts
        .session
        .clone()
        .or_else(|| state.as_ref().map(|state| state.session.clone()))
        .or_else(|| {
            std::env::var("AGENT_BROWSER_SESSION")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "default".to_string());
    let scenario_dir = state
        .as_ref()
        .and_then(|state| crate::paths::scenario_dir(&state.sid).ok())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let mut scope = ValueScope::new(HashMap::new());
    let (kind, verb) = match &step {
        Step::Do { verb, .. } => ("do", Some(format!("{verb:?}").to_ascii_lowercase())),
        Step::Check { .. } => ("check", None),
    };
    let result = match &step {
        Step::Do { .. } => dispatch_do(
            &step,
            &DoContext {
                session: &session,
                scenario_dir: &scenario_dir,
            },
            &mut scope,
        )
        .map(|_| ()),
        Step::Check { claim, .. } => {
            dispatch_check(claim, &CheckContext { session: &session }, &mut scope, None)
        }
    };
    let mut report = json!({ "ok": result.is_ok(), "stepId": "s0", "kind": kind, "intent": step.intent(), "session": session });
    if let Some(verb) = verb {
        report
            .as_object_mut()
            .unwrap()
            .insert("verb".into(), Json::String(verb));
    }
    if let Err(error) = result {
        report
            .as_object_mut()
            .unwrap()
            .insert("error".into(), Json::String(format!("{error:#}")));
    }
    Ok(Outcome { report })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser;
    use crate::test_util::lock_env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn direct_do_dispatches_and_legacy_kind_is_rejected() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("agent-browser");
        let log = tmp.path().join("browser.log");
        fs::write(
            &bin,
            format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&bin).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&bin, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, &bin);
        browser::_reset_bin_cache_for_tests();
        let outcome = run_step(&Opts { kind: StepKind::Do, payload: json!({"intent":"open","verb":"goto","value":{"from":"literal","literal":"https://example.com/"}}), session: Some("s".into()) }).unwrap();
        assert_eq!(outcome.report["ok"], true);
        assert!(fs::read_to_string(log)
            .unwrap()
            .contains("--session s open https://example.com/"));
        assert!(parse_args(&["navigation".into(), "{}".into()]).is_err());
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }
}
