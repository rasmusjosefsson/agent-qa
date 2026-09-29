//! `smart-fill` — type a literal value into a field by accessible name and
//! auto-record the matching do/type step.
//!
//! The label-first companion to `smart-click` / `fill-unique`: resolves the
//! field through agent-browser's role+name `fill` (default role `textbox`),
//! then records `do/type` with `on: {role, name}` + a literal value so replay
//! targets the same accessible name. Use `fill-unique` when the value must be
//! minted per run.
//!
//! CLI shape:
//!
//!   agent-qa smart-fill "<accessible-name>" <value>
//!                        [--role <role>]      default: 'textbox'
//!                        [--no-record]
//!                        [--session <name>]

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::browser::{self, RoleAct};
use crate::record_step::StepKind;
use crate::recorder_state::RecorderState;

const DEFAULT_ROLE: &str = "textbox";

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let mut state = RecorderState::load_active()?;
    let session = opts
        .session
        .clone()
        .unwrap_or_else(|| state.session.clone());

    browser::find_role_act(
        &session,
        &opts.role,
        &opts.name,
        RoleAct::Fill,
        Some(&opts.value),
    )
    .with_context(|| {
        format!(
            "agent-browser find role {} fill --name {:?}",
            opts.role, opts.name
        )
    })?;

    if opts.record {
        let payload = json!({
            "intent": format!("smart-fill {}", opts.name),
            "verb": "type",
            "on": { "role": opts.role, "name": opts.name },
            "value": { "from": "literal", "literal": opts.value },
        });
        match crate::record_step::record_draft(&mut state, StepKind::Do, &payload, &session)? {
            Some(row) => println!(
                "filled {} (step {}) — role={} name={:?}",
                opts.name, row.step_id, opts.role, opts.name
            ),
            None => println!("filled {} — not recorded (recording paused)", opts.name),
        }
    } else {
        println!("filled role={} name={:?}", opts.role, opts.name);
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa smart-fill - type a value into a field by accessible name + auto-record\n\nUsage:\n  agent-qa smart-fill \"<accessible-name>\" <value>\n                       [--role <role>]    (default: textbox)\n                       [--no-record]\n                       [--session <name>]\n\nResolves the field through agent-browser's role+name fill and appends a\ndo/type step carrying on{{role,name}} so replay targets the same\naccessible name. `fill-unique` is the minted-per-run variant."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    name: String,
    value: String,
    role: String,
    record: bool,
    session: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut positional: Vec<String> = Vec::new();
    let mut role: Option<String> = None;
    let mut record = true;
    let mut session: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--role" => role = it.next().cloned(),
            s if s.starts_with("--role=") => role = Some(s["--role=".len()..].to_string()),
            "--no-record" => record = false,
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => positional.push(other.to_string()),
        }
    }
    if positional.len() != 2 {
        bail!("usage: smart-fill \"<accessible-name>\" <value>");
    }
    Ok(Opts {
        name: positional[0].clone(),
        value: positional[1].clone(),
        role: role.unwrap_or_else(|| DEFAULT_ROLE.to_string()),
        record,
        session,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser::{self, BrowserConnection};
    use crate::paths;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn fill_records_a_type_step_with_the_literal() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        let binary = tmp.path().join("agent-browser");
        fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, &binary);
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        browser::_reset_bin_cache_for_tests();
        RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        )
        .save()
        .unwrap();
        run(&["Username".into(), "alice".into()]).unwrap();
        let state = RecorderState::load_active().unwrap();
        let step = serde_json::to_value(&state.steps[0]).unwrap();
        assert_eq!(step["kind"], "do");
        assert_eq!(step["verb"], "type");
        assert_eq!(step["on"]["role"], "textbox");
        assert_eq!(step["on"]["name"], "Username");
        assert_eq!(step["value"]["literal"], "alice");
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn parse_requires_name_and_value() {
        assert!(parse_args(&["Username".into()]).is_err());
        assert!(parse_args(&[]).is_err());
        let opts = parse_args(&["Username".into(), "x".into(), "--role=searchbox".into()]).unwrap();
        assert_eq!(opts.role, "searchbox");
    }
}
