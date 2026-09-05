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
    if !args.is_empty() {
        bail!("verify takes no arguments");
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

fn print_help() {
    println!(
        "agent-qa verify - check the active recording

Usage:
  agent-qa verify

Checks direct step ids and paired snapshot and screenshot sidecars."
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
}
