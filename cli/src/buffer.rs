//! `buffer` inspects and edits typed steps in the active recorder state.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use std::collections::HashMap;

use crate::recorder_state::RecorderState;
use crate::scenario::{Scenario, Step, StepContext, Value};

pub fn run(args: &[String]) -> Result<u8> {
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return Ok(0);
    }
    match args[0].as_str() {
        "list" => cmd_list(&args[1..]),
        "insert" => cmd_insert(&args[1..]),
        "delete" | "rm" => cmd_delete(&args[1..]),
        "move" | "mv" => cmd_move(&args[1..]),
        "edit" => cmd_edit(&args[1..]),
        "load" => cmd_load(&args[1..]),
        "check" => cmd_check(&args[1..]),
        "clear" => cmd_clear(),
        "discard" => cmd_discard(),
        other => {
            bail!("buffer: unknown subcommand {other:?}; try list|insert|delete|move|edit|load|check|clear|discard")
        }
    }
}

fn print_help() {
    println!(
        "agent-qa buffer - inspect or edit the active recording

Usage:
  agent-qa buffer list [--json]
  agent-qa buffer insert <index> <do|check> <draft-json>
  agent-qa buffer delete <index>
  agent-qa buffer move <from> <to>
  agent-qa buffer edit <index> <draft-json>
  agent-qa buffer load <sid> [--force]
  agent-qa buffer check [--strict] [--format text|json|github]
  agent-qa buffer clear
  agent-qa buffer discard

Insert appends a validated draft at <index> (use <len> to append), then
edit replaces the step at <index> with a re-validated draft (same shape as
`record-step`, minus id/kind — the step keeps its id and position; its kind
may not change). Insert, delete, and move reassign dense s0, s1, ... ids; step
references (from=step stepId, opensFromStepId) are rewired to match. Load
pulls a saved scenario's steps into the buffer for editing (`flush` writes
them back to the same sid, preserving fields the buffer doesn't model —
inputs, templates, env.close). Check runs the `scenario check` verifier
(schema + lint) on the scenario flush would write, without writing it.
Discard removes the active recording."
    );
}

fn parse_index(value: &str, label: &str) -> Result<usize> {
    value
        .parse()
        .map_err(|_| anyhow!("{label} must be a non-negative integer; got {value:?}"))
}

pub(crate) fn normalize_ids(steps: &mut [Step]) -> HashMap<String, String> {
    // Rewire `{"from":"step","stepId":…}` values and `opensFromStepId`
    // before renumbering so references keep pointing at the same step.
    // References to a step that no longer exists keep their old id.
    let renames: HashMap<String, String> = steps
        .iter()
        .enumerate()
        .map(|(index, step)| (step.id().to_string(), format!("s{index}")))
        .filter(|(old, new)| old != new)
        .collect();
    if !renames.is_empty() {
        for step in steps.iter_mut() {
            rewrite_step_refs(step, &renames);
        }
    }
    for (index, step) in steps.iter_mut().enumerate() {
        step.set_id(format!("s{index}"));
    }
    renames
}

fn rewrite_step_refs(step: &mut Step, renames: &HashMap<String, String>) {
    match step {
        Step::Do {
            value,
            params,
            context,
            ..
        } => {
            if let Some(Value::Step { step_id, .. }) = value {
                if let Some(new) = renames.get(step_id.as_str()) {
                    *step_id = new.clone();
                }
            }
            if let Some(params) = params {
                for item in params.values_mut() {
                    rewrite_step_refs_json(item, renames);
                }
            }
            rewrite_context_refs(context, renames);
        }
        Step::Check { claim, context, .. } => {
            if let Some(value) = &mut claim.value {
                rewrite_step_refs_json(value, renames);
            }
            // {"shot"/"domshot": "<stepId>"} references a step id like any
            // other step ref — renumbering must rewire it or the claim
            // dangles.
            match &mut claim.subject {
                crate::scenario::ClaimSubject::Shot { shot, .. } => {
                    if let Some(new) = renames.get(shot.as_str()) {
                        *shot = new.clone();
                    }
                }
                crate::scenario::ClaimSubject::Domshot { domshot, .. } => {
                    if let Some(new) = renames.get(domshot.as_str()) {
                        *domshot = new.clone();
                    }
                }
                _ => {}
            }
            rewrite_context_refs(context, renames);
        }
    }
}

