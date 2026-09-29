//! `record-step` appends one direct scenario/2 step to the active recorder.

use std::fs;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value as Json;

use crate::browser;
use crate::recorder_state::RecorderState;
use crate::scenario::Step;
use crate::schema;
use crate::verb_shape;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    match record(&opts)? {
        Some(row) => println!("recorded step {} (stepId={})", row.step_index, row.step_id),
        None => println!("skipped (recording paused — `record resume` to capture)"),
    }
    Ok(0)
}

fn print_help() {
    let help = r#"agent-qa record-step - append one scenario/2 step to the active recording

Usage:
  agent-qa record-step <do|check> <draft-json>

The recorder assigns id and kind. The draft must omit both fields.

Examples:
  agent-qa record-step do '{"intent":"open users","verb":"goto","value":{"from":"literal","literal":"https://example.com/users"}}'
  agent-qa record-step do '{"intent":"open editor","verb":"click","on":{"role":"button","name":"Edit user"}}'
  agent-qa record-step check '{"intent":"editor visible","claim":{"subject":{"element":{"role":"dialog","name":"Edit user"}},"predicate":"isVisible"}}'

The command records only. Drive the browser first, then record the settled step.
Each recorded step gets a best-effort snapshot and screenshot sidecar."#;
    println!("{help}");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepKind {
    Do,
    Check,
}

impl StepKind {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "do" => Ok(Self::Do),
            "check" => Ok(Self::Check),
            other => bail!("record-step: kind must be one of [do, check]; got {other:?}"),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Do => "do",
            Self::Check => "check",
        }
    }
}

#[derive(Debug, Clone)]
struct Opts {
    kind: StepKind,
    payload: Json,
}

#[derive(Debug, Clone)]
pub(crate) struct RecordedStep {
    pub(crate) step_index: usize,
    pub(crate) step_id: String,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        print_help();
        std::process::exit(0);
    }
    if args.len() != 2 {
        bail!("usage: record-step <do|check> <draft-json>");
    }
    let kind = StepKind::parse(&args[0])?;
    let payload: Json = serde_json::from_str(&args[1])
        .with_context(|| format!("parse draft JSON: {:?}", args[1]))?;
    Ok(Opts { kind, payload })
}

fn record(opts: &Opts) -> Result<Option<RecordedStep>> {
    let mut state = RecorderState::load_active()?;
    let session = state.session.clone();
    record_draft(&mut state, opts.kind, &opts.payload, &session)
}

/// Appends one validated step, or `Ok(None)` when the recorder is paused —
/// callers keep their action (the click/fill already happened) but nothing
/// lands in the buffer until `record resume`.
pub(crate) fn record_draft(
    state: &mut RecorderState,
    kind: StepKind,
    payload: &Json,
    session: &str,
) -> Result<Option<RecordedStep>> {
    if state.paused {
        return Ok(None);
    }
    let step_index = state.steps.len();
    let step_id = format!("s{step_index}");
    let step = parse_draft(kind, payload, &step_id)?;
    let is_click = matches!(&step, Step::Do { verb, .. } if click_verb(verb));
    let is_do = matches!(&step, Step::Do { .. });
    state.steps.push(step);
    state.save()?;
    // Click-effect advisory: only meaningful while a dialog isn't blocking
    // evals (the pending dialog IS the click's effect).
    if is_do && !browser::dialog_pending(session) {
        if is_click {
            warn_if_click_inert(session);
        }
        arm_click_fx(session);
    }
    capture_step_sidecars(&state.sid, session, &step_id);
    Ok(Some(RecordedStep {
        step_index,
        step_id,
    }))
}

/// Persistent (per-document) click-effect probe. A capture-phase click
/// listener snapshots the DOM mutation count, URL, and resource count at each
/// click; `record-step` later diffs the latest snapshot against the present.
/// A click that produced nothing — no navigation, mutation, or fetch — is
/// almost always a handler that was not bound yet (post-commit-effect apps)
/// and earns an advisory while the session is still open to retry.
///
/// The probe dies on navigation, so `record-step` re-arms it after every do
/// step: the step's own click is covered when the previous step re-armed.
const CLICK_FX_ARM: &str = "(function(){try{if(window.__aqFx)return '1';window.__aqFx={m:0,clicks:[]};new MutationObserver(function(x){window.__aqFx.m+=x.length}).observe(document.documentElement,{subtree:true,childList:true,attributes:true,characterData:true});document.addEventListener('click',function(){var f=window.__aqFx;if(!f)return;if(f.clicks.length>20)f.clicks.shift();var a=document.activeElement;f.clicks.push({t:Date.now(),m:f.m,u:location.href,r:performance.getEntriesByType('resource').length,a:a?a.tagName+'#'+(a.id||''):''})},true);return '1'}catch(e){return '1'}})()";

