//! `smart-assert` verb — record an element-existence check from a
//! description.
//!
//! Check steps need a concrete role+name or raw locator. An exact role+name
//! in the live snapshot (or a `resolve`-plugin pick) is recorded as
//! role+name. When nothing matches and the description is visible text,
//! the recorded claim is the raw text locator from `references/asserts.md`.
//! The pick is dispatched live before it is appended — replay never needs
//! the plugin.
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

/// Reason carried on the raw text locator. Outcome strings often have no
/// accessible name; this is the shape that replays.
const RAW_TEXT_REASON: &str = "status text has no stable name";

enum Located {
    Role { role: String, name: String },
    RawText { value: String },
}

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let mut state = RecorderState::load_active()?;
    let session = opts
        .session
        .clone()
        .unwrap_or_else(|| state.session.clone());
    let scenario_dir = crate::paths::scenario_dir(&state.sid)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    let located = match resolve_target(&session, &opts)? {
        Some(located) => located,
        None => bail!(
            "could not resolve {:?} to an element{}",
            opts.description,
            resolve_note()
        ),
    };

    let payload = match &located {
        Located::Role { role, name } => json!({
            "intent": format!("smart-assert {}", opts.description),
            "claim": {
                "subject": { "element": { "role": role, "name": name } },
                "predicate": opts.predicate,
            },
        }),
        Located::RawText { value } => json!({
            "intent": format!("smart-assert {}", opts.description),
            "claim": {
                "subject": {
                    "element": {
                        "raw": { "kind": "text", "value": value },
                        "reason": RAW_TEXT_REASON,
                    }
                },
                "predicate": opts.predicate,
            },
        }),
    };
    let step = parse_draft(StepKind::Check, &payload, "s0")?;
    if let Step::Check { claim, .. } = &step {
        let mut scope = ValueScope::new(HashMap::new());
        if let Err(error) = dispatch_check(
            claim,
            &CheckContext {
                session: &session,
                scenario_dir: &scenario_dir,
                run_dir: None,
            },
            &mut scope,
            None,
        ) {
            let message = format!("{error:#}");
            if browser_unreachable(&message) {
                bail!(
                    "browser session {session:?} is not running — live checks need that session: {message}"
                );
            }
            if matches!(located, Located::RawText { .. }) && text_miss(&message) {
                bail!(
                    "could not resolve {:?} to an element — text is not visible on the page{}",
                    opts.description,
                    resolve_note()
                );
            }
            return Err(error).with_context(|| match &located {
                Located::Role { role, name } => {
                    format!("resolved to {role} {name:?} but the live claim failed")
                }
                Located::RawText { value } => {
                    format!("text {value:?} failed the live {} claim", opts.predicate)
                }
            });
        }
    }

    if opts.record {
        match record_draft(&mut state, StepKind::Check, &payload, &session)? {
            Some(row) => match &located {
                Located::Role { role, name } => println!(
                    "recorded {} (step {}) — {role} {name:?} {}",
                    row.step_id, row.step_id, opts.predicate
                ),
                Located::RawText { value } => println!(
                    "recorded {} (step {}) — text {value:?} {}",
                    row.step_id, row.step_id, opts.predicate
                ),
            },
            None => match &located {
                Located::Role { role, name } => {
                    println!("verified {role} {name:?} — not recorded (recording paused)")
                }
                Located::RawText { value } => {
                    println!("verified text {value:?} — not recorded (recording paused)")
                }
            },
        }
    } else {
        match &located {
            Located::Role { role, name } => {
                println!("verified {role} {name:?} {} — not recorded", opts.predicate)
            }
            Located::RawText { value } => {
                println!("verified text {value:?} {} — not recorded", opts.predicate)
            }
        }
    }
    Ok(0)
}

/// Rung 1: exact role+name in the live snapshot (no plugin needed).
/// A `--role` hint matches that role; otherwise an interactive candidate
/// whose accessible name equals the description wins.
/// Rung 2: the `resolve` plugin picks among interactive candidates.
/// Rung 3: visible page text with no role+name match, for `isVisible` /
/// `exists` only — recorded as a raw text locator.
fn resolve_target(session: &str, opts: &Opts) -> Result<Option<Located>> {
    match crate::browser::snapshot_full(session) {
        Ok(snap) => {
            if let Some(hit) = exact_role_name(&snap, opts) {
                return Ok(Some(Located::Role {
                    role: hit.0,
                    name: hit.1,
                }));
            }
        }
        Err(error) => {
            // The page is not observable. A plugin cannot invent a locator
            // for text we cannot see, and a raw-text probe would only
            // repeat this failure.
            bail!(
                "browser session {session:?} is not running — live checks need that session ({error})"
            );
        }
    }
    let pick = match resolve::resolve_element(session, &opts.description, opts.role.as_deref()) {
        Ok(pick) => pick,
        Err(e) => {
            eprintln!("resolve plugin: {e:#} — continuing without it");
            None
        }
    };
    if let Some(ResolvedPick { candidate, .. }) = pick {
        return Ok(Some(Located::Role {
            role: candidate.role,
            name: candidate.name,
        }));
    }
    if matches!(opts.predicate.as_str(), "isVisible" | "exists") {
        return Ok(Some(Located::RawText {
            value: opts.description.clone(),
        }));
    }
    Ok(None)
}