fn rewrite_context_refs(context: &mut Option<StepContext>, renames: &HashMap<String, String>) {
    if let Some(tab) = context.as_mut().and_then(|c| c.tab.as_mut()) {
        if let Some(id) = &mut tab.opens_from_step_id {
            if let Some(new) = renames.get(id.as_str()) {
                *id = new.clone();
            }
        }
    }
}

fn rewrite_step_refs_json(v: &mut serde_json::Value, renames: &HashMap<String, String>) {
    match v {
        serde_json::Value::Object(map) => {
            if map.get("from").and_then(|f| f.as_str()) == Some("step") {
                if let Some(serde_json::Value::String(step_id)) = map.get_mut("stepId") {
                    if let Some(new) = renames.get(step_id.as_str()) {
                        *step_id = new.clone();
                    }
                }
            }
            for item in map.values_mut() {
                rewrite_step_refs_json(item, renames);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                rewrite_step_refs_json(item, renames);
            }
        }
        _ => {}
    }
}

fn step_json(index: usize, step: &Step) -> serde_json::Value {
    json!({
        "stepIndex": index,
        "stepId": step.id(),
        "step": step,
    })
}

fn cmd_list(args: &[String]) -> Result<u8> {
    if args.iter().any(|arg| arg != "--json") {
        bail!("usage: buffer list [--json]");
    }
    let state = RecorderState::load_active()?;
    if args.iter().any(|arg| arg == "--json") {
        let rows: Vec<_> = state
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| step_json(i, step))
            .collect();
        println!(
            "{}",
            serde_json::to_string(&json!({
                "sid": state.sid,
                "intent": state.intent,
                "session": state.session,
                "baseline": state.baseline,
                "paused": state.paused,
                "editing": state.original.is_some(),
                "rows": rows,
            }))?
        );
    } else if state.steps.is_empty() {
        println!("(buffer is empty)");
    } else {
        for (index, step) in state.steps.iter().enumerate() {
            println!(
                "[{index}] {}: {}",
                match step {
                    Step::Do { .. } => "do",
                    Step::Check { .. } => "check",
                },
                step.intent()
            );
        }
    }
    Ok(0)
}

fn cmd_delete(args: &[String]) -> Result<u8> {
    let index = args
        .first()
        .ok_or_else(|| anyhow!("usage: buffer delete <index>"))?;
    let index = parse_index(index, "index")?;
    let mut state = RecorderState::load_active()?;
    if index >= state.steps.len() {
        bail!(
            "index {index} out of range (buffer has {} step(s))",
            state.steps.len()
        );
    }
    state.steps.remove(index);
    normalize_ids(&mut state.steps);
    state.save()?;
    println!("deleted step {index}; {} step(s) remain", state.steps.len());
    Ok(0)
}

fn cmd_move(args: &[String]) -> Result<u8> {
    if args.len() != 2 {
        bail!("usage: buffer move <from> <to>");
    }
    let from = parse_index(&args[0], "from index")?;
    let to = parse_index(&args[1], "to index")?;
    let mut state = RecorderState::load_active()?;
    let len = state.steps.len();
    if from >= len || to >= len {
        bail!("step index out of range (buffer has {len} step(s))");
    }
    let step = state.steps.remove(from);
    state.steps.insert(to, step);
    normalize_ids(&mut state.steps);
    state.save()?;
    println!("moved step {from} to {to}; {len} step(s)");
    Ok(0)
}

