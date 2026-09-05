//! `flush` seals the active recorder state as `scenario.json`.

use std::fs;

use anyhow::{bail, Context, Result};

use crate::paths;
use crate::recorder_state::RecorderState;
use crate::scenario::{Env, Producer, Provenance, Scenario};
use crate::schema;
use crate::sidecar::atomic_write_file;

pub fn run(args: &[String]) -> Result<u8> {
    if let Some(arg) = args.first() {
        match arg.as_str() {
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
        "agent-qa flush - seal the active recording as scenario.json

Usage:
  agent-qa flush

Writes:
  <scenarios_root>/<sid>/scenario.json

On success it removes the active recorder state."
    );
}

#[derive(Debug, Clone)]
struct Summary {
    sid: String,
    steps: usize,
    scenario_file: std::path::PathBuf,
}

fn flush() -> Result<Summary> {
    let state = RecorderState::load_active()?;
    let scenario = Scenario {
        schema: "scenario/2".to_string(),
        id: state.sid.clone(),
        intent: state.intent.clone(),
        tags: None,
        inputs: None,
        env: (!state.env_open.is_empty()).then(|| Env {
            open: Some(state.env_open.clone()),
            close: None,
        }),
        steps: state.steps.clone(),
        templates: None,
        produced_by: Some(Provenance {
            producer: Producer::AgentRecorder,
            produced_at: Some(
                chrono::Utc::now()
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string(),
            ),
            recorded_at: Some(state.started_at),
            source_ref: state.source_ref,
        }),
    };
    let scenario_json = serde_json::to_value(&scenario)?;
    schema::validate_value(&scenario_json)
        .context("assembled scenario failed schema validation")?;

    let scenario_dir = paths::scenario_dir(&state.sid)?;
    fs::create_dir_all(&scenario_dir)
        .with_context(|| format!("mkdir -p {}", scenario_dir.display()))?;
    let scenario_file = scenario_dir.join("scenario.json");
    let mut bytes = serde_json::to_string_pretty(&scenario_json)?.into_bytes();
    bytes.push(b'\n');
    atomic_write_file(&scenario_file, &bytes)?;
    RecorderState::clear()?;

    Ok(Summary {
        sid: state.sid,
        steps: scenario.steps.len(),
        scenario_file,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::record_step::{record_draft, StepKind};
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn flush_seals_typed_state_and_clears_it() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        let mut state = RecorderState::new(
            "mysid".into(),
            "smoke flow".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            Some("change:123".into()),
            BrowserConnection::default(),
        );
        record_draft(&mut state, StepKind::Do, &json!({"intent":"open","verb":"goto","value":{"from":"literal","literal":"https://example.com/"}}), "default").unwrap();
        record_draft(
            &mut state,
            StepKind::Check,
            &json!({"intent":"loaded","claim":{"subject":{"url":true},"predicate":"exists"}}),
            "default",
        )
        .unwrap();

        let summary = flush().unwrap();
        let scenario: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary.scenario_file).unwrap()).unwrap();
        assert_eq!(summary.steps, 2);
        assert_eq!(scenario["steps"][0]["id"], "s0");
        assert_eq!(scenario["steps"][1]["kind"], "check");
        assert_eq!(scenario["producedBy"]["sourceRef"], "change:123");
        assert!(!paths::record_state_file().exists());
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
    }
}
