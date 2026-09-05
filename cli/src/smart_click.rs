//! `smart-click` verb — click an element by accessible name and auto-record
//! the matching do/click step.
//!
//! Resolves through native DOM role/name activation first, then
//! agent-browser role/name, text/chunk, and fresh-snapshot-ref fallbacks. A
//! successful dispatch is appended to the active recorder state
//! unless `--no-record` is set. This verb does not verify post-click app state.
//!
//! CLI shape:
//!
//!   agent-qa smart-click "<accessible-name>"
//!                          [--role <role>]      default: 'button'
//!                          [--no-record]
//!                          [--session <name>]

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;

use crate::browser::{self, RoleAct};
use crate::record_step::StepKind;
use crate::recorder_state::RecorderState;

const DEFAULT_ROLE: &str = "button";

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let mut state = RecorderState::load_active()?;
    let session = opts
        .session
        .clone()
        .unwrap_or_else(|| state.session.clone());

    // Probe role+name quietly so the user only sees one clear message
    // when we recover (or one clear failure when we don't).
    let click_outcome = if try_named_control_click(&session, &opts.role, &opts.name)? {
        Ok(())
    } else {
        browser::find_role_act_quiet(&session, &opts.role, &opts.name, RoleAct::Click, None)
    };
    enum Recovery {
        /// Role and accessible name worked first.
        None,
        /// Text or chunked-text fallback worked. Replay uses a raw text locator.
        Text(String),
        /// Snapshot-ref fallback worked. The direct draft keeps the role and
        /// accessible name because the snapshot confirmed both.
        Snapshot,
    }
    let recovery: Recovery = match click_outcome {
        Ok(()) => Recovery::None,
        Err(role_err) => {
            // Ladder rung 2: text + chunked text.
            if let Some(matched) = try_text_fallback(&session, &opts.name) {
                eprintln!(
                    "[smart-click] role+name miss recovered via text {:?}",
                    matched
                );
                Recovery::Text(matched)
            } else if try_snapshot_fallback(&session, &opts.role, &opts.name).is_ok() {
                // Ladder rung 3: parse snapshot for `<role> "<name>" ref=eN`
                // and click @eN. Bypasses the role engine entirely.
                eprintln!(
                    "[smart-click] role+name miss recovered via snapshot ref ({} {:?})",
                    opts.role, opts.name
                );
                Recovery::Snapshot
            } else {
                return Err(anyhow::Error::new(role_err)).with_context(|| {
                    format!(
                        "agent-browser find role {} click --name {} (text + snapshot fallbacks also missed)",
                        opts.role, opts.name
                    )
                });
            }
        }
    };

    if opts.record {
        let payload = match &recovery {
            Recovery::None | Recovery::Snapshot => json!({
                "intent": format!("smart-click {}", opts.name),
                "verb": "click",
                "on": { "role": opts.role, "name": opts.name },
            }),
            Recovery::Text(matched) => json!({
                "intent": format!("smart-click {}", opts.name),
                "verb": "click",
                "on": {
                    "raw": { "kind": "text", "value": matched },
                    "reason": "role and accessible-name lookup missed during recording"
                },
            }),
        };
        let row = crate::record_step::record_draft(&mut state, StepKind::Do, &payload, &session)?;
        let step_id = row.step_id;
        match &recovery {
            Recovery::None => println!(
                "clicked {} (step {step_id}) — role={} name={:?}",
                step_id, opts.role, opts.name
            ),
            Recovery::Text(matched) => println!(
                "clicked {} (step {step_id}) — text={:?} (role+name miss recovered via text)",
                step_id, matched
            ),
            Recovery::Snapshot => println!(
                "clicked {} (step {step_id}) — role={} name={:?} (recovered via snapshot ref)",
                step_id, opts.role, opts.name
            ),
        }
    } else {
        match &recovery {
            Recovery::None => println!("clicked role={} name={:?}", opts.role, opts.name),
            Recovery::Text(matched) => println!(
                "clicked text={:?} (role+name miss recovered via text)",
                matched
            ),
            Recovery::Snapshot => println!(
                "clicked role={} name={:?} (recovered via snapshot ref)",
                opts.role, opts.name
            ),
        }
    }
    Ok(0)
}