fn exact_role_name(snap: &str, opts: &Opts) -> Option<(String, String)> {
    if let Some(role) = opts.role.as_deref() {
        return crate::browser::find_ref_in_snapshot(snap, role, &opts.description)
            .map(|_| (role.to_string(), opts.description.clone()));
    }
    resolve::candidates_from_snapshot(snap, None, 100)
        .into_iter()
        .find(|candidate| candidate.name == opts.description)
        .map(|candidate| (candidate.role, candidate.name))
}

fn text_miss(message: &str) -> bool {
    message.contains("text not found")
}

fn browser_unreachable(message: &str) -> bool {
    const MARKERS: &[&str] = &[
        "binary not found",
        "All CDP discovery methods failed",
        "Auto-launch failed",
        "spawn `agent-browser`",
        "Failed to read",
        "Connection refused",
        "ECONNREFUSED",
        "is not running",
    ];
    MARKERS.iter().any(|marker| message.contains(marker))
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
        "agent-qa smart-assert - verify + record an element check from a description\n\nUsage:\n  agent-qa smart-assert \"<description>\"\n                        [--role <role>]      preferred element role\n                        [--predicate <p>]    claim predicate (default: isVisible)\n                        [--no-record]        verify live only, don't record\n                        [--session <name>]\n\nExact role+name matches in the snapshot record directly; fuzzy\ndescriptions go through the configured `resolve` plugin. When no\nrole+name matches and the text is visible, an isVisible/exists claim\nrecords a raw text locator. The claim is run live before it is\nappended. Replay needs no plugin."
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

    fn install(tmp: &TempDir, snapshot: &str) {
        let snap_path = tmp.path().join("snap.txt");
        fs::write(&snap_path, snapshot).unwrap();
        let binary = tmp.path().join("agent-browser");
        let script = format!(
            "#!/bin/sh\nif [ \"$3\" = snapshot ]; then cat '{}'; exit 0; fi\nexit 0\n",
            snap_path.display()
        );
        fs::write(&binary, script).unwrap();
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, &binary);
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        std::env::set_var(browser::NO_AUTO_RECOVER_ENV, "1");
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
    }

    fn cleanup() {
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
        std::env::remove_var(browser::NO_AUTO_RECOVER_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn records_raw_text_when_role_name_misses_and_text_is_visible() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        // A nearby button is not a role+name match for the outcome string.
        // Eval (the text probe) succeeds, so the page is showing the text.
        install(
            &tmp,
            "- button \"Delete user\" [ref=e1]\n- StaticText \"User deleted\"\n",
        );
        run(&["User deleted".into()]).unwrap();
        let state = RecorderState::load_active().unwrap();
        let step = serde_json::to_value(&state.steps[0]).unwrap();
        assert_eq!(step["kind"], "check");
        assert_eq!(step["claim"]["predicate"], "isVisible");
        assert_eq!(step["claim"]["subject"]["element"]["raw"]["kind"], "text");
        assert_eq!(
            step["claim"]["subject"]["element"]["raw"]["value"],
            "User deleted"
        );
        assert_eq!(
            step["claim"]["subject"]["element"]["reason"],
            "status text has no stable name"
        );
        assert!(step["claim"]["subject"]["element"].get("role").is_none());
        assert!(step["claim"]["subject"].get("text").is_none());
        cleanup();
    }

    #[test]
    fn records_role_name_when_an_interactive_match_exists() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        install(&tmp, "- button \"Save\" [ref=e4]\n");
        run(&["Save".into()]).unwrap();
        let state = RecorderState::load_active().unwrap();
        let step = serde_json::to_value(&state.steps[0]).unwrap();
        assert_eq!(step["claim"]["subject"]["element"]["role"], "button");
        assert_eq!(step["claim"]["subject"]["element"]["name"], "Save");
        assert!(step["claim"]["subject"]["element"].get("raw").is_none());
        cleanup();
    }
}
