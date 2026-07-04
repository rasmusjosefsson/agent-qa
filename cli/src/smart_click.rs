//! `smart-click` verb — click an element by accessible name and auto-record
//! the matching do/click step.
//!
//! Surface: pure passthrough to `agent-browser find role <r> click
//! --name <n>` plus an append to `<record_root>/scenario.steps.jsonl`.
//! Intent classification, settle-gate polling, and heal-history plumbing
//! land alongside the heal pipeline (separate PR).
//!
//! CLI shape:
//!
//!   agent-qa smart-click "<accessible-name>"
//!                          [--role <role>]      default: 'button'
//!                          [--no-record]
//!                          [--session <name>]

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::browser::{self, RoleAct};
use crate::paths;

const DEFAULT_ROLE: &str = "button";

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let env_file = paths::record_env_file();
    let env_body = fs::read_to_string(&env_file)
        .with_context(|| format!("read {} (was `start` run?)", env_file.display()))?;
    let session = opts
        .session
        .clone()
        .or_else(|| extract(&env_body, "SESSION"))
        .unwrap_or_else(|| "default".to_string());

    // Probe role+name quietly so the user only sees one clear message
    // when we recover (or one clear failure when we don't).
    let click_outcome = if try_named_control_click(&session, &opts.role, &opts.name)? {
        Ok(())
    } else {
        browser::find_role_act_quiet(&session, &opts.role, &opts.name, RoleAct::Click, None)
    };
    enum Recovery {
        /// Role+name worked first try. Record as clickRole.
        None,
        /// Text or chunked-text fallback worked. Record as clickByText
        /// (replay will use the resilient raw.text locator).
        Text(String),
        /// Snapshot ref fallback worked. Record as clickRole anyway —
        /// the snapshot promised this exact (role, name), and replay
        /// will run the same ladder and recover via ref again.
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
        let buffer = paths::record_steps_jsonl();
        let step_index = count_lines(&buffer)? as u32;
        let step_id = format!("s{step_index}");
        let row = match &recovery {
            Recovery::None | Recovery::Snapshot => json!({
                "stepIndex": step_index,
                "stepId": step_id,
                "kind": "action",
                "payload": {
                    "method": "clickRole",
                    "args": [opts.role.clone(), opts.name.clone()],
                    "intent": match &recovery {
                        Recovery::Snapshot => format!(
                            "smart-click {} (role+name miss recovered via snapshot ref)",
                            opts.name
                        ),
                        _ => format!("smart-click {}", opts.name),
                    },
                },
                "recordedAt": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            }),
            Recovery::Text(matched) => json!({
                "stepIndex": step_index,
                "stepId": step_id,
                "kind": "action",
                "payload": {
                    "method": "clickByText",
                    "args": [matched.clone()],
                    "intent": format!(
                        "smart-click {} (role+name miss recovered via text)",
                        opts.name
                    ),
                },
                "recordedAt": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            }),
        };
        append_jsonl(&buffer, &row)?;
        // Keyframe the resulting page so this step shows a screenshot in the
        // recording view, just like record-step does.
        if let Some(sid) = extract(&env_body, "SID") {
            crate::record_step::capture_step_sidecars(&sid, &session, &step_id);
        }
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
/// pointer/mouse event chain on the node — for every interactive role, not
/// just button/link. Non-interactive roles return `Ok(false)` so the caller
/// falls back to agent-browser's role/text/ref ladder. See
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
        "agent-qa smart-click \u{2014} click an element by accessible name + auto-record\n\nUsage:\n  agent-qa smart-click \"<accessible-name>\"\n                       [--role <role>]    (default: button)\n                       [--no-record]\n                       [--session <name>]\n\nResolves the element via agent-browser's `find role <r> click --name <n>`,\nclicks it, and appends a do/click row to scenario.steps.jsonl carrying\nthe role + accessible name. Replay re-resolves the element by the same\nrole+name combination.\n\nIntent classification, settle-gate polling, and heal-history plumbing\nland with the heal pipeline."
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

fn extract(env: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    env.lines()
        .find_map(|l| l.strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

fn count_lines(path: &Path) -> Result<u64> {
    let body = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return Ok(0),
    };
    Ok(body.iter().filter(|&&b| b == b'\n').count() as u64)
}

fn append_jsonl(path: &Path, row: &Json) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    use std::fs::OpenOptions;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let line = serde_json::to_string(row)?;
    f.write_all(line.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser as ab;
    use crate::test_util::lock_env;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn write_exec(dir: &Path, body: &str) -> std::path::PathBuf {
        let p = dir.join("agent-browser");
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn install_fake_logging(dir: &Path, log: &Path) {
        let body = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let bin = write_exec(dir, &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn install_fake_eval_true(dir: &Path, log: &Path) {
        let body = format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nif [ \"$3\" = \"eval\" ]; then echo true; fi\nexit 0\n",
            log.display()
        );
        let bin = write_exec(dir, &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn clear_fake() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    fn write_env(rec: &Path, sid: &str) {
        fs::create_dir_all(rec).unwrap();
        fs::write(
            rec.join("scenario.env"),
            format!("SID={sid}\nSESSION=default\n"),
        )
        .unwrap();
    }

    fn opts(name: &str, role: &str, record: bool) -> Opts {
        Opts {
            name: name.into(),
            role: role.into(),
            record,
            session: None,
        }
    }

    fn run_inner(opts: &Opts) -> Result<()> {
        let env_file = paths::record_env_file();
        let env_body = fs::read_to_string(&env_file)?;
        let session = opts
            .session
            .clone()
            .or_else(|| extract(&env_body, "SESSION"))
            .unwrap_or_else(|| "default".to_string());
        browser::find_role_act(&session, &opts.role, &opts.name, RoleAct::Click, None)?;
        if opts.record {
            let buffer = paths::record_steps_jsonl();
            let step_index = count_lines(&buffer)? as u32;
            let step_id = format!("s{step_index}");
            let row = json!({
                "stepIndex": step_index,
                "stepId": step_id,
                "kind": "action",
                "payload": {
                    "method": "clickRole",
                    "args": [opts.role.clone(), opts.name.clone()],
                    "intent": format!("smart-click {}", opts.name),
                },
                "recordedAt": "now",
            });
            append_jsonl(&buffer, &row)?;
        }
        Ok(())
    }

    #[test]
    fn parse_args_requires_name() {
        parse_args(&["--role".into(), "button".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_default_role_is_button() {
        let opts = parse_args(&["Save".into()]).unwrap();
        assert_eq!(opts.role, "button");
        assert_eq!(opts.name, "Save");
        assert!(opts.record);
    }

    #[test]
    fn parse_args_no_record_flag() {
        let opts = parse_args(&["Save".into(), "--no-record".into()]).unwrap();
        assert!(!opts.record);
    }

    #[test]
    fn click_default_role_button() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        write_env(&rec, "j1");

        run_inner(&opts("Save", "button", true)).unwrap();
        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            invocation.contains("--session default find role button click --name Save"),
            "got: {invocation}"
        );
        let buf = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        let row: Json = serde_json::from_str(buf.lines().next().unwrap()).unwrap();
        assert_eq!(row["kind"], "action");
        assert_eq!(row["payload"]["method"], "clickRole");
        assert_eq!(row["payload"]["args"][0], "button");
        assert_eq!(row["payload"]["args"][1], "Save");

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake();
    }

    #[test]
    fn click_with_role_link_no_record() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        write_env(&rec, "j2");

        run_inner(&opts("Privacy", "link", false)).unwrap();
        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            invocation.contains("--session default find role link click --name Privacy"),
            "got: {invocation}"
        );
        let buf_path = rec.join("scenario.steps.jsonl");
        let buf = fs::read(&buf_path).unwrap_or_default();
        assert!(buf.is_empty(), "no row should be appended with --no-record");

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake();
    }

    #[test]
    fn click_button_prefers_named_control_activation() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        let log = tmp.path().join("ab.log");
        install_fake_eval_true(tmp.path(), &log);
        write_env(&rec, "j3");

        run(&["Add to cart".into()]).unwrap();
        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            invocation.contains("--session default eval"),
            "got: {invocation}"
        );
        assert!(
            !invocation.contains("find role button click --name Add to cart"),
            "got: {invocation}"
        );
        let buf = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        let row: Json = serde_json::from_str(buf.lines().next().unwrap()).unwrap();
        assert_eq!(row["payload"]["method"], "clickRole");
        assert_eq!(row["payload"]["args"][0], "button");
        assert_eq!(row["payload"]["args"][1], "Add to cart");

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake();
    }
}
