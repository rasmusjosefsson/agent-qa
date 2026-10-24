//! `verify` checks the active recorder state, its recording sidecars, and
//! every live `kind:check` step against the browser session named in that
//! state.
//!
//! `record-step check` still only validates claim shape and appends — it
//! does not drive the browser. `verify` is the gate that runs those claims
//! through [`dispatch_check`](crate::claims::dispatch_check) before `flush`.
//! A claim that is already false fails here with the same error replay
//! would report. Shot, domshot, and layout claims compare replay artifacts,
//! so they are not dispatched; every other enabled check is.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{bail, Result};

use crate::claims::{dispatch_check, CheckContext};
use crate::recorder_state::RecorderState;
use crate::scenario::{ClaimSubject, Step};
use crate::value::ValueScope;

pub fn run(args: &[String]) -> Result<u8> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "help"))
    {
        print_help();
        return Ok(0);
    }
    let fix = args.iter().any(|a| a == "--fix");
    if args.iter().any(|a| a != "--fix") {
        bail!("verify takes no arguments (or --fix)");
    }
    if fix {
        return fix_ids();
    }
    let findings = verify()?;
    if findings.is_empty() {
        println!("OK  recorder state is clean");
        Ok(0)
    } else {
        for finding in findings {
            println!("FAIL {finding}");
        }
        Ok(1)
    }
}

