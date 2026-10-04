//! `smart-select` verb — pick an option in a combo/select control by
//! description and auto-record the do/select step.
//!
//! `<desc>` describes the *control* ("the country picker"); `--value` is
//! the option to choose. Fuzzy descriptions resolve through the
//! configured `resolve` plugin; the recorded step keeps the concrete
//! role+name so replay never needs it.
//!
//! CLI shape:
//!
//!   agent-qa smart-select "<description>" --value "<option>"
//!                          [--role <role>]    default: combobox
//!                          [--no-record]
//!                          [--session <name>]

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;

use crate::record_step::{parse_draft, record_draft, StepKind};
use crate::recorder_state::RecorderState;
use crate::resolve::{self, ResolvedPick};
use crate::value::ValueScope;
use crate::verbs::{dispatch_do, DoContext};

const DEFAULT_ROLE: &str = "combobox";

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let mut state = RecorderState::load_active()?;
    let session = opts
        .session
        .clone()
        .unwrap_or_else(|| state.session.clone());
    let scenario_dir = crate::paths::scenario_dir(&state.sid)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let role = opts
        .role
        .clone()
        .unwrap_or_else(|| DEFAULT_ROLE.to_string());
    let (role, name) = match resolve_target(&session, &opts, &role)? {
        Some(t) => t,
        None => bail!(
            "could not resolve {:?} to a {role}{}",
            opts.description,
            resolve_note()
        ),
    };

    let payload = json!({
        "intent": format!("smart-select {}", opts.description),
        "verb": "select",
        "on": { "role": role, "name": name },
        "value": opts.value,
    });
    let step = parse_draft(StepKind::Do, &payload, "s0")?;
    let mut scope = ValueScope::new(HashMap::new());
    dispatch_do(
        &step,
        &DoContext {
            session: &session,
            scenario_dir: &scenario_dir,
            visual_checks: false,
            uses_dialog: false,
        },
        &mut scope,
    )
    .with_context(|| {
        format!(
            "resolved to {role} {name:?} but select {:?} failed",
            opts.value
        )
    })?;

    if opts.record {
        match record_draft(&mut state, StepKind::Do, &payload, &session)? {
            Some(row) => println!(
                "selected {:?} on {} (step {}) — {role} {name:?}",
                opts.value, row.step_id, row.step_id
            ),
            None => println!(
                "selected {:?} on {role} {name:?} — not recorded (recording paused)",
                opts.value
            ),
        }
    } else {
        println!("selected {:?} on {role} {name:?}", opts.value);
    }
    Ok(0)
}

/// Rung 1: exact role+name in the live snapshot. Rung 2: the `resolve`
/// plugin picks the control among interactive candidates — the picked
/// element's own role is what the step records.
fn resolve_target(session: &str, opts: &Opts, role: &str) -> Result<Option<(String, String)>> {
    if let Ok(snap) = crate::browser::snapshot_full(session) {
        if crate::browser::find_ref_in_snapshot(&snap, role, &opts.description).is_some() {
            return Ok(Some((role.to_string(), opts.description.clone())));
        }
    }
    let pick = match resolve::resolve_element_in_role(session, &opts.description, role) {
        Ok(pick) => pick,
        Err(e) => {
            eprintln!("resolve plugin: {e:#} — continuing without it");
            return Ok(None);
        }
    };
    Ok(pick.map(|ResolvedPick { candidate, .. }| (candidate.role, candidate.name)))
}

fn resolve_note() -> String {
    match resolve::find_plugin() {
        Ok(Some(_)) => " — the resolve plugin had no confident pick".to_string(),
        Ok(None) => " — no `resolve` plugin configured ([plugins] resolve = ...)".to_string(),
        Err(_) => String::new(),
    }
}

fn print_help() {
    println!(
        "agent-qa smart-select - pick an option in a select by description + auto-record\n\nUsage:\n  agent-qa smart-select \"<description>\" --value \"<option>\"\n                        [--role <role>]    control role (default: combobox)\n                        [--no-record]\n                        [--session <name>]\n\n<description> names the control, --value the option text. Fuzzy\ndescriptions resolve through the configured `resolve` plugin; the\nrecorded do/select step keeps the concrete role+name."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    description: String,
    value: String,
    role: Option<String>,
    record: bool,
    session: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut description: Option<String> = None;
    let mut value: Option<String> = None;
    let mut role = None;
    let mut record = true;
    let mut session = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--value" => value = it.next().cloned(),
            s if s.starts_with("--value=") => value = Some(s["--value=".len()..].to_string()),
            "--role" => role = it.next().cloned(),
            s if s.starts_with("--role=") => role = Some(s["--role=".len()..].to_string()),
            "--no-record" => record = false,
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if description.is_some() {
                    bail!("unexpected positional {other:?}; usage: smart-select \"<description>\" --value \"<option>\"");
                }
                description = Some(other.to_string());
            }
        }
    }
    Ok(Opts {
        description: description
            .ok_or_else(|| anyhow!("usage: smart-select \"<description>\" --value \"<option>\""))?,
        value: value.ok_or_else(|| anyhow!("smart-select: --value is required"))?,
        role,
        record,
        session,
    })
}
