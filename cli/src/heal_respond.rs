//! `heal-respond` verb — caller-driven heal response.
//!
//! When a replay step fails with a value-rejection (the live form told us
//! "this email is taken" / "must be unique" / etc.), the agent invoking
//! agent-qa decides on a corrected value or a refusal, and records that
//! decision via this verb. A subsequent `replay --heal-from-run <runId>`
//! consumes the heal-responses to override step values at dispatch time.
//!
//! Writes:
//!   <sid>/replays/<runId>/heal-responses/<stepId>.json
//!     { stepId, mode: 'value-correction' | 'reject',
//!       value?, rationale?, recordedAt }
//!   appends to <sid>/recording/heal.jsonl  (audit trail)
//!
//! Does NOT touch the live tab. Re-running replay is the caller's job.
//!
//! CLI shape:
//!
//!   agent-qa heal-respond <sid> --step <stepId>
//!                                (--value <correctedValue> [--rationale <text>]
//!                                 | --reject)
//!                                [--run <runId>]   defaults to latest run
//!
//! `--run` defaults to the contents of `<sid>/replays/latest.txt` so the
//! common case is a one-arg invocation.

use std::fs;
use std::io::Write;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::sidecar::atomic_write_file;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let summary = respond(&opts)?;
    println!(
        "heal-respond {} ({}): {}",
        summary.step_id, summary.mode, summary.run_id
    );
    println!("file: {}", summary.response_file.display());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa heal-respond \u{2014} record a caller-driven heal response\n\nUsage:\n  agent-qa heal-respond <sid> --step <stepId>\n                                (--value <correctedValue> [--rationale <text>]\n                                 | --reject)\n                                [--run <runId>]\n\n--run defaults to <sid>/replays/latest.txt. The response file feeds into\n`replay --heal-from-run <runId>` (lands later) which overrides the step's\nvalue at dispatch time.\n\nDoes NOT touch the live tab \u{2014} re-running replay is the caller's job."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    sid: String,
    step_id: String,
    run_id: Option<String>,
    decision: Decision,
}

#[derive(Debug, Clone)]
enum Decision {
    Value {
        value: String,
        rationale: Option<String>,
    },
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    ValueCorrection,
    Reject,
}

impl Mode {
    fn label(&self) -> &'static str {
        match self {
            Mode::ValueCorrection => "value-correction",
            Mode::Reject => "reject",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    step_id: String,
    mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rationale: Option<String>,
    recorded_at: String,
}

#[derive(Debug)]
struct Summary {
    step_id: String,
    run_id: String,
    mode: &'static str,
    response_file: std::path::PathBuf,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut sid: Option<String> = None;
    let mut step: Option<String> = None;
    let mut value: Option<String> = None;
    let mut rationale: Option<String> = None;
    let mut reject = false;
    let mut run: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--step" => step = it.next().cloned(),
            s if s.starts_with("--step=") => step = Some(s["--step=".len()..].to_string()),
            "--value" => value = it.next().cloned(),
            s if s.starts_with("--value=") => value = Some(s["--value=".len()..].to_string()),
            "--rationale" => rationale = it.next().cloned(),
            s if s.starts_with("--rationale=") => {
                rationale = Some(s["--rationale=".len()..].to_string())
            }
            "--reject" => reject = true,
            "--run" => run = it.next().cloned(),
            s if s.starts_with("--run=") => run = Some(s["--run=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!("unexpected positional {other:?}; usage: heal-respond <sid> --step <stepId> (--value <…> | --reject)");
                }
                sid = Some(other.to_string());
            }
        }
    }
    let sid = sid.ok_or_else(|| {
        anyhow!("usage: heal-respond <sid> --step <stepId> (--value <…> | --reject)")
    })?;
    let step_id = step.ok_or_else(|| anyhow!("--step is required"))?;
    let decision = match (value, reject) {
        (Some(v), false) => Decision::Value {
            value: v,
            rationale,
        },
        (None, true) => Decision::Reject,
        (Some(_), true) => bail!("--value and --reject are mutually exclusive"),
        (None, false) => bail!("one of --value or --reject is required"),
    };
    Ok(Opts {
        sid,
        step_id,
        run_id: run,
        decision,
    })
}

