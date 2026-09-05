//! `buffer` inspects and edits typed steps in the active recorder state.

use anyhow::{anyhow, bail, Result};
use serde_json::json;

use crate::recorder_state::RecorderState;
use crate::scenario::Step;

pub fn run(args: &[String]) -> Result<u8> {
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return Ok(0);
    }
    match args[0].as_str() {
        "list" => cmd_list(&args[1..]),
        "delete" | "rm" => cmd_delete(&args[1..]),
        "move" | "mv" => cmd_move(&args[1..]),
        "clear" => cmd_clear(),
        "discard" => cmd_discard(),
        other => bail!("buffer: unknown subcommand {other:?}; try list|delete|move|clear|discard"),
    }
}

fn print_help() {
    println!(
        "agent-qa buffer - inspect or edit the active recording

Usage:
  agent-qa buffer list [--json]
  agent-qa buffer delete <index>
  agent-qa buffer move <from> <to>
  agent-qa buffer clear
  agent-qa buffer discard

Delete and move reassign dense s0, s1, ... ids. Discard removes the active recording."
    );
}

fn parse_index(value: &str, label: &str) -> Result<usize> {
    value
        .parse()
        .map_err(|_| anyhow!("{label} must be a non-negative integer; got {value:?}"))
}

fn normalize_ids(steps: &mut [Step]) {
    for (index, step) in steps.iter_mut().enumerate() {
        step.set_id(format!("s{index}"));
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
}
