//! `flush` seals the active recorder state as `scenario.json`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    let mut auto_secrets = true;
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            "--auto-shots" => auto_shots = true,
            "--auto-network" => auto_network = true,
            "--auto-errors" => auto_errors = true,
            "--auto-secrets" => auto_secrets = true,
            "--no-auto-network" => auto_network = false,
            "--no-auto-errors" => auto_errors = false,
            "--no-auto-secrets" => auto_secrets = false,
            other => bail!("flush: unknown argument {other:?}"),
        }
    }
    let summary = flush(auto_shots, auto_network, auto_errors, auto_secrets)?;
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
  --auto-secrets Lift literal values typed into password-shaped fields out
                of scenario.json: each becomes a {{from: input}} ref + a
                sensitive inputs entry, and the real value lands in
                inputs.local.json beside the scenario (gitignored). ON by
                default; --no-auto-secrets disables.
  --auto-shots stays opt-in: shot claims need minted baselines.

Writes:
  <scenarios_root>/<sid>/scenario.json
  <scenarios_root>/<sid>/replays/recorded/network.har  — the session's
    traffic while recording; `replay <sid> --mock-from recorded` replays
    the scenario offline from those recorded responses.
  <scenarios_root>/<sid>/inputs.local.json  — recorded secrets lifted out
    of scenario.json by --auto-secrets; gitignored, consumed by replay as
    a last-resort input source.

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
        only_when: None,
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

/// True when any string inside a step's `on` locator mentions "password"
/// — css `input[type=password]`, a role name like "Password", a password-y
/// testid. Same detector the `hardcoded-secret` lint runs.
fn locator_mentions_password(on: &serde_json::Value) -> bool {
    let mut stack = vec![on];
    while let Some(v) = stack.pop() {
        match v {
            serde_json::Value::String(s) if s.to_lowercase().contains("password") => return true,
            serde_json::Value::Object(m) => stack.extend(m.values()),
            serde_json::Value::Array(a) => stack.extend(a.iter()),
            _ => {}
        }
    }
    false
}

/// Rewrite `type` steps whose literal value lands in a password-shaped
/// field: `{"from":"literal","literal":"hunter2"}` becomes
/// `{"from":"input","input":"PASSWORD"}` and `inputs.PASSWORD` is declared
/// `{type: string, sensitive: true}`. Returns the (name, literal) pairs to
/// stash in `inputs.local.json` — the committed scenario stays
/// secret-free while replay still resolves the real value locally.
fn sweep_secret_literals(doc: &mut serde_json::Value) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut taken: std::collections::HashSet<String> = doc
        .get("inputs")
        .and_then(|i| i.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let Some(steps) = doc.get_mut("steps").and_then(|s| s.as_array_mut()) else {
        return found;
    };
    for step in steps.iter_mut() {
        if step.get("verb").and_then(|v| v.as_str()) != Some("type") {
            continue;
        }
        let literal = step
            .get("value")
            .and_then(|v| v.get("literal"))
            .and_then(|l| l.as_str())
            .map(str::to_string);
        let Some(literal) = literal.filter(|l| !l.is_empty()) else {
            continue;
        };
        if !step.get("on").is_some_and(locator_mentions_password) {
            continue;
        }
        let mut n = 1u32;
        let name = loop {
            let cand = if n == 1 {
                "PASSWORD".to_string()
            } else {
                format!("PASSWORD_{n}")
            };
            if !taken.contains(&cand) {
                break cand;
            }
            n += 1;
        };
        taken.insert(name.clone());
        step["value"] = serde_json::json!({ "from": "input", "input": name });
        found.push((name, literal));
    }
    if !found.is_empty() {
        let obj = doc.as_object_mut().expect("scenario doc is an object");
        let inputs = obj.entry("inputs").or_insert_with(|| serde_json::json!({}));
        let inputs = inputs.as_object_mut().expect("inputs is an object");
        for (name, _) in &found {
            inputs
                .entry(name.clone())
                .or_insert_with(|| serde_json::json!({ "type": "string", "sensitive": true }));
        }
    }
    found
}

