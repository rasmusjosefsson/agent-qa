//! `flush` verb — turn the in-flight recording buffer into a sealed
//! `scenario.json`.
//!
//! Reads `<record_root>/scenario.env` (set by `start`) and
//! `<record_root>/scenario.steps.jsonl` (appended by `record-step`),
//! assembles a scenario/2 document, validates it against the embedded
//! schema, and writes it to `<scenarios_root>/<sid>/scenario.json`
//! atomically. On success, the recorder workfiles are wiped so the next
//! `start` lands a clean slate.
//!
//! Translation
//!   payload's keys become the step's body. We just inject
//!   `id = row.stepId` and `kind = row.kind`. The schema validator
//!   catches misshapen rows.

use std::fs;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map as JsonMap, Value as Json};

use crate::paths;
use crate::schema;
use crate::sidecar::atomic_write_file;

pub fn run(args: &[String]) -> Result<u8> {
    if let Some(a) = args.iter().next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            other => bail!("flush takes no arguments; got {other:?}"),
        }
    }
    let summary = flush()?;
    println!("flushed sid={} steps={}", summary.sid, summary.steps);
    println!("wrote   {}", summary.scenario_file.display());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa flush — assemble scenario.json from the in-flight buffer

Usage:
  agent-qa flush

Reads:
  <record_root>/scenario.env         (set by `start`)
  <record_root>/scenario.steps.jsonl (appended by `record-step`)

Writes:
  <scenarios_root>/<sid>/scenario.json    atomic; schema-validated

Side effects on success:
  Wipes <record_root>/scenario.env and scenario.steps.jsonl so the next
  `start` lands a clean slate."
    );
}

#[derive(Debug, Clone)]
struct Summary {
    sid: String,
    steps: usize,
    scenario_file: std::path::PathBuf,
}

fn flush() -> Result<Summary> {
    let env_file = paths::record_env_file();
    let env_body = fs::read_to_string(&env_file)
        .with_context(|| format!("read {} (was `start` run?)", env_file.display()))?;
    let sid = extract(&env_body, "SID")
        .ok_or_else(|| anyhow!("{} has no SID= line", env_file.display()))?;
    let intent = extract(&env_body, "INTENT")
        .map(unquote_single)
        .unwrap_or_else(|| "(unintended)".to_string());
    let baseline = extract(&env_body, "BASELINE").unwrap_or_else(|| "FRESH".to_string());

    let steps_file = paths::record_steps_jsonl();
    let body = fs::read_to_string(&steps_file)
        .with_context(|| format!("read {} (run `record-step` first)", steps_file.display()))?;
    let steps = build_steps(&body)?;
    let env_open = env_open_from_baseline(&baseline);
    let mut scenario = json!({
        "schema": "scenario/2",
        "id": sid,
        "intent": intent,
        "steps": steps,
    });
    if let Some(ops) = env_open {
        scenario
            .as_object_mut()
            .expect("object")
            .insert("env".into(), json!({ "open": ops }));
    }

    schema::validate_value(&scenario)
        .with_context(|| "assembled scenario failed schema validation")?;

    let scenario_dir = paths::scenario_dir(&sid)?;
    fs::create_dir_all(&scenario_dir)
        .with_context(|| format!("mkdir -p {}", scenario_dir.display()))?;
    let out = scenario_dir.join("scenario.json");
    let pretty = serde_json::to_string_pretty(&scenario)?;
    let mut bytes = pretty.into_bytes();
    bytes.push(b'\n');
    atomic_write_file(&out, &bytes)?;

    // Wipe the buffer + env file so the next `start` is clean.
    fs::write(&steps_file, b"").ok();
    fs::write(&env_file, b"").ok();

    Ok(Summary {
        sid,
        steps: steps.as_array().map(|a| a.len()).unwrap_or(0),
        scenario_file: out,
    })
}

