//! `record-step` verb — append one step to the in-flight buffer.
//!
//! Author-friendly trigger API: callers pass one of four kinds with a
//! small payload shape; `flush` translates each row into a scenario/2
//! `do` / `check` step via [`crate::recorder_shape::map_row`]. The
//! allow-lists in `recorder_shape` reject typos at record time so the
//! offending payload is still in the caller's scrollback when it fails.
//!
//! CLI shape:
//!
//!   agent-qa record-step <navigation|action|wait|assert> <payload-json>
//!
//! Each call appends one JSONL row shaped as:
//!
//!   {
//!     "stepIndex": 0,
//!     "stepId": "s0",
//!     "kind": "navigation",
//!     "payload": { …caller JSON… },
//!     "recordedAt": "<isots>"
//!   }
//!
//! No live-DOM validation, no smart-click integration, no heal-history
//! plumbing yet — those land alongside the matching verbs. `record-step`
//! does NOT drive the page; it only records intent and writes the
//! ARIA snapshot + screenshot sidecars under `<sid>/recording/`. The
//! caller (or `smart-click`/`fill-unique`) drives the browser.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::browser;
use crate::paths;
use crate::recorder_shape::{validate_trigger, TriggerKind};

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let row = record(&opts)?;
    println!(
        "recorded step {} (stepId={}, kind={})",
        row.step_index, row.step_id, row.kind
    );
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa record-step — append one step to the in-flight buffer

Usage:
  agent-qa record-step <navigation|action|wait|assert> <payload-json>