fn cmd_edit(args: &[String]) -> Result<u8> {
    if args.len() != 2 {
        bail!("usage: buffer edit <index> <draft-json>");
    }
    let index = parse_index(&args[0], "index")?;
    let mut state = RecorderState::load_active()?;
    let existing = state.steps.get(index).ok_or_else(|| {
        anyhow!(
            "index {index} out of range (buffer has {} step(s))",
            state.steps.len()
        )
    })?;
    let kind = match existing {
        Step::Do { .. } => crate::record_step::StepKind::Do,
        Step::Check { .. } => crate::record_step::StepKind::Check,
    };
    let step_id = existing.id().to_string();
    let payload: serde_json::Value = serde_json::from_str(&args[1])
        .map_err(|e| anyhow!("parse draft JSON: {:?} ({e})", args[1]))?;
    let replacement = crate::record_step::parse_draft(kind, &payload, &step_id)?;
    state.steps[index] = replacement;
    state.save()?;
    println!("edited step {index} (stepId={step_id})");
    Ok(0)
}

fn cmd_insert(args: &[String]) -> Result<u8> {
    if args.len() != 3 {
        bail!("usage: buffer insert <index> <do|check> <draft-json>");
    }
    let index = parse_index(&args[0], "index")?;
    let kind = crate::record_step::StepKind::parse(&args[1])?;
    let mut state = RecorderState::load_active()?;
    if index > state.steps.len() {
        bail!(
            "index {index} out of range (buffer has {} step(s))",
            state.steps.len()
        );
    }
    let step_id = format!("s{index}");
    let payload: serde_json::Value = serde_json::from_str(&args[2])
        .map_err(|e| anyhow!("parse draft JSON: {:?} ({e})", args[2]))?;
    let step = crate::record_step::parse_draft(kind, &payload, &step_id)?;
    state.steps.insert(index, step);
    normalize_ids(&mut state.steps);
    state.save()?;
    println!(
        "inserted {kind} step at {index} (stepId={step_id}); {len} step(s)",
        kind = kind.as_str(),
        len = state.steps.len()
    );
    Ok(0)
}

fn cmd_check(args: &[String]) -> Result<u8> {
    let strict = args.iter().any(|a| a == "--strict");
    let mut format = crate::scenario_cli::LintFormat::Text;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--strict" => {}
            "--format" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--format requires a value"))?;
                format = crate::scenario_cli::parse_lint_format(v)?;
            }
            "--json" => format = crate::scenario_cli::LintFormat::Json,
            _ => {
                if let Some(v) = a.strip_prefix("--format=") {
                    format = crate::scenario_cli::parse_lint_format(v)?;
                } else {
                    bail!("usage: buffer check [--strict] [--format text|json|github]");
                }
            }
        }
    }
    let state = RecorderState::load_active()?;
    let scenario_json = crate::flush::assemble_scenario(&state)?;
    let mut tmp = tempfile::NamedTempFile::new().context("open buffer-check tempfile")?;
    use std::io::Write;
    tmp.write_all(serde_json::to_string_pretty(&scenario_json)?.as_bytes())?;
    let code = crate::scenario_cli::check(tmp.path(), strict, format)?;
    // The compact text report stops at counts — print the findings too.
    if code != 0 && format == crate::scenario_cli::LintFormat::Text {
        crate::scenario_cli::lint(tmp.path(), format, strict, None, None)?;
    }
    Ok(code)
}

fn cmd_clear() -> Result<u8> {
    let mut state = RecorderState::load_active()?;
    state.steps.clear();
    state.save()?;
    println!("buffer cleared");
    Ok(0)
}

fn cmd_discard() -> Result<u8> {
    RecorderState::clear()?;
    println!("recording discarded");
    Ok(0)
}

/// `buffer load <sid> [--force]` pulls a saved scenario's steps into the
/// buffer so the buffer ops (edit/move/delete) apply to it; `flush` seals
/// it back over the same sid. Refuses to clobber a non-empty buffer unless
/// --force. The original document rides along on the state so `flush` can
/// hand back the fields it doesn't model.
fn cmd_load(args: &[String]) -> Result<u8> {
    let mut sid: Option<&str> = None;
    let mut force = false;
    for a in args {
        match a.as_str() {
            "--force" => force = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!("unexpected positional {other:?}; usage: buffer load <sid> [--force]");
                }
                sid = Some(other);
            }
        }
    }
    let sid = sid.ok_or_else(|| anyhow!("usage: buffer load <sid> [--force]"))?;
    let steps = load_into_buffer(sid, "default", force)?;
    println!("loaded {sid} into the buffer ({steps} step(s))");
    Ok(0)
}