/// `verify --fix` — renumber the active recording to dense s0.. ids, rewire
/// `{"from":"step","stepId":…}` references, and move the recording sidecars
/// (screenshots/snapshots) so the pairing survives the renumber.
fn fix_ids() -> Result<u8> {
    let mut state = RecorderState::load_active()?;
    let renames = crate::buffer::normalize_ids(&mut state.steps);
    if renames.is_empty() {
        println!("OK  step ids already dense — nothing to fix");
        return Ok(0);
    }
    let scenario_dir = crate::paths::scenario_dir(&state.sid)?;
    // Two-phase rename so swaps (s1↔s3) can't clobber each other.
    for kind in ["screenshots", "snapshots"] {
        let dir = scenario_dir.join("recording").join(kind);
        let ext = if kind == "screenshots" { "png" } else { "txt" };
        for (old, new) in &renames {
            let from = dir.join(format!("{old}.{ext}"));
            if from.is_file() {
                std::fs::rename(&from, dir.join(format!(".renaming-{new}.{ext}")))?;
            }
        }
        for new in renames.values() {
            let tmp = dir.join(format!(".renaming-{new}.{ext}"));
            if tmp.is_file() {
                std::fs::rename(&tmp, dir.join(format!("{new}.{ext}")))?;
            }
        }
    }
    state.save()?;
    println!(
        "FIXED {} step id(s) renumbered to dense s0..",
        renames.len()
    );
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa verify - check the active recording

Usage:
  agent-qa verify [--fix]

Checks dense step ids, paired snapshot and screenshot sidecars, and
every enabled check step against the live browser session named in the
recorder state. A claim that is already false fails with the same
reason replay would report. With no check steps, no browser is required.
Check steps whose session is not running fail verify.
--fix renumbers the active recording to dense s0.. ids, rewiring
{{\"from\":\"step\"}} references and moving sidecar files to match.
It does not evaluate claims."
    );
}

fn verify() -> Result<Vec<String>> {
    let state = RecorderState::load_active()?;
    let scenario_dir = crate::paths::scenario_dir(&state.sid)?;
    let mut findings = Vec::new();
    for (index, step) in state.steps.iter().enumerate() {
        let expected = format!("s{index}");
        if step.id() != expected {
            findings.push(format!(
                "step {index}: expected id {expected:?}, got {:?}",
                step.id()
            ));
        }
        let screenshot = scenario_dir
            .join("recording/screenshots")
            .join(format!("{}.png", step.id()));
        let snapshot = scenario_dir
            .join("recording/snapshots")
            .join(format!("{}.txt", step.id()));
        match (screenshot.is_file(), snapshot.is_file()) {
            (true, true) | (false, false) => {}
            (true, false) => findings.push(format!(
                "step {}: screenshot exists but snapshot is missing",
                step.id()
            )),
            (false, true) => findings.push(format!(
                "step {}: snapshot exists but screenshot is missing",
                step.id()
            )),
        }
    }
    findings.extend(prove_live_checks(&state, &scenario_dir)?);
    Ok(findings)
}

fn enabled_check(step: &Step) -> bool {
    match step {
        Step::Check {
            enabled: Some(false),
            ..
        } => false,
        Step::Check { .. } => true,
        Step::Do { .. } => false,
    }
}

/// Shot, domshot, and layout claims compare files from a replay run. They
/// have no live page result, so dispatching them here would fail every
/// recording that includes one. The session probe still runs when any
/// enabled check is present.
fn dispatches_on_page(step: &Step) -> bool {
    match step {
        Step::Check { claim, .. } => !matches!(
            claim.subject,
            ClaimSubject::Shot { .. } | ClaimSubject::Domshot { .. } | ClaimSubject::Layout { .. }
        ),
        Step::Do { .. } => false,
    }
}

fn prove_live_checks(state: &RecorderState, scenario_dir: &std::path::Path) -> Result<Vec<String>> {
    let enabled: Vec<&Step> = state
        .steps
        .iter()
        .filter(|step| enabled_check(step))
        .collect();
    if enabled.is_empty() {
        return Ok(Vec::new());
    }
    if let Err(error) = crate::browser::snapshot_full(&state.session) {
        return Ok(vec![format!(
            "browser session {:?} is not running — live check steps need that session ({error})",
            state.session
        )]);
    }
    let mut scope = ValueScope::new(HashMap::new());
    let ctx = CheckContext {
        session: &state.session,
        scenario_dir,
        run_dir: None,
    };
    let mut findings = Vec::new();
    for step in enabled.into_iter().filter(|step| dispatches_on_page(step)) {
        let Step::Check { claim, context, .. } = step else {
            continue;
        };
        let timeout = context
            .as_ref()
            .and_then(|c| c.timeout_ms)
            .map(Duration::from_millis);
        if let Err(error) = dispatch_check(claim, &ctx, &mut scope, timeout) {
            let reason = format!("{error:#}")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            findings.push(format!("step {}: {reason}", step.id()));
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::{self, BrowserConnection};
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn recording_env(tmp: &TempDir) {
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var(browser::NO_AUTO_RECOVER_ENV, "1");
    }

    fn clear_recording_env() {
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::NO_AUTO_RECOVER_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    fn install_browser(dir: &std::path::Path, body: &str) {
        let binary = dir.join("agent-browser");
        fs::write(&binary, body).unwrap();
        #[cfg(unix)]
        {
            let mut permissions = fs::metadata(&binary).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&binary, permissions).unwrap();
        }
        std::env::set_var(browser::BIN_ENV, &binary);
        browser::_reset_bin_cache_for_tests();
    }

    fn fresh_state(sid: &str) -> RecorderState {
        RecorderState::new(
            sid.into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        )
    }

    #[test]
    fn verify_flags_non_dense_step_ids() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        recording_env(&tmp);
        let mut state = fresh_state("s1");
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s3","intent":"reload","kind":"do","verb":"reload"}
        ]))
        .unwrap();
        state.save().unwrap();
        assert!(verify()
            .unwrap()
            .iter()
            .any(|finding| finding.contains("expected id")));
        clear_recording_env();
    }

    #[test]
    fn fix_renumbers_and_moves_sidecars() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        recording_env(&tmp);
        // The rewired check is evaluated live. isVisible ignores `value`,
        // so the from-step reference stays in the draft for the rewire
        // assertion while the stubbed page satisfies the predicate.
        install_browser(tmp.path(), "#!/bin/sh\nexit 0\n");
        let mut state = fresh_state("sid1");
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s1","intent":"open","kind":"do","verb":"goto","value":{"from":"literal","literal":"https://example.com"}},
            {"id":"s5","intent":"reuse","kind":"check","claim":{"subject":{"element":{"raw":{"kind":"text","value":"Ready"},"reason":"t"}},"predicate":"isVisible","value":{"from":"step","stepId":"s1"}}}
        ]))
        .unwrap();
        state.save().unwrap();
        let dir = tmp.path().join("sid1/recording");
        std::fs::create_dir_all(dir.join("screenshots")).unwrap();
        std::fs::create_dir_all(dir.join("snapshots")).unwrap();
        std::fs::write(dir.join("screenshots/s5.png"), b"PNG").unwrap();
        std::fs::write(dir.join("snapshots/s5.txt"), b"SNAP").unwrap();

        assert_eq!(fix_ids().unwrap(), 0);
        // s1→s0, s5→s1; the step ref rewires to the new id; sidecars moved.
        let after = RecorderState::load_active().unwrap();
        let ids: Vec<String> = after.steps.iter().map(|s| s.id().to_string()).collect();
        assert_eq!(ids, ["s0", "s1"]);
        let check = serde_json::to_value(&after.steps[1]).unwrap();
        assert_eq!(check["claim"]["value"]["stepId"], "s0"); // old s1 → new s0
        assert!(dir.join("screenshots/s1.png").is_file());
        assert!(dir.join("snapshots/s1.txt").is_file());
        assert!(verify().unwrap().is_empty());
        clear_recording_env();
    }

    #[test]
    fn verify_fails_a_check_when_dispatch_check_fails() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        recording_env(&tmp);
        let log = tmp.path().join("browser.log");
        install_browser(
            tmp.path(),
            &format!(
                "#!/bin/sh\necho \"$3\" >> '{}'\nif [ \"$3\" = snapshot ]; then exit 0; fi\nif [ \"$3\" = eval ]; then echo 'text not found: Editing Bea' 1>&2; exit 1; fi\nexit 0\n",
                log.display()
            ),
        );
        let mut state = fresh_state("sid-fail");
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"open","kind":"do","verb":"reload"},
            {"id":"s1","intent":"editing bea","kind":"check","context":{"timeoutMs":250},"claim":{"subject":{"element":{"raw":{"kind":"text","value":"Editing Bea"},"reason":"status text has no stable name"}},"predicate":"isVisible"}}
        ]))
        .unwrap();
        state.save().unwrap();
        let findings = verify().unwrap();
        assert!(
            findings.iter().any(|finding| {
                finding.contains("step s1") && finding.contains("text not found: Editing Bea")
            }),
            "findings: {findings:?}"
        );
        assert!(
            findings
                .iter()
                .all(|finding| !finding.contains("not running")),
            "a failed claim is not a dead session: {findings:?}"
        );
        clear_recording_env();
    }

    #[test]
    fn verify_does_not_require_a_browser_when_there_are_no_checks() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        recording_env(&tmp);
        let log = tmp.path().join("browser.log");
        install_browser(
            tmp.path(),
            &format!("#!/bin/sh\necho called >> '{}'\nexit 1\n", log.display()),
        );
        let mut state = fresh_state("sid-do");
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"reload","kind":"do","verb":"reload"}
        ]))
        .unwrap();
        state.save().unwrap();
        assert!(
            verify().unwrap().is_empty(),
            "ids and sidecars are enough when the buffer has no checks"
        );
        assert!(
            !log.exists(),
            "verify must not spawn a browser when there are no check steps"
        );
        clear_recording_env();
    }

    #[test]
    fn verify_fails_when_check_steps_have_no_running_session() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        recording_env(&tmp);
        let log = tmp.path().join("browser.log");
        install_browser(
            tmp.path(),
            &format!(
                "#!/bin/sh\necho \"$3\" >> '{}'\necho 'All CDP discovery methods failed' 1>&2\nexit 1\n",
                log.display()
            ),
        );
        let mut state = fresh_state("sid-down");
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"status","kind":"check","claim":{"subject":{"element":{"role":"status"}},"predicate":"isVisible"}}
        ]))
        .unwrap();
        state.save().unwrap();
        let findings = verify().unwrap();
        assert!(
            findings
                .iter()
                .any(|finding| finding.contains("is not running")),
            "findings: {findings:?}"
        );
        let calls = fs::read_to_string(&log).unwrap_or_default();
        assert!(
            !calls.contains("eval") && !calls.contains("find"),
            "a dead session stops before claim dispatch, got {calls:?}"
        );
        clear_recording_env();
    }
}
