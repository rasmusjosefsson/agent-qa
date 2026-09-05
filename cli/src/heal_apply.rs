//! `heal-apply` patches one literal value in the active recorder state.

use std::fs;
use std::io::Write;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use crate::recorder_state::RecorderState;
use crate::scenario::{Step, Value};

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let summary = apply(&opts)?;
    println!(
        "heal-apply step={} to step {}",
        summary.step_id, summary.target_index
    );
    println!("  after: {}", summary.new_value);
    if opts.dry_run {
        println!("(dry-run; pass without --dry-run to write)");
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa heal-apply - patch one active direct step value

Usage:
  agent-qa heal-apply <sid> --step <stepId> [--target-step <stepIndex|stepId>] [--run <runId>] [--dry-run]

Patches a literal string value in recorder-state.json. It never drives the browser."
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
    mode: String,
    value: Option<String>,
}

#[derive(Debug)]
struct Summary {
    step_id: String,
    target_index: usize,
    new_value: String,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut sid = None;
    let mut step_id = None;
    let mut target_step = None;
    let mut run_id = None;
    let mut dry_run = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--step" => step_id = it.next().cloned(),
            value if value.starts_with("--step=") => {
                step_id = Some(value["--step=".len()..].to_string())
            }
            "--target-step" => target_step = it.next().cloned(),
            value if value.starts_with("--target-step=") => {
                target_step = Some(value["--target-step=".len()..].to_string())
            }
            "--run" => run_id = it.next().cloned(),
            value if value.starts_with("--run=") => {
                run_id = Some(value["--run=".len()..].to_string())
            }
            "--dry-run" => dry_run = true,
            value if value.starts_with("--") => bail!("unknown flag {value:?}"),
            value => {
                if sid.is_some() {
                    bail!(
                        "unexpected positional {value:?}; usage: heal-apply <sid> --step <stepId>"
                    );
                }
                sid = Some(value.to_string());
            }
        }
    }
    Ok(Opts {
        sid: sid.ok_or_else(|| anyhow!("usage: heal-apply <sid> --step <stepId> [flags]"))?,
        step_id: step_id.ok_or_else(|| anyhow!("--step is required"))?,
        target_step,
        run_id,
        dry_run,
    })
}

fn apply(opts: &Opts) -> Result<Summary> {
    let scenario_dir = crate::paths::scenario_dir(&opts.sid)?;
    let run_id = match &opts.run_id {
        Some(run_id) => run_id.clone(),
        None => fs::read_to_string(scenario_dir.join("replays/latest.txt"))?
            .trim()
            .to_string(),
    };
    if !is_safe_segment(&run_id) || !is_safe_segment(&opts.step_id) {
        bail!("unsafe run or step id");
    }
    let response_file = scenario_dir
        .join("replays")
        .join(&run_id)
        .join("heal-responses")
        .join(format!("{}.json", opts.step_id));
    let response: HealResponse = serde_json::from_slice(&fs::read(&response_file)?)
        .with_context(|| format!("parse {}", response_file.display()))?;
    if response.mode != "value-correction" {
        bail!("heal-apply only supports value-correction responses");
    }
    let new_value = response
        .value
        .ok_or_else(|| anyhow!("heal-response has no `value` to apply"))?;
    let mut state = RecorderState::load_active()?;
    if state.sid != opts.sid {
        bail!("active recording is {:?}, not {:?}", state.sid, opts.sid);
    }
    let target_index = resolve_target(
        &state.steps,
        opts.target_step.as_deref().unwrap_or(&opts.step_id),
    )?;
    patch_step_value(&mut state.steps[target_index], &new_value)?;
    if !opts.dry_run {
        state.save()?;
        fs::rename(
            &response_file,
            response_file.with_file_name(format!("{}.applied.json", opts.step_id)),
        )?;
        let audit_path = scenario_dir.join("recording/heal.jsonl");
        fs::create_dir_all(audit_path.parent().unwrap())?;
        let mut audit = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&audit_path)?;
        writeln!(
            audit,
            "{}",
            serde_json::json!({ "schema": "heal-row/v1", "stepId": opts.step_id, "targetIndex": target_index, "mode": "caller-driven-resolved", "newValue": new_value })
        )?;
    }
    Ok(Summary {
        step_id: opts.step_id.clone(),
        target_index,
        new_value,
    })
}

fn patch_step_value(step: &mut Step, new_value: &str) -> Result<()> {
    let Step::Do {
        value: Some(Value::Literal { literal }),
        ..
    } = step
    else {
        bail!("recorded step has no correctable literal value");
    };
    if !literal.is_string() {
        bail!("recorded step value is not a string");
    }
    *literal = serde_json::Value::String(new_value.to_string());
    Ok(())
}

fn resolve_target(steps: &[Step], hint: &str) -> Result<usize> {
    if let Ok(index) = hint.parse::<usize>() {
        if index < steps.len() {
            return Ok(index);
        }
        bail!("target step index {index} out of range");
    }
    steps
        .iter()
        .position(|step| step.id() == hint)
        .ok_or_else(|| anyhow!("no active step matches {hint:?}"))
}

fn is_safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    #[test]
    fn patch_step_value_changes_only_direct_literal_values() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path());
        let mut state = RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"type","kind":"do","verb":"type","on":{"role":"textbox","name":"Email"},"value":{"from":"literal","literal":"old@example.com"}}
        ])).unwrap();
        patch_step_value(&mut state.steps[0], "new@example.com").unwrap();
        assert_eq!(
            serde_json::to_value(&state.steps[0]).unwrap()["value"]["literal"],
            "new@example.com"
        );
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }
}
