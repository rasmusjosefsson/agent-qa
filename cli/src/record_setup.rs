//! `record-setup` appends one schema-valid generic setup operation.

use anyhow::{bail, Context, Result};
use serde_json::Value as Json;

use crate::recorder_state::RecorderState;
use crate::scenario::EnvOp;
use crate::schema;

pub fn run(args: &[String]) -> Result<u8> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "help"))
    {
        print_help();
        return Ok(0);
    }
    if args.len() != 1 {
        bail!("usage: record-setup <env-op-json>");
    }
    let raw: Json = serde_json::from_str(&args[0]).context("parse setup JSON")?;
    let op = parse_env_op(&raw)?;
    let mut state = RecorderState::load_active()?;
    state.env_open.push(op);
    state.save()?;
    println!("recorded setup operation");
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa record-setup — append one replay setup operation

Usage:
  agent-qa record-setup '<env-op-json>'

The JSON must be one schema-valid scenario/2 env.open operation. Supported
kinds are fresh, useProfile, nav, cookie, localStorage, gql, and flag.

Examples:
  agent-qa record-setup '{{\"kind\":\"flag\",\"name\":\"example-flag\",\"enabled\":true}}'
  agent-qa record-setup '{{\"kind\":\"localStorage\",\"key\":\"view\",\"value\":\"compact\"}}'"
    );
}

fn parse_env_op(raw: &Json) -> Result<EnvOp> {
    schema::validate_value(&serde_json::json!({
        "schema": "scenario/2",
        "id": "recording-setup",
        "intent": "validate recording setup",
        "env": { "open": [raw] },
        "steps": [],
    }))
    .context("setup operation failed scenario schema validation")?;
    serde_json::from_value(raw.clone()).context("parse setup operation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::paths;
    use crate::recorder_state::{RecorderBaseline, RecorderState};
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    #[test]
    fn record_setup_appends_a_generic_env_op() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path());
        RecorderState::new(
            "s1".into(),
            "record".into(),
            "session".into(),
            RecorderBaseline::KeepSession,
            None,
            BrowserConnection::default(),
        )
        .save()
        .unwrap();

        run(&[r#"{"kind":"flag","name":"example-flag","enabled":true}"#.into()]).unwrap();

        let state = RecorderState::load_active().unwrap();
        assert!(
            matches!(state.env_open.as_slice(), [EnvOp::Flag { name, enabled: true, .. }] if name == "example-flag")
        );
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn record_setup_rejects_an_invalid_env_op_at_the_cli_boundary() {
        let raw: Json = serde_json::json!({ "kind": "flag", "name": "x" });
        let err = parse_env_op(&raw).unwrap_err().to_string();
        assert!(err.contains("schema"));
    }
}
