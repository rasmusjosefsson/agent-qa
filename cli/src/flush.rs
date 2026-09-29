//! `flush` seals the active recorder state as `scenario.json`.

use std::fs;

use anyhow::{bail, Context, Result};

use crate::paths;
use crate::recorder_state::RecorderState;
use crate::scenario::{Env, Producer, Provenance, Scenario};
use crate::schema;
use crate::sidecar::atomic_write_file;

pub fn run(args: &[String]) -> Result<u8> {
    let mut auto_shots = false;
    let mut auto_network = true;
    let mut auto_errors = true;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            "--auto-shots" => auto_shots = true,
            "--auto-network" => auto_network = true,
            "--auto-errors" => auto_errors = true,
            "--no-auto-network" => auto_network = false,
            "--no-auto-errors" => auto_errors = false,
            other => bail!("flush: unknown argument {other:?}"),
        }
    }
    let summary = flush(auto_shots, auto_network, auto_errors)?;
    println!("flushed sid={} steps={}", summary.sid, summary.steps);
    println!("wrote   {}", summary.scenario_file.display());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa flush - seal the active recording as scenario.json

Usage:
  agent-qa flush [--auto-shots]

Options:
  --auto-shots   Append a {{\"shot\"}} visual claim after every do-step, so a
                recorded flow gains golden-image coverage for free. Mint the
                baselines with `agent-qa shot-accept <sid>` after the first
                replay.
  --auto-network Append a {{\"network\": {{urlMatches,method}},ofKind:fired}}
                check per distinct XHR/fetch/non-GET request the session
                made — replays then prove the same API calls still happen
                (max 12). ON by default; --no-auto-network disables.
  --auto-errors  Append a {{\"pageError\": true}} notExists check — a page
                that starts throwing uncaught exceptions fails the replay.
                ON by default; --no-auto-errors disables.
  --auto-shots stays opt-in: shot claims need minted baselines.

Writes:
  <scenarios_root>/<sid>/scenario.json
  <scenarios_root>/<sid>/replays/recorded/network.har  — the session's
    traffic while recording; `replay <sid> --mock-from recorded` replays
    the scenario offline from those recorded responses.

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

/// Insert a {\"shot\": \"<doStepId>\"} check after every do-step; renumber
/// afterwards so ids stay dense and the new refs get rewritten correctly.
fn insert_auto_shot_claims(steps: &mut Vec<crate::scenario::Step>) {
    let mut out: Vec<crate::scenario::Step> = Vec::with_capacity(steps.len() * 2);
    for step in steps.drain(..) {
        if let crate::scenario::Step::Do { id, intent, .. } = &step {
            out.push(step.clone());
            out.push(crate::scenario::Step::Check {
                id: String::new(),
                intent: format!("visual: {}", intent),
                claim: crate::scenario::Claim {
                    subject: crate::scenario::ClaimSubject::Shot {
                        shot: id.clone(),
                        clip: None,
                        mask: Vec::new(),
                    },
                    predicate: crate::scenario::Predicate::Matches,
                    value: None,
                    tolerance: Some(std::collections::BTreeMap::from([(
                        "pixels".to_string(),
                        serde_json::json!(0.05),
                    )])),
                },
                context: None,
            });
        } else {
            out.push(step);
        }
    }
    *steps = out;
    crate::buffer::normalize_ids(steps);
}

/// Append a `networkFired` check per distinct (method, path) the session
/// captured — XHR/Fetch/EventSource/WebSocket plus any non-GET (POSTs are
/// API calls whatever the resource type reports). Document/script/css
/// traffic is covered implicitly by the goto steps already in the buffer.
/// Capped at 12 so a chatty page doesn't flood the scenario.
fn insert_auto_network_claims(
    steps: &mut Vec<crate::scenario::Step>,
    requests: &[crate::browser::CapturedRequest],
) {
    let mut seen = std::collections::BTreeSet::new();
    let mut added = 0usize;
    for r in requests {
        let rt = r.resource_type.as_deref().unwrap_or("");
        let is_api =
            matches!(rt, "XHR" | "Fetch" | "EventSource" | "WebSocket") || r.method != "GET";
        if !is_api || crate::telemetry::is_telemetry_url(&r.url) {
            continue;
        }
        let path = r.url.split(['?', '#']).next().unwrap_or(&r.url);
        if !seen.insert((r.method.clone(), path.to_string())) {
            continue;
        }
        added += 1;
        if added > 12 {
            break;
        }
        let method = match r.method.as_str() {
            "GET" => Some(crate::scenario::HttpMethod::Get),
            "POST" => Some(crate::scenario::HttpMethod::Post),
            "PUT" => Some(crate::scenario::HttpMethod::Put),
            "PATCH" => Some(crate::scenario::HttpMethod::Patch),
            "DELETE" => Some(crate::scenario::HttpMethod::Delete),
            "HEAD" => Some(crate::scenario::HttpMethod::Head),
            _ => None,
        };
        steps.push(crate::scenario::Step::Check {
            id: String::new(),
            intent: format!("{} {} fired", r.method, path),
            claim: crate::scenario::Claim {
                subject: crate::scenario::ClaimSubject::Network {
                    network: crate::scenario::NetworkMatcher {
                        url_matches: Some(path.to_string()),
                        method,
                        ..Default::default()
                    },
                    of_kind: Some(crate::scenario::NetworkClaimKind::Fired),
                    path: None,
                },
                predicate: crate::scenario::Predicate::Exists,
                value: None,
                tolerance: None,
            },
            context: None,
        });
    }
    crate::buffer::normalize_ids(steps);
}

/// Append a `{"pageError": true}` notExists check — an uncaught exception
/// during replay then fails the scenario the same way a broken element does.
fn append_auto_error_claims(steps: &mut Vec<crate::scenario::Step>) {
    steps.push(crate::scenario::Step::Check {
        id: String::new(),
        intent: "page raised no uncaught exceptions".to_string(),
        claim: crate::scenario::Claim {
            subject: crate::scenario::ClaimSubject::PageError {
                page_error: crate::scenario::PageErrorSubject::Flag(true),
            },
            predicate: crate::scenario::Predicate::NotExists,
            value: None,
            tolerance: None,
        },
        context: None,
    });
    crate::buffer::normalize_ids(steps);
}

fn flush(auto_shots: bool, auto_network: bool, auto_errors: bool) -> Result<Summary> {
    let mut state = RecorderState::load_active()?;
    if auto_shots {
        insert_auto_shot_claims(&mut state.steps);
    }
    if auto_network {
        match crate::browser::network_requests(&state.session) {
            Ok(requests) => insert_auto_network_claims(&mut state.steps, &requests),
            Err(e) => eprintln!("[v2-record] auto-network skipped: {e}"),
        }
    }
    if auto_errors {
        append_auto_error_claims(&mut state.steps);
    }
    let scenario_json = assemble_scenario(&state)?;

    let scenario_dir = paths::scenario_dir(&state.sid)?;
    fs::create_dir_all(&scenario_dir)
        .with_context(|| format!("mkdir -p {}", scenario_dir.display()))?;
    let scenario_file = scenario_dir.join("scenario.json");
    let mut bytes = serde_json::to_string_pretty(&scenario_json)?.into_bytes();
    bytes.push(b'\n');
    atomic_write_file(&scenario_file, &bytes)?;

    // Stop the HAR recording `start` opened and keep it next to the
    // scenario: `replay <sid> --mock-from recorded` then replays
    // hermetically from the recorded responses.
    let har_dest = scenario_dir
        .join("replays")
        .join("recorded")
        .join("network.har");
    if let Some(parent) = har_dest.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("[v2-record] mkdir {}: {e}", parent.display());
        }
    }
    match crate::browser::network_har_stop(&state.session, &har_dest) {
        Ok(()) => eprintln!(
            "[v2-record] network.har → replays/recorded/ — `replay {} --mock-from recorded` replays offline",
            state.sid
        ),
        Err(e) => eprintln!("[v2-record] har stop skipped: {e}"),
    }

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

        let summary = flush(false, false, false).unwrap();
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
        flush(false, false, false).unwrap();

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

    #[test]
    fn flush_auto_shots_appends_a_visual_claim_after_each_do_step() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        let mut state = RecorderState::new(
            "autoshot".into(),
            "flow".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        record_draft(&mut state, StepKind::Do, &json!({"intent":"go","verb":"goto","value":{"from":"literal","literal":"https://example.com/"}}), "default").unwrap();
        record_draft(
            &mut state,
            StepKind::Check,
            &json!({"intent":"loaded","claim":{"subject":{"url":true},"predicate":"exists"}}),
            "default",
        )
        .unwrap();
        record_draft(
            &mut state,
            StepKind::Do,
            &json!({"intent":"hit save","verb":"reload"}),
            "default",
        )
        .unwrap();

        let summary = flush(true, false, false).unwrap();
        let scenario: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary.scenario_file).unwrap()).unwrap();
        let steps = scenario["steps"].as_array().unwrap();
        // do, shot, check, do, shot — dense ids, shot refs point at the
        // renumbered do-step they follow.
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[1]["claim"]["subject"]["shot"], "s0");
        assert_eq!(steps[1]["id"], "s1");
        assert_eq!(steps[2]["claim"]["subject"]["url"], true);
        assert_eq!(steps[3]["id"], "s3");
        assert_eq!(steps[4]["claim"]["subject"]["shot"], "s3");
        assert_eq!(steps[4]["claim"]["tolerance"]["pixels"], json!(0.05));
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
    }

    #[test]
    fn flush_auto_network_claims_dedup_api_calls() {
        use crate::browser::CapturedRequest;
        let req = |method: &str, url: &str, rt: &str| CapturedRequest {
            request_id: String::new(),
            url: url.to_string(),
            method: method.to_string(),
            status: Some(200),
            resource_type: Some(rt.to_string()),
            mime_type: None,
            post_data: None,
        };
        let mut steps = vec![crate::scenario::Step::Do {
            id: "s0".into(),
            intent: "open".into(),
            verb: crate::scenario::Verb::Goto,
            on: None,
            value: None,
            save_as: None,
            params: None,
            context: None,
        }];
        let requests = vec![
            req("GET", "https://x/app.css", "Stylesheet"),
            req("POST", "https://x/api/login?a=1", "XHR"),
            req("POST", "https://x/api/login?a=2", "XHR"),
            req("GET", "https://x/api/me", "Fetch"),
        ];
        insert_auto_network_claims(&mut steps, &requests);
        // css is dropped, the dup POST collapses to one claim → 2 appended
        assert_eq!(steps.len(), 3);
        let subj = &steps[1];
        let json = serde_json::to_value(subj).unwrap();
        assert_eq!(json["claim"]["subject"]["network"]["method"], "POST");
        assert_eq!(
            json["claim"]["subject"]["network"]["urlMatches"],
            "https://x/api/login"
        );
        assert_eq!(json["claim"]["subject"]["ofKind"], "fired");
        assert_eq!(json["claim"]["predicate"], "exists");
        let json2 = serde_json::to_value(&steps[2]).unwrap();
        assert_eq!(
            json2["claim"]["subject"]["network"]["urlMatches"],
            "https://x/api/me"
        );
    }

    #[test]
    fn flush_auto_network_claims_skip_telemetry() {
        use crate::browser::CapturedRequest;
        let req = |method: &str, url: &str, rt: &str| CapturedRequest {
            request_id: String::new(),
            url: url.to_string(),
            method: method.to_string(),
            status: Some(200),
            resource_type: Some(rt.to_string()),
            mime_type: None,
            post_data: None,
        };
        let mut steps = vec![];
        let requests = vec![
            // third-party analytics beacon
            req("POST", "https://www.google-analytics.com/g/collect", "XHR"),
            // same-origin Cloudflare RUM beacon — host filter can't see it
            req("POST", "https://x/cdn-cgi/rum", "XHR"),
            req("GET", "https://x/api/me", "Fetch"),
        ];
        insert_auto_network_claims(&mut steps, &requests);
        assert_eq!(steps.len(), 1);
        let json = serde_json::to_value(&steps[0]).unwrap();
        assert_eq!(
            json["claim"]["subject"]["network"]["urlMatches"],
            "https://x/api/me"
        );
    }

    #[test]
    fn flush_auto_errors_appends_page_error_gate() {
        let mut steps = vec![];
        append_auto_error_claims(&mut steps);
        assert_eq!(steps.len(), 1);
        let json = serde_json::to_value(&steps[0]).unwrap();
        assert_eq!(json["claim"]["subject"]["pageError"], true);
        assert_eq!(json["claim"]["predicate"], "notExists");
        assert_eq!(json["id"], "s0");
    }
}
