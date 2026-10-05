//! The `triage` plugin kind + `agent-qa triage` verb — post-run drift
//! triage delegated to a plugin.
//!
//! A replay produces raw evidence: audit.json, events.jsonl, heal.jsonl,
//! shot/domshot diffs. The `triage` kind hands that evidence to a plugin
//! and gets back a human-ready triage — what's drift vs what's a real
//! bug, what to promote vs what to fix. agent-qa writes the answer to
//! `triage.md` in the run dir and echoes the summary.
//!
//! Request (stdin):
//!   { "scenarioId": "sid", "runId": "...", "exitCode": 1,
//!     "summary": "SUMMARY: 8/9 (FAIL)",
//!     "quarantined": false,
//!     "autoHealed": ["s4"],
//!     "failures": [{"id":"s9","intent":"...","kind":"check","error":"..."}],
//!     "heals":    [ <heal.jsonl rows> ],
//!     "diffs":    ["shots-diff/s1.diff.png", ...],
//!     "runDir":   "/abs/path/to/run" }
//! Response (stdout):
//!   { "summary": "one-line verdict",
//!     "issues":  [{"title": "...", "detail": "..."}, ...] }   // optional
//!
//! No adapter lives in the repo — like every kind, only the protocol and
//! the host do. With no `triage` plugin configured the verb errors with
//! setup instructions.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::paths;
use crate::plugin::{discovery, host};

/// Triage reads real artifacts and may call a model — give it room.
const INVOKE_TIMEOUT: Duration = Duration::from_secs(120);

/// Cap on raw rows inlined into the payload; the plugin always has
/// `runDir` for the full files.
const MAX_ROWS: usize = 50;

pub(crate) const KIND: &str = "triage";

/// Locate the `triage` plugin: declared `[plugins] triage = "<bin>"`
/// first, then any discovered plugin that pings as serving the kind.
fn find_plugin() -> Result<Option<PathBuf>> {
    let plugins = discovery::discover(&discovery::DiscoveryOpts::default())?;
    for p in &plugins {
        if p.declared_kind.as_deref() == Some(KIND) {
            return Ok(Some(p.binary.clone()));
        }
    }
    for p in &plugins {
        if let Ok(pong) = host::ping(&p.binary) {
            if pong.kinds.iter().any(|k| k == KIND) {
                return Ok(Some(p.binary.clone()));
            }
        }
    }
    Ok(None)
}

/// Read a JSONL file into parsed values, silently skipping bad lines and
/// truncating to the last `MAX_ROWS` entries.
fn read_jsonl_tail(path: &Path) -> Vec<Value> {
    let Ok(body) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut rows: Vec<Value> = body
        .lines()
        .filter_map(|l| serde_json::from_str(l.trim()).ok())
        .collect();
    if rows.len() > MAX_ROWS {
        rows.drain(..rows.len() - MAX_ROWS);
    }
    rows
}