/// Merge (name, literal) pairs into `<scenario_dir>/inputs.local.json` —
/// a gitignored local store `resolve_inputs` consults last.
fn merge_local_inputs(dir: &std::path::Path, secrets: &[(String, String)]) -> Result<()> {
    let p = dir.join("inputs.local.json");
    let mut map: serde_json::Map<String, serde_json::Value> = match fs::read(&p) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("parse {}", p.display()))?
        }
        Err(_) => serde_json::Map::new(),
    };
    for (name, literal) in secrets {
        map.insert(name.clone(), serde_json::Value::String(literal.clone()));
    }
    let mut bytes = serde_json::to_string_pretty(&serde_json::Value::Object(map))?.into_bytes();
    bytes.push(b'\n');
    atomic_write_file(&p, &bytes)
}

/// Verbs whose step produces no *newly* viewable page state — a shot
/// minted after a navigation catches a transition frame and goes flaky,
/// and a shot after a non-visual step just duplicates the previous one.
/// Interaction + wait verbs keep theirs: those are the settled states
/// worth pinning.
fn auto_shot_worthy(verb: &crate::scenario::Verb) -> bool {
    use crate::scenario::Verb as V;
    !matches!(
        verb,
        V::Goto
            | V::Reload
            | V::Back
            | V::Forward
            | V::Tab
            | V::Frame
            | V::State
            | V::Mock
            | V::Unmock
            | V::Dialog
            | V::Read
            | V::CallGql
            | V::Loop
            | V::Group
            | V::UseTemplate
    )
}

/// Copy absolute-path file literals on `upload`/`fileChooser` steps into
/// `<scenario_dir>/files/` and rewrite them to scenario-relative paths —
/// a recording made on one machine replays on another only when its
/// uploads travel with the scenario. Returns the relative paths written.
fn package_file_literals(doc: &mut serde_json::Value, scenario_dir: &Path) -> Vec<String> {
    let mut packaged = Vec::new();
    let mut by_src: HashMap<String, String> = HashMap::new();
    let Some(steps) = doc.get_mut("steps").and_then(|s| s.as_array_mut()) else {
        return packaged;
    };
    for step in steps.iter_mut() {
        if step.get("kind").and_then(|k| k.as_str()) != Some("do") {
            continue;
        }
        let node = match step.get("verb").and_then(|v| v.as_str()) {
            Some("upload") => step.get_mut("value").and_then(|v| v.get_mut("literal")),
            Some("fileChooser") => step.get_mut("params").and_then(|p| p.get_mut("files")),
            _ => None,
        };
        if let Some(node) = node {
            package_node_paths(node, scenario_dir, &mut by_src, &mut packaged);
        }
    }
    packaged
}

/// Rewrite each absolute existing path inside a Json string|string[]
/// node to its packaged `files/<name>` relative path.
fn package_node_paths(
    node: &mut serde_json::Value,
    scenario_dir: &Path,
    by_src: &mut HashMap<String, String>,
    packaged: &mut Vec<String>,
) {
    match node {
        serde_json::Value::String(s) => {
            if let Some(rel) = package_one_path(s, scenario_dir, by_src) {
                *node = serde_json::Value::String(rel.clone());
                packaged.push(rel);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                if let Some(rel) = item
                    .as_str()
                    .and_then(|s| package_one_path(s, scenario_dir, by_src))
                {
                    *item = serde_json::Value::String(rel.clone());
                    packaged.push(rel);
                }
            }
        }
        _ => {}
    }
}

/// Absolute path that exists → copy under `<dir>/files/` (deduped per
/// flush) and return the scenario-relative path; anything else → None,
/// left untouched (relative paths already resolve like uploads do).
fn package_one_path(
    raw: &str,
    scenario_dir: &Path,
    by_src: &mut HashMap<String, String>,
) -> Option<String> {
    if let Some(cached) = by_src.get(raw) {
        return Some(cached.clone());
    }
    let src = PathBuf::from(raw);
    if !src.is_absolute() || !src.is_file() {
        return None;
    }
    let base = src.file_name()?.to_str()?.to_string();
    let files_dir = scenario_dir.join("files");
    let stem = Path::new(&base)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = Path::new(&base)
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let mut name = base.clone();
    let mut n = 2u32;
    while files_dir.join(&name).exists() {
        name = format!("{stem}-{n}{ext}");
        n += 1;
    }
    let dest = files_dir.join(&name);
    fs::create_dir_all(&files_dir).ok()?;
    fs::copy(&src, &dest).ok()?;
    let rel = format!("files/{name}");
    by_src.insert(raw.to_string(), rel.clone());
    Some(rel)
}