/// Seed the active recorder state with a saved scenario's steps (same sid,
/// so `flush` writes back to it). `session` is the browser session the
/// caller intends to keep driving — record-step captures sidecars there.
/// Returns the number of seeded steps. Reused by `record continue`.
pub(crate) fn load_into_buffer(sid: &str, session: &str, force: bool) -> Result<usize> {
    if let Some(existing) = RecorderState::try_load_active()? {
        if !existing.steps.is_empty() && !force {
            bail!(
                "buffer already holds {} step(s) for sid {:?} — flush or discard first (or --force)",
                existing.steps.len(),
                existing.sid,
            );
        }
    }
    let scenario_file = crate::paths::scenario_dir(sid)?.join("scenario.json");
    let bytes = std::fs::read(&scenario_file)
        .with_context(|| format!("read {}", scenario_file.display()))?;
    let value = crate::schema::validate_bytes(&bytes)
        .with_context(|| format!("validate {}", scenario_file.display()))?;
    let sc: Scenario = serde_json::from_value(value.clone())
        .with_context(|| format!("parse {} as Scenario", scenario_file.display()))?;
    let mut state = RecorderState::new(
        sid.to_string(),
        sc.intent.clone(),
        session.to_string(),
        crate::recorder_state::RecorderBaseline::KeepSession,
        Some(format!("scenario:{sid}")),
        crate::browser::BrowserConnection::default(),
    );
    state.env_open = sc
        .env
        .as_ref()
        .and_then(|e| e.open.clone())
        .unwrap_or_default();
    state.steps = sc.steps.clone();
    if let Some(recorded_at) = sc.produced_by.as_ref().and_then(|p| p.recorded_at.clone()) {
        state.started_at = recorded_at;
    }
    state.original = Some(value);
    let steps = state.steps.len();
    state.save()?;
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    fn state() -> RecorderState {
        let mut state = RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"first","kind":"do","verb":"reload"},
            {"id":"s1","intent":"second","kind":"check","claim":{"subject":{"url":true},"predicate":"exists"}}
        ])).unwrap();
        state
    }

    #[test]
    fn move_and_delete_keep_dense_ids() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        state().save().unwrap();
        cmd_move(&["1".into(), "0".into()]).unwrap();
        cmd_delete(&["0".into()]).unwrap();
        let state = RecorderState::load_active().unwrap();
        assert_eq!(state.steps.len(), 1);
        assert_eq!(state.steps[0].id(), "s0");
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn move_rewires_step_refs() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        let mut state = state();
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"produce","kind":"do","verb":"reload"},
            {"id":"s1","intent":"consume","kind":"do","verb":"type",
             "on":{"raw":{"kind":"css","value":"#x"},"reason":"test"},
             "value":{"from":"step","stepId":"s0","path":"data.id"}},
            {"id":"s2","intent":"assert","kind":"check",
             "claim":{"subject":{"url":true},"predicate":"contains",
                      "value":{"from":"step","stepId":"s1"}},
             "context":{"tab":{"opensFromStepId":"s1"}}}
        ]))
        .unwrap();
        state.save().unwrap();
        // Move last → first: old ids s2,s0,s1 become s0,s1,s2.
        cmd_move(&["2".into(), "0".into()]).unwrap();
        let steps = RecorderState::load_active().unwrap().steps;
        let check = serde_json::to_value(&steps[0]).unwrap();
        assert_eq!(check["claim"]["value"]["stepId"], "s2"); // old s1 → new s2
        assert_eq!(check["context"]["tab"]["opensFromStepId"], "s2");
        let last = serde_json::to_value(&steps[2]).unwrap();
        assert_eq!(last["value"]["stepId"], "s1"); // old s0 → new s1
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn delete_rewires_shot_claim_refs() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        let mut state = state();
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"drop","kind":"do","verb":"reload"},
            {"id":"s1","intent":"capture","kind":"do","verb":"reload"},
            {"id":"s2","intent":"assert visual","kind":"check",
             "claim":{"subject":{"shot":"s1"},"predicate":"matches"}}
        ]))
        .unwrap();
        state.save().unwrap();
        // Removing s0 renumbers s1→s0; the shot claim must follow it.
        cmd_delete(&["0".into()]).unwrap();
        let steps = RecorderState::load_active().unwrap().steps;
        let check = serde_json::to_value(&steps[1]).unwrap();
        assert_eq!(check["claim"]["subject"]["shot"], "s0");
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn delete_rewires_refs_to_survivors() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        let mut state = state();
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"drop","kind":"do","verb":"reload"},
            {"id":"s1","intent":"keep","kind":"do","verb":"reload"},
            {"id":"s2","intent":"assert","kind":"check",
             "claim":{"subject":{"url":true},"predicate":"contains",
                      "value":{"from":"step","stepId":"s1"}}}
        ]))
        .unwrap();
        state.save().unwrap();
        cmd_delete(&["0".into()]).unwrap();
        let steps = RecorderState::load_active().unwrap().steps;
        assert_eq!(steps.len(), 2);
        let check = serde_json::to_value(&steps[1]).unwrap();
        assert_eq!(check["claim"]["value"]["stepId"], "s0"); // old s1 → new s0
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn insert_adds_a_validated_step_and_renumbers() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        state().save().unwrap();
        cmd_insert(&[
            "1".into(),
            "check".into(),
            "{\"intent\":\"page looks right\",\"claim\":{\"subject\":{\"shot\":\"s0\"},\"predicate\":\"matches\"}}".into(),
        ])
        .unwrap();
        let state = RecorderState::load_active().unwrap();
        assert_eq!(state.steps.len(), 3);
        assert_eq!(state.steps[1].id(), "s1");
        assert_eq!(state.steps[1].intent(), "page looks right");
        // Steps after the insert point renumber densely.
        assert_eq!(state.steps[2].id(), "s2");
        // Out-of-range index and malformed drafts are rejected without writing.
        assert!(cmd_insert(&["9".into(), "check".into(), "{}".into()]).is_err());
        assert!(cmd_insert(&["0".into(), "nope".into(), "{}".into()]).is_err());
        assert_eq!(RecorderState::load_active().unwrap().steps.len(), 3);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn check_runs_schema_and_lint_on_flush_doc() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        // A do/click with no locator → lint error → check exits 1.
        let mut state = state();
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"click without locator","kind":"do","verb":"click"},
        ]))
        .unwrap();
        state.save().unwrap();
        assert_eq!(cmd_check(&["--json".into()]).unwrap(), 1);
        // Fix the buffer → check exits 0.
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"click it","kind":"do","verb":"click",
             "on":{"raw":{"kind":"css","value":"#x"},"reason":"test"}},
            {"id":"s1","intent":"it worked","kind":"check",
             "claim":{"subject":{"url":true},"predicate":"exists"}}
        ]))
        .unwrap();
        state.save().unwrap();
        assert_eq!(cmd_check(&["--json".into()]).unwrap(), 0);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }

    #[test]
    fn edit_replaces_step_keeping_id_and_position() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        state().save().unwrap();
        cmd_edit(&[
            "1".into(),
            "{\"intent\":\"url now\",\"claim\":{\"subject\":{\"url\":true},\"predicate\":\"contains\",\"value\":\"/done\"}}".into(),
        ])
        .unwrap();
        let state = RecorderState::load_active().unwrap();
        assert_eq!(state.steps.len(), 2);
        assert_eq!(state.steps[1].id(), "s1");
        assert_eq!(state.steps[1].intent(), "url now");
        assert!(matches!(state.steps[1], Step::Check { .. }));
        // Kind mismatch and malformed drafts are rejected without writing.
        assert!(cmd_edit(&["1".into(), "{\"intent\":\"x\",\"verb\":\"reload\"}".into()]).is_err());
        assert!(cmd_edit(&["9".into(), "{}".into()]).is_err());
        assert_eq!(
            RecorderState::load_active().unwrap().steps[1].intent(),
            "url now"
        );
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }
}
