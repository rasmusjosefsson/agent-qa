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
    if let Some(warn) = opaque_origin_warning(&state.env_open, &op) {
        eprintln!("{warn}");
    }
    state.env_open.push(op);
    state.save()?;
    println!("recorded setup operation");
    Ok(0)
}

/// cookie/localStorage ops run against the page's origin; on a session
/// that never navigated (`about:blank`) they die at replay with
/// `SecurityError: storage denied`. Warn unless a nav op precedes them.
fn opaque_origin_warning(existing: &[EnvOp], new_op: &EnvOp) -> Option<String> {
    let origin_bound = |op: &EnvOp| matches!(op, EnvOp::Cookie { .. } | EnvOp::LocalStorage { .. });
    let is_nav = |op: &EnvOp| matches!(op, EnvOp::Nav { .. });
    if origin_bound(new_op) && !existing.iter().any(is_nav) {
        return Some(format!(
            "warning: {} runs against the page origin, but env.open has no nav op — \
             replay lands on about:blank and fails with a storage SecurityError. \
             Add a nav op first (or record `start --open <url>`).",
            op_kind(new_op)
        ));
    }
    if is_nav(new_op) && existing.iter().any(origin_bound) {
        return Some(
            "warning: nav lands after origin-bound ops that already ran on about:blank — \
             move it before the cookie/localStorage ops or they still fail at replay."
                .to_string(),
        );
    }
    None
}

fn op_kind(op: &EnvOp) -> &'static str {
    match op {
        EnvOp::Cookie { .. } => "cookie",
        EnvOp::LocalStorage { .. } => "localStorage",
        _ => "this op",
    }
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

    fn nav() -> EnvOp {
        EnvOp::Nav {
            intent: None,
            url: Some("https://app.example.com/".into()),
            policy: None,
        }
    }

    fn storage() -> EnvOp {
        EnvOp::LocalStorage {
            intent: None,
            key: Some("k".into()),
            value: Some("v".into()),
            policy: None,
        }
    }

    #[test]
    fn storage_op_without_a_nav_warns() {
        let warn = opaque_origin_warning(
            &[EnvOp::Fresh {
                intent: None,
                policy: None,
            }],
            &storage(),
        );
        assert!(warn.unwrap().contains("no nav op"));
    }

    #[test]
    fn storage_op_after_a_nav_is_silent() {
        assert!(opaque_origin_warning(&[nav()], &storage()).is_none());
    }

    #[test]
    fn nav_after_storage_warns_about_order() {
        let warn = opaque_origin_warning(&[storage()], &nav());
        assert!(warn.unwrap().contains("after origin-bound"));
    }

    #[test]
    fn non_origin_bound_ops_never_warn() {
        let flag = EnvOp::Flag {
            intent: None,
            name: "f".into(),
            enabled: true,
            policy: None,
        };
        assert!(opaque_origin_warning(&[], &flag).is_none());
        assert!(opaque_origin_warning(&[], &nav()).is_none());
    }
}