fn extract(env: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    env.lines()
        .find_map(|l| l.strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

fn unquote_single(s: String) -> String {
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        // Reverse the shell-friendly escape that start.rs applied.
        return inner.replace("'\\''", "'");
    }
    s
}

/// Translate the `BASELINE=` marker `start` wrote into the env.env
/// file into an `env.open[]` array (or `None` for `KEEP_SESSION`).
///
/// The recording session is the only place that knows what the
/// starting baseline was; baking it into the scenario at flush time
/// keeps replay declarative — the runner just executes `env.open[]`
/// before step 0, no flags / heuristics required.
fn env_open_from_baseline(baseline: &str) -> Option<Vec<Json>> {
    let trimmed = baseline.trim();
    if trimmed.eq_ignore_ascii_case("KEEP_SESSION") {
        return None;
    }
    if let Some(name) = trimmed.strip_prefix("PROFILE=") {
        let name = name.trim();
        if name.is_empty() {
            return Some(vec![json!({ "kind": "fresh" })]);
        }
        return Some(vec![json!({ "kind": "useProfile", "name": name })]);
    }
    // Default + unknown values land on "fresh" — the safe baseline.
    Some(vec![json!({ "kind": "fresh" })])
}

fn build_steps(jsonl: &str) -> Result<Json> {
    let mut out: Vec<Json> = Vec::new();
    for (lineno, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Json = serde_json::from_str(line)
            .with_context(|| format!("parse row at line {} ({:?})", lineno + 1, line))?;
        let step =
            row_to_step(&row).with_context(|| format!("row at line {}: build step", lineno + 1))?;
        out.push(step);
    }
    Ok(Json::Array(out))
}

fn row_to_step(row: &Json) -> Result<Json> {
    let step_id = row
        .get("stepId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("row missing stepId"))?;
    let kind = row
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("row missing kind"))?;
    let payload = row
        .get("payload")
        .ok_or_else(|| anyhow!("row missing payload"))?;

    // Friendly trigger shape → scenario/2 schema shape.
    crate::recorder_shape::map_row(kind, payload, step_id)
        .with_context(|| format!("row stepId={step_id}: translate to scenario/2"))
}

#[allow(dead_code)] // legacy JsonMap import kept for downstream call sites
fn _legacy_jsonmap_marker(_: JsonMap<String, Json>) {}

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

    fn write_env(record_root: &Path, sid: &str, intent: &str) {
        fs::create_dir_all(record_root).unwrap();
        fs::write(
            record_root.join("scenario.env"),
            format!("SID={sid}\nINTENT='{intent}'\nSESSION=default\nSTARTED=now\n"),
        )
        .unwrap();
    }
    fn write_row(record_root: &Path, row: serde_json::Value) {
        let path = record_root.join("scenario.steps.jsonl");
        let mut body = fs::read_to_string(&path).unwrap_or_default();
        body.push_str(&serde_json::to_string(&row).unwrap());
        body.push('\n');
        fs::write(&path, body).unwrap();
    }

    #[test]
    fn flush_assembles_validates_and_wipes_buffer() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "mysid", "smoke flow");
        write_row(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "navigation",
                "payload": { "route": "https://x/", "intent": "land" },
                "recordedAt": "now"
            }),
        );
        write_row(
            &rec,
            json!({
                "stepIndex": 1, "stepId": "s1", "kind": "assert",
                "payload": { "kind": "url", "args": ["/x"], "intent": "url ok" },
                "recordedAt": "now"
            }),
        );

        let s = flush().unwrap();
        assert_eq!(s.sid, "mysid");
        assert_eq!(s.steps, 2);

        let body = fs::read_to_string(&s.scenario_file).unwrap();
        let parsed: Json = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["schema"], "scenario/2");
        assert_eq!(parsed["id"], "mysid");
        assert_eq!(parsed["intent"], "smoke flow");
        let steps = parsed["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["id"], "s0");
        assert_eq!(steps[0]["verb"], "goto");
        assert_eq!(steps[1]["id"], "s1");
        assert_eq!(steps[1]["kind"], "check");

        // Wipe verified.
        assert!(fs::read_to_string(rec.join("scenario.steps.jsonl"))
            .unwrap()
            .is_empty());
        assert!(fs::read_to_string(rec.join("scenario.env"))
            .unwrap()
            .is_empty());

        teardown();
    }

    #[test]
    fn flush_errors_when_no_env_file() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let err = flush().unwrap_err().to_string();
        assert!(err.contains("scenario.env"), "got: {err}");
        teardown();
    }

    #[test]
    fn flush_errors_when_assembled_scenario_fails_schema() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "mysid", "x");
        // assert payload without 'intent' — caught by validator on map.
        write_row(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "assert",
                "payload": { "kind": "url", "args": ["/x"] },
                "recordedAt": "now"
            }),
        );
        let err = format!("{:#}", flush().unwrap_err());
        assert!(err.contains("intent"), "got: {err}");
        teardown();
    }

    #[test]
    fn row_to_step_rejects_unknown_kind() {
        let row = json!({ "stepId": "s0", "kind": "wat", "payload": { "intent": "x" } });
        row_to_step(&row).unwrap_err();
    }

    #[test]
    fn unquote_single_reverses_start_escape() {
        assert_eq!(unquote_single("'hello'".into()), "hello");
        assert_eq!(unquote_single("'it'\\''s'".into()), "it's");
        assert_eq!(unquote_single("bare".into()), "bare");
    }

    #[test]
    fn baseline_default_to_fresh_env_open() {
        let ops = env_open_from_baseline("FRESH").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0]["kind"], "fresh");
    }

    #[test]
    fn baseline_keep_session_emits_no_env_open() {
        assert!(env_open_from_baseline("KEEP_SESSION").is_none());
    }

    #[test]
    fn baseline_profile_emits_use_profile_op() {
        let ops = env_open_from_baseline("PROFILE=admin").unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0]["kind"], "useProfile");
        assert_eq!(ops[0]["name"], "admin");
    }

    #[test]
    fn baseline_unknown_value_falls_back_to_fresh() {
        let ops = env_open_from_baseline("wat").unwrap();
        assert_eq!(ops[0]["kind"], "fresh");
    }
}