/// Try `agent-browser find text <name|chunk> click` when role+name misses.
/// Returns the text that actually matched (so the recorded step can use
/// it verbatim) or None if every attempt missed. Probes are quiet
/// (capture stderr) so a non-recovering miss doesn't pollute the user's
/// console with `✗ Element not found` lines for the speculative tries.
fn try_text_fallback(session: &str, name: &str) -> Option<String> {
    if browser::find_text_act_quiet(session, name, RoleAct::Click, None).is_ok() {
        return Some(name.to_string());
    }
    for chunk in name.split_whitespace() {
        if chunk.len() < 3 {
            continue;
        }
        if browser::find_text_act_quiet(session, chunk, RoleAct::Click, None).is_ok() {
            return Some(chunk.to_string());
        }
    }
    None
}

/// Named interactive controls can be present in the snapshot while
/// agent-browser's coordinate click lands without triggering app handlers
/// (overlay interception, or a mousedown-bound handler). Prefer native DOM
/// activation — role + name resolution, scrollIntoView, and the full
/// pointer/mouse event chain on the node — for a broad set of interactive
/// roles, not just button/link. Non-interactive roles return `Ok(false)` so
/// the caller falls back to agent-browser's role/text/ref ladder. See
/// [`crate::dom_activate`].
fn try_named_control_click(session: &str, role: &str, name: &str) -> Result<bool> {
    if !crate::dom_activate::is_interactive_role(role) {
        return Ok(false);
    }
    crate::dom_activate::activate_role_name(session, role, name)
}

/// Final fallback rung: take a fresh ARIA snapshot, find the line that
/// matches `<role> "<name>" [... ref=eN]`, and click `@eN`. This
/// bypasses agent-browser's role engine and visible-text matcher
/// entirely — if the snapshot promises the element exists with that
/// role+name, the backend-node-id click is guaranteed to land on it.
///
/// Returns Ok(()) on success, Err on snapshot failure / no match / click
/// failure. Used after `find_role_act` and `find_text_act` both miss.
fn try_snapshot_fallback(session: &str, role: &str, name: &str) -> Result<()> {
    let snap =
        browser::snapshot_full(session).with_context(|| "snapshot for ref-based fallback")?;
    let r = browser::find_ref_in_snapshot(&snap, role, name)
        .ok_or_else(|| anyhow!("snapshot has no `{role} \"{name}\" [ref=...]` line"))?;
    browser::click_ref(session, &r).with_context(|| format!("click @{r}"))?;
    Ok(())
}

fn print_help() {
    println!(
        "agent-qa smart-click - click an element by accessible name + auto-record\n\nUsage:\n  agent-qa smart-click \"<accessible-name>\"\n                       [--role <role>]    (default: button)\n                       [--no-record]\n                       [--session <name>]\n\nTries native DOM activation by role + accessible name, then agent-browser\nrole/name, text/chunk, and fresh-snapshot-ref fallbacks. On successful\ndispatch it appends a direct click step.\nIt does not verify post-click app state; record a check when the outcome matters."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    name: String,
    role: String,
    record: bool,
    session: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut name: Option<String> = None;
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
            other => {
                if name.is_some() {
                    bail!("unexpected positional {other:?}; usage: smart-click \"<name>\"");
                }
                name = Some(other.to_string());
            }
        }
    }
    let name = name.ok_or_else(|| anyhow!("usage: smart-click \"<accessible-name>\""))?;
    let role = role.unwrap_or_else(|| DEFAULT_ROLE.to_string());
    Ok(Opts {
        name,
        role,
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
    fn click_records_a_direct_step() {
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
        run(&["Save".into()]).unwrap();
        let state = RecorderState::load_active().unwrap();
        let step = serde_json::to_value(&state.steps[0]).unwrap();
        assert_eq!(step["kind"], "do");
        assert_eq!(step["verb"], "click");
        assert_eq!(step["on"]["role"], "button");
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }
}
