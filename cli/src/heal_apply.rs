//! `heal-apply` verb — patch the in-flight recording buffer in place
//! after a `heal-respond` decision.
//!
//! Pure disk: never drives the live tab. Matches the row in
//! `<record_root>/scenario.steps.jsonl` (default: same stepId; override
//! with `--target-step <stepIndex|stepId>`), replaces the recorder-native
//! action value in `payload.args` (or the legacy canonical
//! `payload.value.literal`) with the corrected value carried in the sibling
//! heal-response file, and renames that file `.json` → `.applied.json` so a
//! re-run of the verb can't double-apply.
//!
//! CLI shape:
//!
//!   agent-qa heal-apply <sid> --step <stepId>
//!                              [--target-step <stepIndex|stepId>]
//!                              [--run <runId>]
//!                              [--dry-run]
//!
//! Effects:
//!   - Read <sid>/replays/<runId>/heal-responses/<stepId>.json
//!   - Update tmp/agent-qa-record/scenario.steps.jsonl in place
//!   - Rename the heal-response file to `<stepId>.applied.json`
//!   - Append an audit row to <sid>/recording/heal.jsonl
//!
//! After this, drive the live tab back to the matching state yourself
//! and re-issue `record-step` from the patched index — heal-apply does
//! not orchestrate replay or live navigation.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value as Json;

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let summary = apply(&opts)?;
    println!(
        "heal-apply step={} → row {}",
        summary.step_id, summary.target_index
    );
    if let Some(prev) = &summary.previous_value {
        println!("  before: {prev}");
    }
    println!("  after:  {}", summary.new_value);
    if opts.dry_run {
        println!("(dry-run; pass without --dry-run to write)");
    } else {
        println!("response file: {}", summary.applied_marker.display());
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa heal-apply \u{2014} patch the in-flight recording buffer\n\nUsage:\n  agent-qa heal-apply <sid> --step <stepId>\n                              [--target-step <stepIndex|stepId>]\n                              [--run <runId>]\n                              [--dry-run]\n\nReads <sid>/replays/<runId>/heal-responses/<stepId>.json (set by\n`heal-respond`), patches the matching value-bearing action argument (or\nlegacy payload.value.literal) in <record_root>/scenario.steps.jsonl, renames\nthe response file to <stepId>.applied.json so it can't be re-applied, and appends an audit\nrow to <sid>/recording/heal.jsonl.\n\nNEVER touches the live tab \u{2014} drive it yourself before re-issuing\nrecord-step from the patched index."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    sid: String,
    step_id: String,
    target_step: Option<String>,
    run_id: Option<String>,
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealResponse {
    #[allow(dead_code)]
    step_id: String,
    mode: String,
    value: Option<String>,
    #[allow(dead_code)]
    rationale: Option<String>,
}

#[derive(Debug)]
struct Summary {
    step_id: String,
    target_index: u32,
    previous_value: Option<String>,
    new_value: String,
    applied_marker: PathBuf,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut sid: Option<String> = None;
    let mut step: Option<String> = None;
    let mut target: Option<String> = None;
    let mut run: Option<String> = None;
    let mut dry_run = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--step" => step = it.next().cloned(),
            s if s.starts_with("--step=") => step = Some(s["--step=".len()..].to_string()),
            "--target-step" => target = it.next().cloned(),
            s if s.starts_with("--target-step=") => {
                target = Some(s["--target-step=".len()..].to_string())
            }
            "--run" => run = it.next().cloned(),
            s if s.starts_with("--run=") => run = Some(s["--run=".len()..].to_string()),
            "--dry-run" => dry_run = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!(
                        "unexpected positional {other:?}; usage: heal-apply <sid> --step <stepId>"
                    );
                }
                sid = Some(other.to_string());
            }
        }
    }
    let sid = sid.ok_or_else(|| anyhow!("usage: heal-apply <sid> --step <stepId> [flags]"))?;
    let step_id = step.ok_or_else(|| anyhow!("--step is required"))?;
    Ok(Opts {
        sid,
        step_id,
        target_step: target,
        run_id: run,
        dry_run,
    })
}

