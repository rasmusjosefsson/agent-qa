//! `smart-assert` verb — record an element-existence check from a
//! description.
//!
//! Check steps need a concrete role+name or raw locator; fuzzy authoring
//! descriptions resolve through the same snapshot-exact + `resolve`-plugin
//! ladder smart-click uses. The pick is dispatched live as feedback and
//! recorded with the resolved locator — replay never needs the plugin.
//!
//! CLI shape:
//!
//!   agent-qa smart-assert "<description>"
//!                          [--role <role>]
//!                          [--predicate <p>]   default: isVisible
//!                          [--no-record]
//!                          [--session <name>]

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;

use crate::claims::{dispatch_check, CheckContext};
use crate::record_step::{parse_draft, record_draft, StepKind};
use crate::recorder_state::RecorderState;
use crate::resolve::{self, ResolvedPick};
use crate::scenario::Step;
use crate::value::ValueScope;

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
        "intent": format!("smart-assert {}", opts.description),
        "claim": {
            "subject": { "element": { "role": role, "name": name } },
            "predicate": opts.predicate,
        },
    });
    let step = parse_draft(StepKind::Check, &payload, "s0")?;
    if let Step::Check { claim, .. } = &step {
        let mut scope = ValueScope::new(HashMap::new());
        dispatch_check(
            claim,
            &CheckContext {
                session: &session,
                scenario_dir: &scenario_dir,
                run_dir: None,
            },
            &mut scope,
            None,
        )
        .with_context(|| {
            format!("resolved to {role} {name:?} but the live {opts:?} claim failed")
        })?;
    }

    if opts.record {
        match record_draft(&mut state, StepKind::Check, &payload, &session)? {
            Some(row) => println!(
                "recorded {} (step {}) — {role} {name:?} {}",
                row.step_id, row.step_id, opts.predicate
            ),
            None => println!("verified {role} {name:?} — not recorded (recording paused)"),
        }
    } else {
        println!("verified {role} {name:?} {} — not recorded", opts.predicate);
    }
    Ok(0)
}

/// Rung 1: exact role+name in the live snapshot (no plugin needed).
/// Rung 2: the `resolve` plugin picks among interactive candidates.
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
        "agent-qa smart-assert - verify + record an element check from a description\n\nUsage:\n  agent-qa smart-assert \"<description>\"\n                        [--role <role>]      preferred element role\n                        [--predicate <p>]    claim predicate (default: isVisible)\n                        [--no-record]        verify live only, don't record\n                        [--session <name>]\n\nExact role+name matches in the snapshot record directly; fuzzy\ndescriptions go through the configured `resolve` plugin. The recorded\nstep keeps the resolved concrete locator, so replay needs no plugin."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    description: String,
    role: Option<String>,
    predicate: String,
    record: bool,
    session: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut description: Option<String> = None;
    let mut role = None;
    let mut predicate = "isVisible".to_string();
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
            "--predicate" => {
                predicate = it
                    .next()
                    .context("smart-assert: --predicate needs a value")?
                    .clone()
            }
            s if s.starts_with("--predicate=") => predicate = s["--predicate=".len()..].to_string(),
            "--no-record" => record = false,
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if description.is_some() {
                    bail!("unexpected positional {other:?}; usage: smart-assert \"<description>\"");
                }
                description = Some(other.to_string());
            }
        }
    }
    Ok(Opts {
        description: description.ok_or_else(|| anyhow!("usage: smart-assert \"<description>\""))?,
        role,
        predicate,
        record,
        session,
    })
}