/// List files directly under `dir` as `dir-name/name` relative paths.
fn list_files(dir: &Path, prefix: &str) -> Vec<String> {
    let mut out: Vec<String> = match fs::read_dir(dir) {
        Ok(it) => it
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| format!("{prefix}/{}", e.file_name().to_string_lossy()))
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// Assemble the triage payload from a run dir. All artifact reads are
/// best-effort — a partial run still produces a triage-able payload.
fn build_payload(sid: &str, run_id: &str, run_dir: &Path) -> Value {
    let audit: Value = fs::read(run_dir.join("audit.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);

    let failures: Vec<Value> = read_jsonl_tail(&run_dir.join("events.jsonl"))
        .into_iter()
        .filter(|e| e.get("status").and_then(|s| s.as_str()) == Some("fail"))
        .map(|e| {
            json!({
                "id": e.get("id"),
                "intent": e.get("intent"),
                "kind": e.get("kind"),
                "error": e.get("error"),
            })
        })
        .collect();

    let mut diffs = list_files(&run_dir.join("shots-diff"), "shots-diff");
    diffs.extend(list_files(&run_dir.join("domshot-diff"), "domshot-diff"));
    diffs.extend(list_files(&run_dir.join("diffs"), "diffs"));

    json!({
        "scenarioId": sid,
        "runId": run_id,
        "exitCode": audit.get("exitCode"),
        "summary": audit.get("summary"),
        "quarantined": audit.get("quarantined").and_then(|v| v.as_bool()).unwrap_or(false),
        "autoHealed": audit.get("autoHealed").cloned().unwrap_or(json!([])),
        "failures": failures,
        "heals": read_jsonl_tail(&run_dir.join("heal.jsonl")),
        "diffs": diffs,
        "runDir": run_dir.display().to_string(),
    })
}

/// Render the plugin response into `triage.md` content.
fn render_triage_md(sid: &str, run_id: &str, response: &Value) -> String {
    let mut out = format!("# Triage: {sid} / {run_id}\n\n");
    if let Some(s) = response.get("summary").and_then(|v| v.as_str()) {
        out.push_str(s);
        out.push('\n');
    }
    if let Some(issues) = response.get("issues").and_then(|v| v.as_array()) {
        if !issues.is_empty() {
            out.push_str("\n## Issues\n");
            for issue in issues {
                let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                out.push_str(&format!("- **{title}**"));
                if let Some(detail) = issue.get("detail").and_then(|v| v.as_str()) {
                    out.push_str(&format!(" — {detail}"));
                }
                out.push('\n');
            }
        }
    }
    out
}

pub fn run(args: &[String]) -> Result<u8> {
    let mut positionals: Vec<String> = Vec::new();
    let mut json_out = false;
    for a in args {
        match a.as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(0);
            }
            "--json" => json_out = true,
            s if s.starts_with('-') => anyhow::bail!("triage: unknown flag {s}"),
            s => positionals.push(s.to_string()),
        }
    }
    let sid = positionals
        .first()
        .ok_or_else(|| anyhow!("usage: triage <sid> [<runId | latest>]"))?;
    let run_ref = positionals.get(1).map(|s| s.as_str()).unwrap_or("latest");
    let dir = paths::scenario_dir(sid)?;
    let run_id = crate::audit::resolve_run_id(&dir, run_ref)?;
    let run_dir = dir.join("replays").join(&run_id);
    if !run_dir.is_dir() {
        anyhow::bail!("triage: no replay {run_id:?} under {}", dir.display());
    }

    let Some(binary) = find_plugin()? else {
        anyhow::bail!(
            "no `triage` plugin configured — add one to agent-qa.toml:\n  [plugins]\n  triage = \"<plugin-binary>\""
        );
    };

    let payload = build_payload(sid, &run_id, &run_dir);
    let outcome = host::invoke(&binary, KIND, None, payload, INVOKE_TIMEOUT)
        .map_err(|e| anyhow!("triage plugin {}: {e}", binary.display()))?;
    let response = outcome.response;

    let md = render_triage_md(sid, &run_id, &response);
    let md_path = run_dir.join("triage.md");
    fs::write(&md_path, &md).with_context(|| format!("write {}", md_path.display()))?;
    // Machine-readable copy beside the markdown.
    fs::write(
        run_dir.join("triage.json"),
        serde_json::to_string_pretty(&response)?,
    )?;

    if json_out {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if let Some(s) = response.get("summary").and_then(|v| v.as_str()) {
        println!("{s}");
    }
    println!("triage → {}", md_path.display());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa triage - post-run drift triage via the `triage` plugin\n\nUsage:\n  agent-qa triage <sid> [<runId | latest>] [--json]\n\nCollects the run's audit, failures, heal.jsonl rows and diff artifacts,\nhands them to the configured `triage` plugin, writes triage.md +\ntriage.json into the run dir, and prints the plugin's summary.\n\nPlugins are configured in agent-qa.toml: [plugins] triage = \"<binary>\"."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_collects_failures_heals_and_diffs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let run_dir = tmp.path().join("replays").join("r1");
        fs::create_dir_all(run_dir.join("shots-diff")).unwrap();
        fs::write(
            run_dir.join("audit.json"),
            r#"{"exitCode":1,"summary":"SUMMARY: 1/2 (FAIL)","autoHealed":["s1"]}"#,
        )
        .unwrap();
        fs::write(
            run_dir.join("events.jsonl"),
            concat!(
                r#"{"idx":1,"id":"s1","kind":"do:click","status":"pass"}"#,
                "\n",
                r#"{"idx":2,"id":"s2","kind":"check","status":"fail","error":"boom","intent":"check x"}"#,
                "\n"
            ),
        )
        .unwrap();
        fs::write(
            run_dir.join("heal.jsonl"),
            r#"{"stepId":"s1","strategy":"plugin-resolve"}"#.to_string() + "\n",
        )
        .unwrap();
        fs::write(run_dir.join("shots-diff").join("s1.diff.png"), b"png").unwrap();

        let p = build_payload("sid", "r1", &run_dir);
        assert_eq!(p["scenarioId"], "sid");
        assert_eq!(p["exitCode"], 1);
        assert_eq!(p["autoHealed"], json!(["s1"]));
        assert_eq!(p["failures"].as_array().unwrap().len(), 1);
        assert_eq!(p["failures"][0]["id"], "s2");
        assert_eq!(p["heals"].as_array().unwrap().len(), 1);
        assert_eq!(p["diffs"], json!(["shots-diff/s1.diff.png"]));
        assert!(p["runDir"].as_str().unwrap().ends_with("r1"));
    }

    #[test]
    fn payload_tolerates_missing_artifacts() {
        let tmp = tempfile::TempDir::new().unwrap();
        let run_dir = tmp.path().join("replays").join("r1");
        fs::create_dir_all(&run_dir).unwrap();
        let p = build_payload("sid", "r1", &run_dir);
        assert_eq!(p["exitCode"], Value::Null);
        assert!(p["failures"].as_array().unwrap().is_empty());
        assert!(p["heals"].as_array().unwrap().is_empty());
        assert!(p["diffs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn render_includes_summary_and_issues() {
        let md = render_triage_md(
            "sid",
            "r1",
            &json!({
                "summary": "verdict line",
                "issues": [{"title": "T", "detail": "D"}],
            }),
        );
        assert!(md.contains("verdict line"));
        assert!(md.contains("- **T** — D"));
    }
}
