//! `record` controls the active recording: pause/resume capture and status.

use anyhow::{bail, Result};
use serde_json::json;

use crate::browser;
use crate::recorder_state::RecorderState;

pub fn run(args: &[String]) -> Result<u8> {
    if args.is_empty()
        || args
            .iter()
            .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        print_help();
        return Ok(0);
    }
    match args[0].as_str() {
        "pause" => set_paused(true, &args[1..]),
        "resume" => set_paused(false, &args[1..]),
        "status" => status(&args[1..]),
        "continue" => continue_scenario(&args[1..]),
        other => bail!("record: unknown subcommand {other:?}; try pause|resume|status|continue"),
    }
}

/// `record continue <sid>` — replay the scenario to its end state in the
/// recording session, then seed the buffer with its steps so newly captured
/// steps append where it left off; `flush` writes the extended scenario back
/// to <sid>. `record pause` + `--skip-replay` when you must hand-drive the
/// browser to the end state instead.
fn continue_scenario(args: &[String]) -> Result<u8> {
    let mut sid: Option<String> = None;
    let mut session = "default".to_string();
    let mut skip_replay = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--skip-replay" => skip_replay = true,
            "--session" => {
                session = it
                    .next()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("--session requires a value"))?
            }
            v if v.starts_with("--session=") => session = v["--session=".len()..].to_string(),
            v if v.starts_with("--") => bail!("record continue: unknown flag {v:?}"),
            v => {
                if sid.is_some() {
                    bail!("unexpected positional {v:?}");
                }
                sid = Some(v.to_string());
            }
        }
    }
    let sid = sid.ok_or_else(|| {
        anyhow::anyhow!("usage: record continue <sid> [--session <n>] [--skip-replay]")
    })?;
    if !skip_replay {
        // Replaying on the recording's own session is exactly what
        // continue wants — opt out of the mid-recording refusal.
        std::env::set_var(crate::runner::RUN_ON_RECORDED_SESSION_ENV, "1");
        let code = crate::runner::cli(&[
            sid.clone(),
            "--session".into(),
            session.clone(),
            "--quiet".into(),
        ]);
        std::env::remove_var(crate::runner::RUN_ON_RECORDED_SESSION_ENV);
        let code = code?;
        if code != 0 {
            bail!(
                "replay of {sid:?} failed (exit {code}) — the browser isn't at the scenario's end state; fix the scenario or pass --skip-replay to hand-drive"
            );
        }
    }
    let steps = crate::buffer::load_into_buffer(&sid, &session, false)?;
    println!("continuing {sid}: {steps} step(s) loaded, session {session:?} at end state — new captures append after s{}", steps.saturating_sub(1));
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa record - pause/resume capture on the active recording

Usage:
  agent-qa record pause
  agent-qa record resume
  agent-qa record status [--json]
  agent-qa record continue <sid> [--session <n>] [--skip-replay]

While paused, record-step / smart-click / fill-unique and the workbench
auto-record hook still run but drop the step — drive the browser into the
state you need without capturing it, then resume. Edit the buffer meanwhile
with `buffer edit`.

`continue` extends an existing scenario: replay it to its end state (same
session), seed the buffer with its steps, keep recording. `flush` writes
the extended scenario back to the same sid."
    );
}

fn set_paused(paused: bool, args: &[String]) -> Result<u8> {
    if !args.is_empty() {
        bail!("usage: record {}", if paused { "pause" } else { "resume" });
    }
    let mut state = RecorderState::load_active()?;
    if state.paused == paused {
        println!("already {}", if paused { "paused" } else { "recording" });
        return Ok(0);
    }
    state.paused = paused;
    state.save()?;
    if paused {
        println!(
            "paused {} — steps are skipped until `record resume`",
            state.sid
        );
    } else {
        println!("resumed {} — capturing steps again", state.sid);
        // Resume on a dead session flips the flag but nothing recaptures —
        // the daemon's capture died with it. Probe (same 3s bound status
        // uses) and say so instead of printing a lie.
        let alive = browser::run(
            &state.session,
            ["--json", "errors"],
            browser::RunOpts::new()
                .capture()
                .lenient()
                .timeout_ms(3_000),
        )
        .is_ok_and(|r| r.exit_code == 0);
        if !alive {
            eprintln!(
                "[v2-record] warning: session {:?} is unreachable — resume can't restart a dead session's capture; `record status` checks liveness, `agent-qa start --force` begins a fresh recording",
                state.session
            );
        }
    }
    Ok(0)
}