/// Insert a {"shot": "<doStepId>"} check after every shot-worthy
/// do-step; renumber afterwards so ids stay dense and the new refs get
/// rewritten correctly.
fn insert_auto_shot_claims(steps: &mut Vec<crate::scenario::Step>) {
    let mut out: Vec<crate::scenario::Step> = Vec::with_capacity(steps.len() * 2);
    for step in steps.drain(..) {
        if let crate::scenario::Step::Do {
            id, intent, verb, ..
        } = &step
        {
            out.push(step.clone());
            if !auto_shot_worthy(verb) {
                continue;
            }
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
                enabled: None,
                context: None,
            });
        } else {
            out.push(step);
        }
    }
    *steps = out;
    crate::buffer::normalize_ids(steps);
}

/// Strip `;k=v` matrix params (e.g. `;jsessionid=…`) from each path segment —
/// they are almost always session-scoped, so a claim matcher carrying one can
/// never match a fresh session at replay.
fn strip_matrix_params(url_path: &str) -> String {
    url_path
        .split('/')
        .map(|seg| seg.split(';').next().unwrap_or(seg))
        .collect::<Vec<_>>()
        .join("/")
}

/// Rewrite entity ids in URL path segments into regexes — an auto claim
/// recorded as `…/customers/63827` can never match the fresh id a replay
/// mints, so all-numeric segments become `\d+` and long hex/uuid segments
/// become `[0-9a-fA-F-]{8,}`.
fn normalize_volatile_path_segments(url_path: &str) -> String {
    url_path
        .split('/')
        .map(|seg| {
            if !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit()) {
                "\\d+".to_string()
            } else if seg.len() >= 8 && seg.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
                "[0-9a-fA-F-]{8,}".to_string()
            } else {
                seg.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
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
        // `;` cuts matrix params — Java containers append a volatile
        // `;jsessionid=<id>` path segment whose record-time value can never
        // match a fresh replay session.
        let path = normalize_volatile_path_segments(&strip_matrix_params(
            r.url.split(['?', '#']).next().unwrap_or(&r.url),
        ));
        if !seen.insert((r.method.clone(), path.clone())) {
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
                        url_matches: Some(path),
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
            enabled: None,
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
        enabled: None,
        context: None,
    });
    crate::buffer::normalize_ids(steps);
}

