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

/// Assemble the scenario.json document `flush` would write for this
/// recorder state — also used by `buffer check` to validate pre-flush.
pub(crate) fn assemble_scenario(state: &RecorderState) -> Result<serde_json::Value> {
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
            recorded_at: Some(state.started_at.clone()),
            source_ref: state.source_ref.clone(),
        }),
    };
    let mut scenario_json = serde_json::to_value(&scenario)?;
    // `buffer load` stashes the source document on the state; fields the
    // buffer doesn't model (id, tags, inputs, templates, env.close and any
    // other env keys besides open) merge back so a load → flush round-trip
    // doesn't drop them.
    if let Some(orig) = state.original.as_ref().and_then(|v| v.as_object()) {
        let obj = scenario_json
            .as_object_mut()
            .expect("serialized scenario is an object");
        for key in ["id", "tags", "inputs", "templates"] {
            if let Some(v) = orig.get(key) {
                obj.insert(key.to_string(), v.clone());
            }
        }
        if let Some(orig_env) = orig.get("env").and_then(|v| v.as_object()) {
            let env_obj = match obj.get_mut("env").and_then(|v| v.as_object_mut()) {
                Some(e) => e,
                None => {
                    obj.insert("env".to_string(), serde_json::json!({}));
                    obj.get_mut("env").unwrap().as_object_mut().unwrap()
                }
            };
            for (k, v) in orig_env {
                if k != "open" {
                    env_obj.insert(k.clone(), v.clone());
                }
            }
            if env_obj.is_empty() {
                obj.remove("env");
            }
        }
    }
    schema::validate_value(&scenario_json)
        .context("assembled scenario failed schema validation")?;
    Ok(scenario_json)
}

fn flush() -> Result<Summary> {
    let state = RecorderState::load_active()?;
    let scenario_json = assemble_scenario(&state)?;

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
        steps: state.steps.len(),
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

    #[test]
    fn buffer_load_then_flush_preserves_unmodeled_fields() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        // A saved scenario carrying fields the buffer doesn't model.
        let dir = tmp.path().join("flow");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("scenario.json"),
            serde_json::to_string_pretty(&json!({
                "schema": "scenario/2", "id": "flow-doc-id", "intent": "the flow",
                "tags": ["smoke"],
                "inputs": { "email": { "type": "string", "default": "a@b" } },
                "env": {
                    "open": [{ "kind": "nav", "url": "https://example.com/", "intent": "land" }],
                    "close": [{ "kind": "fresh" }]
                },
                "steps": [{ "id": "s0", "intent": "go", "kind": "do", "verb": "reload" }]
            }))
            .unwrap(),
        )
        .unwrap();

        crate::buffer::run(&["load".into(), "flow".into()]).unwrap();
        // Edit a step mid-buffer, then seal.
        let mut state = RecorderState::load_active().unwrap();
        assert_eq!(state.steps.len(), 1);
        state.steps[0] = serde_json::from_value(json!({
            "id": "s0", "intent": "edited", "kind": "do", "verb": "reload"
        }))
        .unwrap();
        state.save().unwrap();
        flush().unwrap();

        let scenario: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("scenario.json")).unwrap()).unwrap();
        assert_eq!(scenario["id"], "flow-doc-id");
        assert_eq!(scenario["tags"], json!(["smoke"]));
        assert_eq!(scenario["inputs"]["email"]["default"], "a@b");
        assert_eq!(scenario["env"]["close"], json!([{ "kind": "fresh" }]));
        assert_eq!(scenario["env"]["open"][0]["kind"], "nav");
        assert_eq!(scenario["steps"][0]["intent"], "edited");
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
    }
}