Kinds and payload shapes:
  navigation   {{\"route\":\"<url>\"}}
  action       {{\"method\":\"<m>\",\"args\":[...]}}
  wait         {{\"condition\":{{\"kind\":\"<duration|selector|selectorAbsent|selectorText|text|url>\",...}}}}
  assert       {{\"kind\":\"<present|absent|url>\",\"args\":[...],\"intent\":\"...\"}}

Allow-listed action methods:
  clickRole, clickByText, clickByLabel, clickSelector,
  focusSelector, fillByLabel, fillBySelector, uploadBySelector, pressKey, pressSelector, submit, selectBySelector, selectByRole,
  scrollIntoViewByText, navigate

Examples:
  agent-qa record-step navigation '{{\"route\":\"https://example.com/\"}}'
  agent-qa record-step action     '{{\"method\":\"clickRole\",\"args\":[\"link\",\"Sign in\"]}}'
  agent-qa record-step action     '{{\"method\":\"fillByLabel\",\"args\":[\"Email\",\"a@b.com\"]}}'
  agent-qa record-step wait       '{{\"condition\":{{\"kind\":\"url\",\"pattern\":\"/dashboard\"}}}}'
  agent-qa record-step assert     '{{\"kind\":\"url\",\"args\":[\"/dashboard\"],\"intent\":\"signed in\"}}'

`record-step` only RECORDS the step; it does not drive the browser.
Drive page interactions with `agent-qa smart-click \"<accessible name>\"`,
`agent-qa fill-unique`, or `agent-browser <verb>` and call `record-step`
immediately after the action settles.

Side effects per call:
  - Append one JSONL row to <record_root>/scenario.steps.jsonl
  - Best-effort capture to <sid>/recording/snapshots/<stepId>.txt
  - Best-effort capture to <sid>/recording/screenshots/<stepId>.png
"
    );
}

#[derive(Debug, Clone)]
struct Opts {
    kind: TriggerKind,
    payload: Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // retained for any downstream consumer; record-step now uses TriggerKind
pub enum StepKind {
    Do,
    Check,
}

impl StepKind {
    #[cfg(test)]
    #[allow(dead_code)]
    fn as_str(self) -> &'static str {
        match self {
            StepKind::Do => "do",
            StepKind::Check => "check",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StepRow {
    pub step_index: u32,
    pub step_id: String,
    pub kind: String,
    pub payload: Json,
    pub recorded_at: String,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        print_help();
        std::process::exit(0);
    }
    if args.len() < 2 {
        bail!("usage: record-step <navigation|action|wait|assert> <payload-json>");
    }
    let kind = TriggerKind::parse(&args[0])?;
    let payload: Json = serde_json::from_str(&args[1])
        .with_context(|| format!("parse payload JSON: {:?}", args[1]))?;
    validate_trigger(kind, &payload)
        .with_context(|| format!("record-step {}: invalid payload", kind.as_str()))?;
    Ok(Opts { kind, payload })
}

fn record(opts: &Opts) -> Result<StepRow> {
    // Resolve the active sid from scenario.env (set by `start`).
    let env_file = paths::record_env_file();
    let env_body = fs::read_to_string(&env_file)
        .with_context(|| format!("read {} (was `start` run?)", env_file.display()))?;
    let sid = parse_sid(&env_body).ok_or_else(|| {
        anyhow!(
            "{} has no SID= line; run `agent-qa start` first",
            env_file.display()
        )
    })?;
    let session = parse_session(&env_body).unwrap_or_else(|| "default".into());

    // Determine next step index by counting lines in the existing buffer.
    let buf_path = paths::record_steps_jsonl();
    let step_index = count_lines(&buf_path).unwrap_or(0) as u32;
    let step_id = format!("s{step_index}");

    let row = StepRow {
        step_index,
        step_id: step_id.clone(),
        kind: opts.kind.as_str().to_string(),
        payload: opts.payload.clone(),
        recorded_at: chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
    };

    // Append (atomic enough for a single short line; create if missing).
    append_jsonl(&buf_path, &row)?;

    // Best-effort sidecar capture under <sid>/recording/.
    // Evals can disable this so a slow snapshot/screenshot never blocks
    // recording the scenario contract itself.
    capture_step_sidecars(&sid, &session, &row.step_id);

    Ok(row)
}

/// Best-effort keyframe capture for a freshly recorded step. Shared by
/// `record-step`, `smart-click`, and `fill-unique` so every recorded step —
/// however it was driven — gets a matching
/// `<sid>/recording/{snapshots,screenshots}/<stepId>.{txt,png}` keyframe.
/// No-ops when AGENT_QA_RECORD_SKIP_SIDECARS=1 (evals).
pub(crate) fn capture_step_sidecars(sid: &str, session: &str, step_id: &str) {
    if std::env::var("AGENT_QA_RECORD_SKIP_SIDECARS")
        .ok()
        .as_deref()
        == Some("1")
    {
        return;
    }
    match paths::scenario_dir(sid) {
        Ok(scenario_dir) => {
            let _ = capture_recording_sidecars(&scenario_dir, step_id, session);
        }
        Err(e) => eprintln!("[record] sidecar capture: resolve scenario dir failed: {e}"),
    }
}

fn parse_sid(env: &str) -> Option<String> {
    for line in env.lines() {
        if let Some(rest) = line.strip_prefix("SID=") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn parse_session(env: &str) -> Option<String> {
    for line in env.lines() {
        if let Some(rest) = line.strip_prefix("SESSION=") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn count_lines(path: &Path) -> Result<u64> {
    let body = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return Ok(0),
    };
    Ok(body.iter().filter(|&&b| b == b'\n').count() as u64)
}

fn append_jsonl(path: &Path, row: &StepRow) -> Result<()> {
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

fn capture_recording_sidecars(
    scenario_dir: &std::path::Path,
    step_id: &str,
    session: &str,
) -> Result<()> {
    use crate::sidecar::write_step_sidecar;
    // Recording-side sidecars live under <sid>/recording/{snapshots,screenshots}/
    // (mirroring the replay-side layout). We build the RunPaths shape by
    // pointing run_root at <sid>/recording — sidecar's helpers don't care
    // about the literal `replays` segment.
    let run = crate::sidecar::RunPaths {
        scenario_dir: scenario_dir.to_path_buf(),
        run_id: "recording".into(),
        run_root: scenario_dir.join("recording"),
    };
    fs::create_dir_all(&run.run_root).ok();

    match browser::snapshot_full(session) {
        Ok(text) => {
            let _ = write_step_sidecar(
                &run,
                crate::sidecar::SidecarKind::Snapshots,
                step_id,
                text.as_bytes(),
            );
        }
        Err(e) => eprintln!("[record-step] snapshot {step_id} failed: {e}"),
    }
    if let Ok(p) =
        crate::sidecar::step_sidecar_path(&run, crate::sidecar::SidecarKind::Screenshots, step_id)
    {
        // Ensure the screenshots directory exists before the binary tries to write.
        let _ = crate::sidecar::ensure_kind_dir(&run, crate::sidecar::SidecarKind::Screenshots);
        if let Err(e) = browser::screenshot(session, &p, true) {
            eprintln!("[record-step] screenshot {step_id} failed: {e}");
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn install_fake_browser(dir: &Path) {
        let body = "#!/bin/sh\nif [ \"$3\" = 'screenshot' ]; then shift 3; [ \"$1\" = '--full' ] && shift; : > \"$1\"; fi\nexit 0\n".to_string();
        let p = dir.join("agent-browser");
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        std::env::set_var(browser::BIN_ENV, &p);
        browser::_reset_bin_cache_for_tests();
    }

    fn clear_fake_browser() {
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    fn write_env(record_root: &Path, sid: &str) {
        fs::create_dir_all(record_root).unwrap();
        fs::write(
            record_root.join("scenario.env"),
            format!("SID={sid}\nINTENT='x'\nSESSION=default\nSTARTED=now\n"),
        )
        .unwrap();
    }

    #[test]
    fn record_appends_row_and_assigns_step_index() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        install_fake_browser(tmp.path());

        write_env(&rec, "mysid");
        // First call: index 0.
        let r1 = record(&Opts {
            kind: TriggerKind::Navigation,
            payload: json!({ "route": "https://x/" }),
        })
        .unwrap();
        assert_eq!(r1.step_index, 0);
        assert_eq!(r1.step_id, "s0");
        // Second call: index 1.
        let r2 = record(&Opts {
            kind: TriggerKind::Assert,
            payload: json!({ "kind": "url", "args": ["/x"], "intent": "loaded" }),
        })
        .unwrap();
        assert_eq!(r2.step_index, 1);
        assert_eq!(r2.step_id, "s1");

        // steps.jsonl should have exactly 2 lines.
        let body = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        assert_eq!(body.lines().count(), 2);
        // Each row parses to a StepRow.
        for line in body.lines() {
            let _row: StepRow = serde_json::from_str(line).unwrap();
        }

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake_browser();
    }

    #[test]
    fn record_errors_when_no_env_file() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("nope"));
        install_fake_browser(tmp.path());

        let err = record(&Opts {
            kind: TriggerKind::Navigation,
            payload: json!({ "route": "https://x/" }),
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("scenario.env"), "got: {err}");

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake_browser();
    }

    #[test]
    fn record_writes_recording_sidecars() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        install_fake_browser(tmp.path());
        write_env(&rec, "j2");

        record(&Opts {
            kind: TriggerKind::Navigation,
            payload: json!({ "route": "https://x/" }),
        })
        .unwrap();
        let jdir = tmp.path().join("j2").join("recording");
        assert!(jdir.join("snapshots").join("s0.txt").is_file());
        assert!(jdir.join("screenshots").join("s0.png").is_file());

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake_browser();
    }

    #[test]
    fn parse_args_rejects_unknown_kind() {
        parse_args(&["wat".into(), "{}".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_rejects_bad_json() {
        parse_args(&["navigation".into(), "not json".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_rejects_action_with_unknown_method() {
        let err = format!(
            "{:#}",
            parse_args(&[
                "action".into(),
                r#"{"method":"doTheThing","args":[]}"#.into(),
            ])
            .unwrap_err()
        );
        assert!(err.contains("allow-list"), "got: {err}");
    }

    #[test]
    fn parse_args_rejects_assert_without_intent() {
        let err = format!(
            "{:#}",
            parse_args(&["assert".into(), r#"{"kind":"url","args":["/x"]}"#.into(),]).unwrap_err()
        );
        assert!(err.contains("intent"), "got: {err}");
    }
}