fn status(args: &[String]) -> Result<u8> {
    if args.iter().any(|arg| arg != "--json") {
        bail!("usage: record status [--json]");
    }
    let state = RecorderState::load_active()?;
    // Probe the recording browser — a stale buffer flushes fine but
    // silently loses HAR/network capture when the session is gone.
    // Bounded at 3s so a dead daemon doesn't stall the status call.
    let alive = browser::run(
        &state.session,
        ["--json", "errors"],
        browser::RunOpts::new()
            .capture()
            .lenient()
            .timeout_ms(3_000),
    )
    .is_ok_and(|r| r.exit_code == 0);
    if args.iter().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "sid": state.sid,
                "intent": state.intent,
                "session": state.session,
                "sessionAlive": alive,
                "paused": state.paused,
                "steps": state.steps.len(),
                "startedAt": state.started_at,
            }))?
        );
    } else {
        println!(
            "{}: {} step(s), {}{}",
            state.sid,
            state.steps.len(),
            if state.paused { "paused" } else { "recording" },
            if alive {
                String::new()
            } else {
                format!(
                    " — session {:?} unreachable (HAR/network capture lost)",
                    state.session
                )
            },
        );
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    fn seed(paused: bool) -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path().join("record"));
        let mut state = RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        state.paused = paused;
        state.save().unwrap();
        tmp
    }

    fn cleanup() {
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn pause_and_resume_toggle_the_flag() {
        let _guard = lock_env();
        let tmp = seed(false);
        set_paused(true, &[]).unwrap();
        assert!(RecorderState::load_active().unwrap().paused);
        set_paused(false, &[]).unwrap();
        assert!(!RecorderState::load_active().unwrap().paused);
        drop(tmp);
        cleanup();
    }

    #[test]
    fn resume_on_a_dead_session_still_flips_the_flag() {
        let _guard = lock_env();
        let tmp = seed(true);
        // The 3s liveness probe fails fast with no binary — resume warns
        // about the dead session but still succeeds (flag flips).
        std::env::remove_var(crate::browser::BIN_ENV);
        crate::browser::_reset_bin_cache_for_tests();
        set_paused(false, &[]).unwrap();
        assert!(!RecorderState::load_active().unwrap().paused);
        drop(tmp);
        cleanup();
    }

    #[test]
    fn pause_is_idempotent() {
        let _guard = lock_env();
        let tmp = seed(true);
        set_paused(true, &[]).unwrap();
        assert!(RecorderState::load_active().unwrap().paused);
        drop(tmp);
        cleanup();
    }

    #[test]
    fn paused_state_skips_record_draft() {
        let _guard = lock_env();
        let tmp = seed(true);
        let mut state = RecorderState::load_active().unwrap();
        let out = crate::record_step::record_draft(
            &mut state,
            crate::record_step::StepKind::Do,
            &serde_json::json!({"intent":"open","verb":"reload"}),
            "default",
        )
        .unwrap();
        assert!(out.is_none());
        assert!(state.steps.is_empty());
        drop(tmp);
        cleanup();
    }

    /// `record <subverb> --help` prints the record usage instead of the
    /// subverb's flag parser rejecting --help.
    #[test]
    fn help_flag_wins_after_a_subverb() {
        for args in [
            vec!["pause".to_string(), "--help".to_string()],
            vec!["continue".to_string(), "help".to_string()],
        ] {
            assert_eq!(run(&args).unwrap(), 0, "args {args:?}");
        }
    }
}