const CLICK_FX_CHECK: &str = "(function(){var p=window.__aqFx;if(!p)return '';var c=p.clicks[p.clicks.length-1];if(!c||Date.now()-c.t>30000)return '';var a=document.activeElement;var fa=(a?a.tagName+'#'+(a.id||''):'')!==c.a;return JSON.stringify({dm:p.m-c.m,nav:location.href!==c.u,dr:performance.getEntriesByType('resource').length-c.r,fa:fa})})()";

fn click_verb(verb: &crate::scenario::Verb) -> bool {
    matches!(
        verb,
        crate::scenario::Verb::Click
            | crate::scenario::Verb::DblClick
            | crate::scenario::Verb::RightClick
            | crate::scenario::Verb::Check
            | crate::scenario::Verb::Uncheck
    )
}

/// `Some(true)` when the probe saw a click that produced no navigation, DOM
/// mutation, or resource fetch; `None`/`Some(false)` are both "no warning".
fn click_inert(raw: &str) -> Option<bool> {
    let raw = raw.trim();
    // eval results arrive JSON-encoded: a JS string result reads
    // `"{\"dm\":0,...}"` — decode the outer layer first, then the payload.
    let payload = serde_json::from_str::<String>(raw).unwrap_or_else(|_| raw.to_string());
    let payload = payload.trim();
    if payload.is_empty() {
        return None;
    }
    let parsed: Json = serde_json::from_str(payload).ok()?;
    let dm = parsed.get("dm").and_then(|v| v.as_i64()).unwrap_or(0);
    let nav = parsed.get("nav").and_then(|v| v.as_bool()).unwrap_or(true);
    let dr = parsed.get("dr").and_then(|v| v.as_i64()).unwrap_or(0);
    // Focus moving (e.g. clicking to focus an input) counts as an effect.
    let fa = parsed.get("fa").and_then(|v| v.as_bool()).unwrap_or(false);
    Some(dm == 0 && !nav && dr == 0 && !fa)
}

/// Re-install the probe if the page navigated it away. Best-effort: a dead
/// or dialog-blocked session just skips.
fn arm_click_fx(session: &str) {
    let _ = browser::eval_expression(session, CLICK_FX_ARM);
}

fn warn_if_click_inert(session: &str) {
    let Ok(raw) = browser::eval_expression(session, CLICK_FX_CHECK) else {
        return;
    };
    if click_inert(&raw) == Some(true) {
        eprintln!(
            "[v2-record] click produced no observable effect (no navigation, DOM mutation, or network) — the app's handler may not have been bound yet; a short wait before clicking usually fixes it"
        );
    }
}

/// Arms the click-effect probe at record start so the first page's clicks
/// are covered. Public for `start`.
pub(crate) fn arm_click_probe(session: &str) {
    if browser::dialog_pending(session) {
        return;
    }
    arm_click_fx(session);
}

pub(crate) fn parse_draft(kind: StepKind, payload: &Json, step_id: &str) -> Result<Step> {
    let mut draft = payload.as_object().cloned().ok_or_else(|| {
        anyhow!(
            "record-step {kind}: draft must be a JSON object",
            kind = kind.as_str()
        )
    })?;
    if draft.contains_key("id") || draft.contains_key("kind") {
        bail!(
            "record-step {} draft must not contain id or kind",
            kind.as_str()
        );
    }
    draft.insert("id".into(), Json::String(step_id.to_string()));
    draft.insert("kind".into(), Json::String(kind.as_str().to_string()));
    let step: Step =
        serde_json::from_value(Json::Object(draft)).context("parse scenario/2 step")?;
    match (&kind, &step) {
        (StepKind::Do, Step::Do { .. }) => verb_shape::assert_verb_shape(&step)?,
        (StepKind::Check, Step::Check { .. }) => {}
        _ => bail!(
            "record-step {} draft did not parse as that step kind",
            kind.as_str()
        ),
    }
    schema::validate_value(&serde_json::json!({
        "schema": "scenario/2",
        "id": "recording-validation",
        "intent": "validate recorded step",
        "steps": [step],
    }))
    .context("recorded step failed scenario schema validation")?;
    Ok(step)
}

