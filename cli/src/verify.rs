//! `verify` verb — sanity-check the in-flight recording buffer.
//!
//! Walks `<record_root>/scenario.steps.jsonl` and confirms:
//!   - every row parses as JSON
//!   - every row carries stepId + kind + payload + recordedAt
//!   - stepIds are sequential (`s0`, `s1`, …)
//!   - if recording sidecars exist for a stepId, both
//!     `<sid>/recording/snapshots/<stepId>.txt` and
//!     `<sid>/recording/screenshots/<stepId>.png` are present (one
//!     without the other is suspicious)
//!
//! Exit 0 on a clean buffer, 1 on any finding. Findings are printed
//! one per line.

use std::fs;

use anyhow::{anyhow, bail, Result};
use serde_json::Value as Json;

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let mut sid_override: Option<String> = None;
    for a in args {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            other if other.starts_with('-') => {
                bail!("verify: unknown flag {other:?}")
            }
            other => {
                if sid_override.is_some() {
                    bail!("verify takes at most one positional <sid>; got extra {other:?}");
                }
                sid_override = Some(other.to_string());
            }
        }
    }
    let findings = verify_with(sid_override.as_deref())?;
    if findings.is_empty() {
        println!("OK  buffer is clean");
        Ok(0)
    } else {
        for f in &findings {
            println!("FAIL {f}");
        }
        Ok(1)
    }
}

fn print_help() {
    println!(
        "agent-qa verify — sanity-check the in-flight recording buffer

Usage:
  agent-qa verify [<sid>]

Resolves the active SID from (in order):
  1. <sid> arg, if supplied
  2. SID= line in tmp/agent-qa-record/scenario.env (in-flight)
  3. tmp/agent-qa-record/scenario.last (last flushed; written by start)

Reports any of:
  - rows that don't parse as JSON
  - rows missing stepId / kind / payload / recordedAt
  - stepId sequence gaps (s0, s1, s2, …)
  - sidecar mismatch (snapshot present without screenshot, or vice versa)"
    );
}

#[cfg(test)]
fn verify() -> Result<Vec<String>> {
    verify_with(None)
}

fn verify_with(sid_override: Option<&str>) -> Result<Vec<String>> {
    let sid = resolve_sid(sid_override)?;

    let steps_file = paths::record_steps_jsonl();
    let body = fs::read_to_string(&steps_file).unwrap_or_default();

    let scenario_dir = paths::scenario_dir(&sid)?;
    let snap_dir = scenario_dir.join("recording").join("snapshots");
    let shot_dir = scenario_dir.join("recording").join("screenshots");

    let mut findings: Vec<String> = Vec::new();
    let mut expected_index: u32 = 0;
    for (lineno, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Json = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                findings.push(format!("line {}: invalid JSON: {e}", lineno + 1));
                continue;
            }
        };
        for required in ["stepId", "kind", "payload", "recordedAt"] {
            if row.get(required).is_none() {
                findings.push(format!("line {}: missing field {required:?}", lineno + 1));
            }
        }
        let kind = row.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
        if !matches!(kind, "do" | "check") {
            findings.push(format!(
                "line {}: kind {kind:?} must be 'do' or 'check'",
                lineno + 1
            ));
        }
        let actual = row
            .get("stepIndex")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32);
        if actual != Some(expected_index) {
            findings.push(format!(
                "line {}: expected stepIndex={expected_index}, got {:?}",
                lineno + 1,
                actual,
            ));
        }
        let step_id = row.get("stepId").and_then(|v| v.as_str()).unwrap_or("?");
        if step_id != format!("s{expected_index}") {
            findings.push(format!(
                "line {}: expected stepId='s{}', got {step_id:?}",
                lineno + 1,
                expected_index,
            ));
        }
        // Sidecar pairing.
        let snap = snap_dir.join(format!("{step_id}.txt"));
        let shot = shot_dir.join(format!("{step_id}.png"));
        match (snap.is_file(), shot.is_file()) {
            (true, true) | (false, false) => {}
            (true, false) => findings.push(format!(
                "step {step_id}: snapshot exists but screenshot missing ({})",
                shot.display()
            )),
            (false, true) => findings.push(format!(
                "step {step_id}: screenshot exists but snapshot missing ({})",
                snap.display()
            )),
        }
        expected_index += 1;
    }

    Ok(findings)
}