fn apply(opts: &Opts) -> Result<Summary> {
    let scenario_dir = paths::scenario_dir(&opts.sid)?;

    let run_id = match &opts.run_id {
        Some(r) => r.clone(),
        None => {
            let latest = scenario_dir.join("replays").join("latest.txt");
            fs::read_to_string(&latest)
                .map(|s| s.trim().to_string())
                .map_err(|_| anyhow!("--run not given and no {} pointer found", latest.display()))?
        }
    };
    if !is_safe_segment(&run_id) {
        bail!("unsafe runId: {run_id:?}");
    }
    if !is_safe_segment(&opts.step_id) {
        bail!("unsafe stepId: {:?}", opts.step_id);
    }

    let response_file = scenario_dir
        .join("replays")
        .join(&run_id)
        .join("heal-responses")
        .join(format!("{}.json", opts.step_id));
    let body = fs::read(&response_file).map_err(|_| {
        anyhow!(
            "no heal-response at {} \u{2014} run `agent-qa heal-respond` first",
            response_file.display()
        )
    })?;
    let response: HealResponse = serde_json::from_slice(&body)
        .with_context(|| format!("parse {}", response_file.display()))?;
    if response.mode != "value-correction" {
        bail!(
            "heal-apply only supports value-correction responses; mode was {:?}",
            response.mode
        );
    }
    let new_value = response
        .value
        .ok_or_else(|| anyhow!("heal-response has no `value` to apply"))?;

    let buffer_path = paths::record_steps_jsonl();
    let buffer_body = fs::read_to_string(&buffer_path)
        .with_context(|| format!("read {} (was `start` run?)", buffer_path.display()))?;
    let mut rows: Vec<Json> = Vec::new();
    for (lineno, line) in buffer_body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Json = serde_json::from_str(line)
            .with_context(|| format!("parse buffer line {}", lineno + 1))?;
        rows.push(row);
    }

    let target_idx = resolve_target(&rows, opts.target_step.as_deref().unwrap_or(&opts.step_id))?;
    let target_row = rows
        .get_mut(target_idx)
        .ok_or_else(|| anyhow!("target index {target_idx} out of range"))?;

    let payload = target_row
        .get_mut("payload")
        .ok_or_else(|| anyhow!("buffer row at index {target_idx} has no payload"))?;
    let previous_value = patch_payload_value(payload, &new_value)
        .with_context(|| format!("patch buffer row {target_idx}"))?;

    let new_buffer: String = rows
        .iter()
        .map(|r| serde_json::to_string(r).expect("row serializes"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut new_buffer = new_buffer;
    if !new_buffer.is_empty() {
        new_buffer.push('\n');
    }

    let applied_marker = response_file.with_file_name(format!("{}.applied.json", opts.step_id));

    if !opts.dry_run {
        fs::write(&buffer_path, new_buffer.as_bytes())
            .with_context(|| format!("write {}", buffer_path.display()))?;
        fs::rename(&response_file, &applied_marker).with_context(|| {
            format!(
                "rename {} -> {}",
                response_file.display(),
                applied_marker.display()
            )
        })?;

        // Audit row.
        let recording = scenario_dir.join("recording");
        fs::create_dir_all(&recording).ok();
        let audit_path = recording.join("heal.jsonl");
        let row = serde_json::json!({
            "schema": "heal-row/v1",
            "ts": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            "runId": run_id,
            "stepId": opts.step_id,
            "mode": "caller-driven-resolved",
            "targetIndex": target_idx,
            "previousValue": previous_value,
            "newValue": new_value,
        });
        use std::fs::OpenOptions;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&audit_path)
            .with_context(|| format!("open {}", audit_path.display()))?;
        let line = serde_json::to_string(&row)?;
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;
    }

    Ok(Summary {
        step_id: opts.step_id.clone(),
        target_index: target_idx as u32,
        previous_value,
        new_value,
        applied_marker,
    })
}