fn flush(
    auto_shots: bool,
    auto_network: bool,
    auto_errors: bool,
    auto_secrets: bool,
) -> Result<Summary> {
    let mut state = RecorderState::load_active()?;
    if state.paused {
        // A paused recorder drops page actions AND record-step appends —
        // flushing now seals a scenario that's silently missing the tail
        // of the flow. Legit for an intentional prefix, so warn only.
        eprintln!(
            "[v2-flush] warning: recording is paused — page actions and record-step appends after `record pause` are NOT in the buffer (`record resume` recaptures)"
        );
    }
    if state.steps.is_empty() {
        bail!(
            "nothing recorded for {:?} — the buffer has 0 steps, so flush would write a scenario that replays nothing. Capture steps first (`record status` shows the buffer); `start --force` abandons the recording if it was a false start.",
            state.sid
        );
    }
    if auto_shots {
        insert_auto_shot_claims(&mut state.steps);
    }
    // A replay/crawl in flight on this session would feed its own traffic
    // into the auto-network claims and the HAR — the scenario asserts on
    // requests that were never part of the recording.
    if crate::session_lock::held_by_live_process(&state.session).is_some() {
        eprintln!(
            "[v2-flush] warning: a run is in flight on session {:?} — auto-network claims and the HAR may capture that run's traffic",
            state.session
        );
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
    let mut scenario_json = assemble_scenario(&state)?;
    let mut secrets = Vec::new();
    if auto_secrets {
        secrets = sweep_secret_literals(&mut scenario_json);
        if !secrets.is_empty() {
            schema::validate_value(&scenario_json)
                .context("scenario failed schema validation after the secret sweep")?;
        }
    }

    let scenario_dir = paths::scenario_dir(&state.sid)?;
    fs::create_dir_all(&scenario_dir)
        .with_context(|| format!("mkdir -p {}", scenario_dir.display()))?;
    let packaged = package_file_literals(&mut scenario_json, &scenario_dir);
    if !packaged.is_empty() {
        eprintln!(
            "[v2-record] {} upload file(s) → files/ — recording replays on any machine",
            packaged.len()
        );
    }
    let scenario_file = scenario_dir.join("scenario.json");
    // Refuse to clobber a scenario this recording didn't come from:
    // `record continue`/`buffer load` stamp source_ref "scenario:<sid>",
    // while a fresh recording flushing onto an occupied sid would silently
    // replace an unrelated scenario.
    let continuing = state
        .source_ref
        .as_deref()
        .is_some_and(|r| r == format!("scenario:{}", state.sid));
    if scenario_file.exists() && !continuing {
        bail!(
            "{:?} already has a scenario — refusing to overwrite it from a fresh recording. `record continue {}` extends it, or record under a different sid.",
            state.sid,
            state.sid
        );
    }
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

    if !secrets.is_empty() {
        merge_local_inputs(&scenario_dir, &secrets)?;
        eprintln!(
            "[v2-record] {} secret value(s) → sensitive inputs; real values in inputs.local.json (gitignored)",
            secrets.len()
        );
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

        let summary = flush(false, false, false, false).unwrap();
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
    fn flush_refuses_an_empty_buffer() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        let mut state = RecorderState::new(
            "emptysid".into(),
            "false start".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        state.save().unwrap();

        let err = flush(false, false, false, false).unwrap_err().to_string();
        assert!(err.contains("nothing recorded"), "unexpected error: {err}");
        // The recording stays active — the operator can still capture or abandon.
        assert!(paths::record_state_file().exists());
        // And no scenario directory leaked.
        assert!(!tmp.path().join("emptysid").exists());
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
    }

    #[test]
    fn flush_refuses_to_clobber_an_unrelated_scenario() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        // `taken` already holds a sealed scenario the recording knows
        // nothing about (source_ref is None — a fresh recording, not
        // `record continue`/`buffer load`).
        let dir = tmp.path().join("taken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("scenario.json"), b"{\"original\": true}\n").unwrap();
        let mut state = RecorderState::new(
            "taken".into(),
            "fresh recording".into(),
            "default".into(),
            crate::recorder_state::RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        state.steps = serde_json::from_value(json!([
            {"id":"s0","intent":"r","kind":"do","verb":"reload"}
        ]))
        .unwrap();
        state.save().unwrap();

        let err = flush(false, false, false, false).unwrap_err().to_string();
        assert!(err.contains("refusing to overwrite"), "unexpected: {err}");
        // The unrelated scenario survived untouched, and the recording is
        // still active so the operator can re-flush under a free sid.
        assert_eq!(
            fs::read(dir.join("scenario.json")).unwrap(),
            b"{\"original\": true}\n"
        );
        assert!(paths::record_state_file().exists());
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
        flush(false, false, false, false).unwrap();

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
    fn flush_auto_secrets_lifts_password_literals_into_local_inputs() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        let mut state = RecorderState::new(
            "login".into(),
            "login flow".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        record_draft(&mut state, StepKind::Do, &json!({"intent":"go","verb":"goto","value":{"from":"literal","literal":"https://example.com/"}}), "default").unwrap();
        record_draft(&mut state, StepKind::Do, &json!({"intent":"type password","verb":"type","on":{"raw":{"kind":"css","value":"input[type=password]"},"reason":"css"},"value":{"from":"literal","literal":"hunter2"}}), "default").unwrap();
        // A plain literal on a non-secret field stays literal.
        record_draft(&mut state, StepKind::Do, &json!({"intent":"type email","verb":"type","on":{"raw":{"kind":"css","value":"input[name=email]"},"reason":"css"},"value":{"from":"literal","literal":"a@b"}}), "default").unwrap();

        let summary = flush(false, false, false, true).unwrap();
        let scenario: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary.scenario_file).unwrap()).unwrap();
        // The secret became an input ref + a sensitive declaration; the
        // literal is gone from the committable file.
        assert_eq!(
            scenario["steps"][1]["value"],
            json!({ "from": "input", "input": "PASSWORD" })
        );
        assert_eq!(
            scenario["inputs"]["PASSWORD"],
            json!({ "type": "string", "sensitive": true })
        );
        assert_eq!(
            scenario["steps"][2]["value"],
            json!({ "from": "literal", "literal": "a@b" })
        );
        assert!(!scenario.to_string().contains("hunter2"));
        // The real value lives in the gitignored local store replay reads.
        let local: serde_json::Value =
            serde_json::from_slice(&fs::read(tmp.path().join("login/inputs.local.json")).unwrap())
                .unwrap();
        assert_eq!(local["PASSWORD"], "hunter2");
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
    }

    #[test]
    fn flush_auto_secrets_disabled_keeps_the_literal() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        let mut state = RecorderState::new(
            "login2".into(),
            "login flow".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        record_draft(&mut state, StepKind::Do, &json!({"intent":"type password","verb":"type","on":{"raw":{"kind":"css","value":"input[type=password]"},"reason":"css"},"value":{"from":"literal","literal":"hunter2"}}), "default").unwrap();
        let summary = flush(false, false, false, false).unwrap();
        let scenario: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary.scenario_file).unwrap()).unwrap();
        assert_eq!(
            scenario["steps"][0]["value"],
            json!({ "from": "literal", "literal": "hunter2" })
        );
        assert!(scenario["inputs"].is_null() || scenario.get("inputs").is_none());
        assert!(!tmp.path().join("login2/inputs.local.json").exists());
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
            &json!({"intent":"hit save","verb":"click","on":"css:#save"}),
            "default",
        )
        .unwrap();

        let summary = flush(true, false, false, false).unwrap();
        let scenario: serde_json::Value =
            serde_json::from_slice(&fs::read(&summary.scenario_file).unwrap()).unwrap();
        let steps = scenario["steps"].as_array().unwrap();
        // do(goto — skipped: transition frames go flaky), check,
        // do(click), shot — dense ids, the shot ref points at the
        // renumbered do-step it follows.
        assert_eq!(steps.len(), 4);
        assert_eq!(steps[1]["claim"]["subject"]["url"], true);
        assert_eq!(steps[2]["id"], "s2");
        assert_eq!(steps[3]["claim"]["subject"]["shot"], "s2");
        assert_eq!(steps[3]["claim"]["tolerance"]["pixels"], json!(0.05));
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
            ws_frames: vec![],
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
            req("POST", "https://x/app;jsessionid=abc123?x=1", "XHR"),
        ];
        insert_auto_network_claims(&mut steps, &requests);
        // css is dropped, the dup POST collapses to one claim → 3 appended
        assert_eq!(steps.len(), 4);
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
        let json3 = serde_json::to_value(&steps[3]).unwrap();
        assert_eq!(
            json3["claim"]["subject"]["network"]["urlMatches"],
            "https://x/app"
        );
    }

    #[test]
    fn flush_auto_network_rewrites_entity_id_segments() {
        use crate::browser::CapturedRequest;
        let req = |method: &str, url: &str, rt: &str| CapturedRequest {
            request_id: String::new(),
            url: url.to_string(),
            method: method.to_string(),
            status: Some(200),
            resource_type: Some(rt.to_string()),
            mime_type: None,
            post_data: None,
            ws_frames: vec![],
        };
        let mut steps = vec![];
        insert_auto_network_claims(
            &mut steps,
            &[
                req(
                    "GET",
                    "https://x/api/contacts/6abbfc58b45a2a0015047afa",
                    "XHR",
                ),
                // a second entity id collapses onto the same pattern claim
                req(
                    "GET",
                    "https://x/api/contacts/00112233445566778899aabb",
                    "XHR",
                ),
                req("GET", "https://x/api/items/1234567", "XHR"),
                req("GET", "https://x/api/v2/users", "XHR"),
            ],
        );
        assert_eq!(steps.len(), 3);
        let urls: Vec<String> = steps
            .iter()
            .map(|s| {
                serde_json::to_value(s).unwrap()["claim"]["subject"]["network"]["urlMatches"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(urls[0], "https://x/api/contacts/[0-9a-fA-F-]{8,}");
        assert_eq!(urls[1], "https://x/api/items/\\d+");
        // short segments and api versions stay literal
        assert_eq!(urls[2], "https://x/api/v2/users");
    }

    #[test]
    fn flush_auto_network_claims_normalize_volatile_urls() {
        use crate::browser::CapturedRequest;
        let mut steps = vec![];
        let requests = vec![
            CapturedRequest {
                request_id: String::new(),
                url: "https://x/app/login.htm;jsessionid=ABC123".into(),
                method: "POST".into(),
                status: Some(200),
                resource_type: Some("XHR".into()),
                mime_type: None,
                post_data: None,
                ws_frames: vec![],
            },
            CapturedRequest {
                request_id: String::new(),
                url: "https://x/services_proxy/bank/customers/63827".into(),
                method: "GET".into(),
                status: Some(200),
                resource_type: Some("XHR".into()),
                mime_type: None,
                post_data: None,
                ws_frames: vec![],
            },
            CapturedRequest {
                request_id: String::new(),
                url: "https://x/api/items/550e8400-e29b-41d4-a716".into(),
                method: "GET".into(),
                status: Some(200),
                resource_type: Some("Fetch".into()),
                mime_type: None,
                post_data: None,
                ws_frames: vec![],
            },
        ];
        insert_auto_network_claims(&mut steps, &requests);
        let json = serde_json::to_value(&steps[0]).unwrap();
        assert_eq!(
            json["claim"]["subject"]["network"]["urlMatches"],
            "https://x/app/login.htm"
        );
        let json = serde_json::to_value(&steps[1]).unwrap();
        assert_eq!(
            json["claim"]["subject"]["network"]["urlMatches"],
            "https://x/services_proxy/bank/customers/\\d+"
        );
        let json = serde_json::to_value(&steps[2]).unwrap();
        assert_eq!(
            json["claim"]["subject"]["network"]["urlMatches"],
            "https://x/api/items/[0-9a-fA-F-]{8,}"
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
            ws_frames: vec![],
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

    #[test]
    fn package_file_literals_copies_and_rewrites_absolute_paths() {
        let tmp = TempDir::new().unwrap();
        let upload = tmp.path().join("avatar.png");
        fs::write(&upload, b"png-bytes").unwrap();
        let scenario_dir = tmp.path().join("scn");
        fs::create_dir_all(&scenario_dir).unwrap();
        let abs = upload.display().to_string();
        let mut doc = json!({
            "steps": [
                {"kind":"do","id":"s0","intent":"up","verb":"upload",
                 "value":{"from":"literal","literal": abs}},
                {"kind":"do","id":"s1","intent":"pick","verb":"fileChooser",
                 "params":{"files":[abs]}},
                {"kind":"do","id":"s2","intent":"rel","verb":"upload",
                 "value":{"from":"literal","literal":"files/already.png"}},
                {"kind":"do","id":"s3","intent":"miss","verb":"upload",
                 "value":{"from":"literal","literal":"/nonexistent/x.png"}},
                {"kind":"check","id":"s4","intent":"noop",
                 "claim":{"subject":{"url":true},"predicate":"exists"}}
            ]
        });
        let packaged = package_file_literals(&mut doc, &scenario_dir);
        // Same source dedupes to one copy under files/; relative and
        // missing literals stay untouched.
        assert_eq!(
            packaged,
            vec![
                "files/avatar.png".to_string(),
                "files/avatar.png".to_string()
            ]
        );
        assert_eq!(doc["steps"][0]["value"]["literal"], "files/avatar.png");
        assert_eq!(doc["steps"][1]["params"]["files"][0], "files/avatar.png");
        assert_eq!(doc["steps"][2]["value"]["literal"], "files/already.png");
        assert_eq!(doc["steps"][3]["value"]["literal"], "/nonexistent/x.png");
        assert_eq!(
            fs::read(scenario_dir.join("files/avatar.png"))
                .unwrap()
                .as_slice(),
            b"png-bytes"
        );
        assert!(!scenario_dir.join("files/avatar-2.png").exists());
    }

    #[test]
    fn package_one_path_suffixes_same_named_different_files() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("logo.png"), b"a").unwrap();
        fs::write(b.join("logo.png"), b"b").unwrap();
        let scenario_dir = tmp.path().join("scn");
        fs::create_dir_all(&scenario_dir).unwrap();
        let mut by_src = HashMap::new();
        let first = package_one_path(
            a.join("logo.png").to_str().unwrap(),
            &scenario_dir,
            &mut by_src,
        );
        let second = package_one_path(
            b.join("logo.png").to_str().unwrap(),
            &scenario_dir,
            &mut by_src,
        );
        assert_eq!(first.as_deref(), Some("files/logo.png"));
        assert_eq!(second.as_deref(), Some("files/logo-2.png"));
        assert_eq!(
            fs::read(scenario_dir.join("files/logo.png"))
                .unwrap()
                .as_slice(),
            b"a"
        );
        assert_eq!(
            fs::read(scenario_dir.join("files/logo-2.png"))
                .unwrap()
                .as_slice(),
            b"b"
        );
    }
}