/// Pick a SID for verify, in priority order:
///   1. explicit override (e.g. `agent-qa verify <sid>`)
///   2. `SID=` in `<record_root>/scenario.env` (live recording)
///   3. `<record_root>/scenario.last` (last sid stamped by `start`,
///      preserved across `flush` so post-flush `verify` still works)
fn resolve_sid(sid_override: Option<&str>) -> Result<String> {
    if let Some(s) = sid_override {
        let s = s.trim();
        if s.is_empty() {
            bail!("verify: <sid> argument is empty");
        }
        return Ok(s.to_string());
    }
    let env_file = paths::record_env_file();
    if let Ok(body) = fs::read_to_string(&env_file) {
        if let Some(sid) = body
            .lines()
            .find_map(|l| l.strip_prefix("SID=").map(|v| v.trim().to_string()))
            .filter(|s| !s.is_empty())
        {
            return Ok(sid);
        }
    }
    let last_file = paths::record_last_sid_file();
    if let Ok(body) = fs::read_to_string(&last_file) {
        let sid = body.trim();
        if !sid.is_empty() {
            return Ok(sid.to_string());
        }
    }
    Err(anyhow!(
        "no SID found: {} has no SID= line and {} is empty/missing (was `start` run?)",
        env_file.display(),
        last_file.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp);
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.join("rec"));
    }
    fn teardown() {
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    fn write_env(record_root: &Path, sid: &str) {
        fs::create_dir_all(record_root).unwrap();
        fs::write(record_root.join("scenario.env"), format!("SID={sid}\n")).unwrap();
    }
    fn append_row(record_root: &Path, row: serde_json::Value) {
        let path = record_root.join("scenario.steps.jsonl");
        let mut body = fs::read_to_string(&path).unwrap_or_default();
        body.push_str(&serde_json::to_string(&row).unwrap());
        body.push('\n');
        fs::write(&path, body).unwrap();
    }
    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"").unwrap();
    }

    #[test]
    fn verify_clean_buffer_no_findings() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "j1");
        let jdir = tmp.path().join("j1");
        for i in 0..2u32 {
            append_row(
                &rec,
                json!({
                    "stepIndex": i, "stepId": format!("s{i}"), "kind": "do",
                    "payload": { "intent": "x", "verb": "reload" },
                    "recordedAt": "now"
                }),
            );
            touch(&jdir.join("recording/snapshots").join(format!("s{i}.txt")));
            touch(&jdir.join("recording/screenshots").join(format!("s{i}.png")));
        }
        let findings = verify().unwrap();
        assert!(findings.is_empty(), "got: {findings:?}");
        teardown();
    }

    #[test]
    fn verify_flags_index_gap() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "j1");
        // Skip stepIndex=1.
        append_row(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "do",
                "payload": { "intent": "x", "verb": "reload" },
                "recordedAt": "now"
            }),
        );
        append_row(
            &rec,
            json!({
                "stepIndex": 2, "stepId": "s2", "kind": "do",
                "payload": { "intent": "y", "verb": "reload" },
                "recordedAt": "now"
            }),
        );
        let findings = verify().unwrap();
        assert!(
            findings.iter().any(|f| f.contains("expected stepIndex=1")),
            "got: {findings:?}"
        );
        teardown();
    }

    #[test]
    fn verify_flags_orphaned_screenshot() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "j1");
        let jdir = tmp.path().join("j1");
        append_row(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "do",
                "payload": { "intent": "x", "verb": "reload" },
                "recordedAt": "now"
            }),
        );
        // Only screenshot exists, snapshot missing.
        touch(&jdir.join("recording/screenshots").join("s0.png"));
        let findings = verify().unwrap();
        assert!(
            findings.iter().any(|f| f.contains("snapshot missing")),
            "got: {findings:?}"
        );
        teardown();
    }

    #[test]
    fn verify_errors_when_no_env_file() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let err = verify().unwrap_err().to_string();
        assert!(err.contains("scenario.env"), "got: {err}");
        teardown();
    }

    #[test]
    fn verify_falls_back_to_last_sid_after_flush() {
        // Reproduces the post-flush bug: flush() wipes scenario.env, but
        // scenario.last still holds the sid. verify must use it.
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        fs::create_dir_all(&rec).unwrap();
        // Empty env (post-flush state) + populated scenario.last.
        fs::write(rec.join("scenario.env"), b"").unwrap();
        fs::write(rec.join("scenario.last"), "j1\n").unwrap();
        // No steps row -> trivially clean buffer.
        let findings = verify_with(None).unwrap();
        assert!(findings.is_empty(), "got: {findings:?}");
        teardown();
    }

    #[test]
    fn verify_accepts_explicit_sid_argument() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        fs::create_dir_all(&rec).unwrap();
        fs::write(rec.join("scenario.env"), b"").unwrap();
        // No scenario.last either; we rely on the explicit arg.
        let findings = verify_with(Some("j1")).unwrap();
        assert!(findings.is_empty(), "got: {findings:?}");
        teardown();
    }
}