/// Patch the value consumed by `recorder_shape::map_row`.
///
/// Current recording rows use friendly action payloads whose value lives in
/// `args`. The legacy canonical shape is retained as a fallback for buffers
/// produced by older builds and for hand-authored recovery fixtures.
fn patch_payload_value(payload: &mut Json, new_value: &str) -> Result<Option<String>> {
    if let Some(method) = payload
        .get("method")
        .and_then(Json::as_str)
        .map(str::to_string)
    {
        let value_idx = match method.as_str() {
            "fillByLabel" | "fillBySelector" | "uploadBySelector" | "pressSelector"
            | "selectBySelector" => 1,
            "selectByRole" => 2,
            "pressKey" | "navigate" => 0,
            other => bail!("recorded action method {other:?} has no correctable string value"),
        };
        let args = payload
            .get_mut("args")
            .and_then(Json::as_array_mut)
            .ok_or_else(|| anyhow!("recorded action {method:?} has no args array"))?;
        let slot = args.get_mut(value_idx).ok_or_else(|| {
            anyhow!("recorded action {method:?} has no value at args[{value_idx}]")
        })?;
        let previous = match &*slot {
            Json::String(s) => Some(s.clone()),
            Json::Null => None,
            other => Some(other.to_string()),
        };
        *slot = Json::String(new_value.to_string());
        return Ok(previous);
    }

    let value = payload
        .get_mut("value")
        .and_then(Json::as_object_mut)
        .ok_or_else(|| {
            anyhow!("payload is neither a recorded action nor a canonical value step")
        })?;
    let previous = value
        .get("literal")
        .and_then(Json::as_str)
        .map(str::to_string);
    value.insert("from".into(), Json::String("literal".to_string()));
    value.insert("literal".into(), Json::String(new_value.to_string()));
    Ok(previous)
}

fn resolve_target(rows: &[Json], hint: &str) -> Result<usize> {
    // Numeric hint → that row index.
    if let Ok(idx) = hint.parse::<usize>() {
        if idx < rows.len() {
            return Ok(idx);
        }
        bail!(
            "target stepIndex {idx} out of range (buffer has {} rows)",
            rows.len()
        );
    }
    // Otherwise: find the row whose stepId matches.
    for (i, row) in rows.iter().enumerate() {
        if row.get("stepId").and_then(|v| v.as_str()) == Some(hint) {
            return Ok(i);
        }
    }
    bail!("no buffer row matches stepId {hint:?}")
}

