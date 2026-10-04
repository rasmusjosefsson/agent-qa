//! `smart-hover` verb — hover an element by accessible name / description
//! and auto-record the matching do/hover step.
//!
//! Same ladder idea as smart-click: exact role+name snapshot match first,
//! then the configured `resolve` plugin picks among interactive
//! candidates. The recorded step keeps the concrete role+name — replay
//! never needs the plugin.
//!
//! CLI shape:
//!
//!   agent-qa smart-hover "<description>"
//!                          [--role <role>]
//!                          [--no-record]
//!                          [--session <name>]

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use serde_json::json;

use crate::record_step::{parse_draft, record_draft, StepKind};
use crate::recorder_state::RecorderState;
use crate::resolve::{self, ResolvedPick};
use crate::value::ValueScope;
use crate::verbs::{dispatch_do, DoContext};

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let mut state = RecorderState::load_active()?;
    let session = opts
        .session
        .clone()
        .unwrap_or_else(|| state.session.clone());
    let scenario_dir = crate::paths::scenario_dir(&state.sid)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let (role, name) = match resolve_target(&session, &opts)? {
        Some(t) => t,
        None => bail!(
            "could not resolve {:?} to an element{}",
            opts.description,
            resolve_note()
        ),
    };

    let payload = json!({
        "intent": format!("smart-hover {}", opts.description),
        "verb": "hover",
        "on": { "role": role, "name": name },
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
    )?;

    if opts.record {
        match record_draft(&mut state, StepKind::Do, &payload, &session)? {
            Some(row) => println!(
                "hovered {} (step {}) — {role} {name:?}",
                row.step_id, row.step_id
            ),
            None => println!("hovered {role} {name:?} — not recorded (recording paused)"),
        }
    } else {
        println!("hovered {role} {name:?}");
    }
    Ok(0)
}

fn resolve_target(session: &str, opts: &Opts) -> Result<Option<(String, String)>> {
    if let Some(role) = opts.role.as_deref() {
        if let Ok(snap) = crate::browser::snapshot_full(session) {
            if crate::browser::find_ref_in_snapshot(&snap, role, &opts.description).is_some() {
                return Ok(Some((role.to_string(), opts.description.clone())));
            }
        }
    }
    let pick = match resolve::resolve_element(session, &opts.description, opts.role.as_deref()) {
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
        "agent-qa smart-hover - hover an element by description + auto-record\n\nUsage:\n  agent-qa smart-hover \"<description>\"\n                       [--role <role>]    preferred element role\n                       [--no-record]\n                       [--session <name>]\n\nFuzzy descriptions resolve through the configured `resolve` plugin; the\nrecorded do/hover step keeps the resolved concrete role+name."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    description: String,
    role: Option<String>,
    record: bool,
    session: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut description: Option<String> = None;
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
            "--role" => role = it.next().cloned(),
            s if s.starts_with("--role=") => role = Some(s["--role=".len()..].to_string()),
            "--no-record" => record = false,
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if description.is_some() {
                    bail!("unexpected positional {other:?}; usage: smart-hover \"<description>\"");
                }
                description = Some(other.to_string());
            }
        }
    }
    Ok(Opts {
        description: description.ok_or_else(|| anyhow!("usage: smart-hover \"<description>\""))?,
        role,
        record,
        session,
    })
}