fn respond(opts: &Opts) -> Result<Summary> {
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

    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let (mode_value, response) = match &opts.decision {
        Decision::Value { value, rationale } => (
            Mode::ValueCorrection,
            Response {
                step_id: opts.step_id.clone(),
                mode: Mode::ValueCorrection,
                value: Some(value.clone()),
                rationale: rationale.clone(),
                recorded_at: now.clone(),
            },
        ),
        Decision::Reject => (
            Mode::Reject,
            Response {
                step_id: opts.step_id.clone(),
                mode: Mode::Reject,
                value: None,
                rationale: None,
                recorded_at: now.clone(),
            },
        ),
    };

    // Write the per-step heal-response file.
    let dir = scenario_dir
        .join("replays")
        .join(&run_id)
        .join("heal-responses");
    fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    let response_file = dir.join(format!("{}.json", opts.step_id));
    let mut bytes = serde_json::to_string_pretty(&response)?.into_bytes();
    bytes.push(b'\n');
    atomic_write_file(&response_file, &bytes)?;

    // Append an audit row to <sid>/recording/heal.jsonl.
    let recording = scenario_dir.join("recording");
    fs::create_dir_all(&recording).ok();
    let audit_path = recording.join("heal.jsonl");
    let row = serde_json::json!({
        "schema": "heal-row/v1",
        "ts": now,
        "runId": run_id,
        "stepId": opts.step_id,
        "mode": mode_value.label(),
        "rationale": match &opts.decision { Decision::Value { rationale, .. } => rationale.clone(), Decision::Reject => None },
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

    Ok(Summary {
        step_id: opts.step_id.clone(),
        run_id,
        mode: mode_value.label(),
        response_file,
    })
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
    use serde_json::Value as Json;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp);
    }
    fn teardown() {
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
    }

    fn opts(sid: &str, step: &str, decision: Decision, run: Option<&str>) -> Opts {
        Opts {
            sid: sid.into(),
            step_id: step.into(),
            run_id: run.map(str::to_string),
            decision,
        }
    }

    #[test]
    fn respond_writes_value_correction() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());

        let s = respond(&opts(
            "j1",
            "s5",
            Decision::Value {
                value: "qa-new@e.com".into(),
                rationale: Some("uniqueness".into()),
            },
            Some("r1"),
        ))
        .unwrap();
        assert_eq!(s.step_id, "s5");
        assert_eq!(s.mode, "value-correction");

        let body = fs::read_to_string(&s.response_file).unwrap();
        let r: Json = serde_json::from_str(&body).unwrap();
        assert_eq!(r["stepId"], "s5");
        assert_eq!(r["mode"], "value-correction");
        assert_eq!(r["value"], "qa-new@e.com");
        assert_eq!(r["rationale"], "uniqueness");

        // Audit row appended.
        let audit = fs::read_to_string(tmp.path().join("j1/recording/heal.jsonl")).unwrap();
        assert_eq!(audit.lines().count(), 1);
        assert!(audit.contains("\"mode\":\"value-correction\""));
        assert!(audit.contains("\"runId\":\"r1\""));

        teardown();
    }

    #[test]
    fn respond_writes_reject_without_value() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());

        let s = respond(&opts("j1", "s2", Decision::Reject, Some("r1"))).unwrap();
        assert_eq!(s.mode, "reject");
        let body = fs::read_to_string(&s.response_file).unwrap();
        let r: Json = serde_json::from_str(&body).unwrap();
        assert_eq!(r["mode"], "reject");
        assert!(r.get("value").map(|v| v.is_null()).unwrap_or(true));
        teardown();
    }

    #[test]
    fn respond_uses_latest_pointer_when_run_missing() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());

        let latest = tmp.path().join("j1/replays/latest.txt");
        fs::create_dir_all(latest.parent().unwrap()).unwrap();
        fs::write(&latest, "rXYZ\n").unwrap();

        let s = respond(&opts(
            "j1",
            "s0",
            Decision::Value {
                value: "v".into(),
                rationale: None,
            },
            None,
        ))
        .unwrap();
        assert_eq!(s.run_id, "rXYZ");

        teardown();
    }

    #[test]
    fn respond_errors_when_no_run_and_no_pointer() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let err = respond(&opts("j1", "s0", Decision::Reject, None))
            .unwrap_err()
            .to_string();
        assert!(err.contains("latest.txt"), "got: {err}");
        teardown();
    }

    #[test]
    fn parse_args_value_and_reject_mutually_exclusive() {
        let err = parse_args(&[
            "j1".into(),
            "--step".into(),
            "s0".into(),
            "--value".into(),
            "v".into(),
            "--reject".into(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("mutually exclusive"));
    }

    #[test]
    fn parse_args_requires_value_or_reject() {
        let err = parse_args(&["j1".into(), "--step".into(), "s0".into()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("--value or --reject"));
    }

    #[test]
    fn parse_args_rejects_unsafe_run_id() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        respond(&opts("j1", "s0", Decision::Reject, Some("../escape"))).unwrap_err();
        teardown();
    }
}