fn is_safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
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

    fn write_response(jdir: &Path, run: &str, step: &str, value: Option<&str>, mode: &str) {
        let dir = jdir.join("replays").join(run).join("heal-responses");
        fs::create_dir_all(&dir).unwrap();
        let mut obj = serde_json::Map::new();
        obj.insert("stepId".into(), json!(step));
        obj.insert("mode".into(), json!(mode));
        if let Some(v) = value {
            obj.insert("value".into(), json!(v));
        }
        obj.insert("recordedAt".into(), json!("now"));
        fs::write(
            dir.join(format!("{step}.json")),
            serde_json::to_string_pretty(&Json::Object(obj)).unwrap(),
        )
        .unwrap();
    }

    fn append_buffer(rec: &Path, row: serde_json::Value) {
        let p = rec.join("scenario.steps.jsonl");
        let mut body = fs::read_to_string(&p).unwrap_or_default();
        body.push_str(&serde_json::to_string(&row).unwrap());
        body.push('\n');
        fs::create_dir_all(rec).unwrap();
        fs::write(&p, body).unwrap();
    }

    #[test]
    fn apply_patches_recorded_fill_value_and_flushes_correction() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let rec = tmp.path().join("rec");
        append_buffer(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "action",
                "payload": { "method": "fillByLabel", "args": ["Email", "old@e.com"],
                  "intent": "fill email" },
                "recordedAt": "now"
            }),
        );
        write_response(&jdir, "rA", "s0", Some("new@e.com"), "value-correction");

        let s = apply(&Opts {
            sid: "j1".into(),
            step_id: "s0".into(),
            target_step: None,
            run_id: Some("rA".into()),
            dry_run: false,
        })
        .unwrap();
        assert_eq!(s.previous_value.as_deref(), Some("old@e.com"));
        assert_eq!(s.new_value, "new@e.com");

        // The recorder-native args value is what flush consumes.
        let body = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        let row: Json = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(row["payload"]["args"][1], "new@e.com");

        // Seal the buffer and prove the corrected value reaches scenario.json.
        fs::write(
            rec.join("scenario.env"),
            "SID=j1\nINTENT='heal apply integration'\nSESSION=default\nBASELINE=KEEP_SESSION\n",
        )
        .unwrap();
        crate::flush::run(&[]).unwrap();
        let scenario: Json =
            serde_json::from_slice(&fs::read(jdir.join("scenario.json")).unwrap()).unwrap();
        assert_eq!(scenario["steps"][0]["value"]["literal"], "new@e.com");

        // Response renamed to .applied.json and audit row appended.
        assert!(jdir
            .join("replays/rA/heal-responses/s0.applied.json")
            .is_file());
        assert!(!jdir.join("replays/rA/heal-responses/s0.json").exists());
        let audit = fs::read_to_string(jdir.join("recording/heal.jsonl")).unwrap();
        assert!(audit.contains("\"mode\":\"caller-driven-resolved\""));
        assert!(audit.contains("\"newValue\":\"new@e.com\""));

        teardown();
    }

    #[test]
    fn dry_run_does_not_modify_buffer_or_rename_response() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let rec = tmp.path().join("rec");
        append_buffer(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "action",
                "payload": { "method": "fillByLabel", "args": ["Email", "old@e.com"],
                  "intent": "fill email" },
                "recordedAt": "now"
            }),
        );
        write_response(&jdir, "rA", "s0", Some("new@e.com"), "value-correction");
        apply(&Opts {
            sid: "j1".into(),
            step_id: "s0".into(),
            target_step: None,
            run_id: Some("rA".into()),
            dry_run: true,
        })
        .unwrap();
        // Buffer untouched.
        let body = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        assert!(body.contains("old@e.com"));
        // Response file still in place.
        assert!(jdir.join("replays/rA/heal-responses/s0.json").is_file());
        teardown();
    }

    #[test]
    fn target_step_overrides_default_match() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let rec = tmp.path().join("rec");
        // Two rows; response says "s0" but we override to target s1.
        append_buffer(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "action",
                "payload": { "method": "fillByLabel", "args": ["Email", "untouched"] },
                "recordedAt": "now"
            }),
        );
        append_buffer(
            &rec,
            json!({
                "stepIndex": 1, "stepId": "s1", "kind": "action",
                "payload": { "method": "fillByLabel", "args": ["Email", "old"] },
                "recordedAt": "now"
            }),
        );
        write_response(&jdir, "rA", "s0", Some("new"), "value-correction");
        let s = apply(&Opts {
            sid: "j1".into(),
            step_id: "s0".into(),
            target_step: Some("s1".into()),
            run_id: Some("rA".into()),
            dry_run: false,
        })
        .unwrap();
        assert_eq!(s.target_index, 1);
        let body = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        let r0: Json = serde_json::from_str(lines[0]).unwrap();
        let r1: Json = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(r0["payload"]["args"][1], "untouched");
        assert_eq!(r1["payload"]["args"][1], "new");
        teardown();
    }

    #[test]
    fn apply_errors_on_reject_response() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let rec = tmp.path().join("rec");
        append_buffer(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "action",
                "payload": { "method": "fillByLabel", "args": ["Email", "v"] },
                "recordedAt": "now"
            }),
        );
        write_response(&jdir, "rA", "s0", None, "reject");
        let err = apply(&Opts {
            sid: "j1".into(),
            step_id: "s0".into(),
            target_step: None,
            run_id: Some("rA".into()),
            dry_run: false,
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("value-correction"), "got: {err}");
        teardown();
    }

    #[test]
    fn apply_rejects_non_value_action_without_consuming_response() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let rec = tmp.path().join("rec");
        append_buffer(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "action",
                "payload": { "method": "clickRole", "args": ["button", "Save"] },
                "recordedAt": "now"
            }),
        );
        let buffer = rec.join("scenario.steps.jsonl");
        let before = fs::read_to_string(&buffer).unwrap();
        write_response(&jdir, "rA", "s0", Some("new"), "value-correction");

        let err = apply(&Opts {
            sid: "j1".into(),
            step_id: "s0".into(),
            target_step: None,
            run_id: Some("rA".into()),
            dry_run: false,
        })
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(
            message.contains("no correctable string value"),
            "got: {message}"
        );
        assert_eq!(fs::read_to_string(&buffer).unwrap(), before);
        assert!(jdir.join("replays/rA/heal-responses/s0.json").is_file());
        assert!(!jdir
            .join("replays/rA/heal-responses/s0.applied.json")
            .exists());

        teardown();
    }

    #[test]
    fn parse_args_requires_step() {
        parse_args(&["j1".into()]).unwrap_err();
    }
}
