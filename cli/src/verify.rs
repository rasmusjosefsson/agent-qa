//! `verify` checks the active recorder state and its recording sidecars.

use anyhow::{bail, Result};

use crate::recorder_state::RecorderState;

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

Checks dense step ids and paired snapshot and screenshot sidecars.
--fix renumbers the active recording to dense s0.. ids, rewiring
{{\"from\":\"step\"}} references and moving sidecar files to match."
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
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    #[test]
    fn verify_flags_non_dense_step_ids() {
        let _guard = lock_env();
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
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s3","intent":"reload","kind":"do","verb":"reload"}
        ]))
        .unwrap();
        state.save().unwrap();
        assert!(verify()
            .unwrap()
            .iter()
            .any(|finding| finding.contains("expected id")));
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn fix_renumbers_and_moves_sidecars() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path().join("record"));
        let mut state = RecorderState::new(
            "sid1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s1","intent":"open","kind":"do","verb":"goto","value":{"from":"literal","literal":"https://example.com"}},
            {"id":"s5","intent":"reuse","kind":"check","claim":{"subject":{"url":true},"predicate":"contains","value":{"from":"step","stepId":"s1"}}}
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
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }
}