pub(crate) fn capture_step_sidecars(sid: &str, session: &str, step_id: &str) {
    if std::env::var("AGENT_QA_RECORD_SKIP_SIDECARS")
        .ok()
        .as_deref()
        == Some("1")
    {
        return;
    }
    match crate::paths::scenario_dir(sid) {
        Ok(scenario_dir) => {
            let _ = capture_recording_sidecars(&scenario_dir, step_id, session);
        }
        Err(e) => eprintln!("[record] sidecar capture: resolve scenario dir failed: {e}"),
    }
}

fn capture_recording_sidecars(
    scenario_dir: &std::path::Path,
    step_id: &str,
    session: &str,
) -> Result<()> {
    use crate::sidecar::write_step_sidecar;
    let run = crate::sidecar::RunPaths {
        scenario_dir: scenario_dir.to_path_buf(),
        run_id: "recording".into(),
        run_root: scenario_dir.join("recording"),
    };
    fs::create_dir_all(&run.run_root).ok();
    // A pending native dialog blocks the page: skip BOTH sidecars so
    // `verify`'s snapshot+screenshot pairing stays consistent — the dialog
    // step that resolves it lands the next step's capture instead.
    if browser::dialog_pending(session) {
        return Ok(());
    }
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
    if let Ok(path) =
        crate::sidecar::step_sidecar_path(&run, crate::sidecar::SidecarKind::Screenshots, step_id)
    {
        let _ = crate::sidecar::ensure_kind_dir(&run, crate::sidecar::SidecarKind::Screenshots);
        if let Err(e) = browser::screenshot(session, &path, true, None) {
            eprintln!("[record-step] screenshot {step_id} failed: {e}");
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn install_fake_browser(dir: &std::path::Path) {
        let path = dir.join("agent-browser");
        fs::write(&path, "#!/bin/sh\nif [ \"$3\" = screenshot ]; then shift 3; [ \"$1\" = --full ] && shift; : > \"$1\"; fi\n").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, &path);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn record_appends_typed_direct_steps() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path().join("record"));
        install_fake_browser(tmp.path());
        RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        )
        .save()
        .unwrap();

        let first = record(&Opts { kind: StepKind::Do, payload: json!({"intent":"open","verb":"goto","value":{"from":"literal","literal":"https://example.com/"}}) }).unwrap().unwrap();
        let second = record(&Opts { kind: StepKind::Check, payload: json!({"intent":"loaded","claim":{"subject":{"url":true},"predicate":"exists"}}) }).unwrap().unwrap();
        let state = RecorderState::load_active().unwrap();

        assert_eq!(first.step_id, "s0");
        assert_eq!(second.step_id, "s1");
        assert_eq!(state.steps.len(), 2);
        assert!(matches!(state.steps[0], Step::Do { .. }));
        assert!(matches!(state.steps[1], Step::Check { .. }));
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn click_inert_reads_the_probe_result() {
        // No probe / no recent click → advisory suppressed.
        assert_eq!(click_inert("\"\""), None);
        assert_eq!(click_inert(""), None);
        assert_eq!(click_inert("not json"), None);
        // Zero delta across mutation/url/resources → inert.
        assert_eq!(
            click_inert("\"{\\\"dm\\\":0,\\\"nav\\\":false,\\\"dr\\\":0}\""),
            Some(true)
        );
        // Any single signal discharges the warning.
        assert_eq!(
            click_inert("\"{\\\"dm\\\":4,\\\"nav\\\":false,\\\"dr\\\":0}\""),
            Some(false)
        );
        assert_eq!(
            click_inert("\"{\\\"dm\\\":0,\\\"nav\\\":false,\\\"dr\\\":0,\\\"fa\\\":true}\""),
            Some(false)
        );
        assert_eq!(
            click_inert("\"{\\\"dm\\\":0,\\\"nav\\\":true,\\\"dr\\\":0}\""),
            Some(false)
        );
        assert_eq!(
            click_inert("\"{\\\"dm\\\":0,\\\"nav\\\":false,\\\"dr\\\":2}\""),
            Some(false)
        );
    }

    #[test]
    fn parse_draft_rejects_legacy_kind_and_injected_fields() {
        assert!(StepKind::parse("navigation").is_err());
        let err = parse_draft(
            StepKind::Do,
            &json!({"id":"bad","intent":"x","verb":"reload"}),
            "s0",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("must not contain id"));
    }
}
