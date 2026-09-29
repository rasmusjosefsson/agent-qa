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
        bail!("usage: record-setup <env-op-json>|-");
    }
    let raws: Vec<Json> = if args[0] == "-" {
        let stdin = std::io::read_to_string(std::io::stdin()).context("read stdin")?;
        crate::record_step::parse_stdin_drafts(&stdin).context("record-setup stdin")?
    } else {
        vec![serde_json::from_str(&args[0]).context("parse setup JSON")?]
    };
    let mut state = RecorderState::load_active()?;
    for (i, raw) in raws.iter().enumerate() {
        let op = parse_env_op(raw).with_context(|| format!("setup op {}", i + 1))?;
        state.env_open.push(op);
    }
    state.save()?;
    println!("recorded {} setup operation(s)", raws.len());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa record-setup — append one replay setup operation

Usage:
  agent-qa record-setup '<env-op-json>'    append one env op
  agent-qa record-setup -                  read env ops as JSONL from stdin
                                           (one op per line, blanks skipped,
                                           errors name their line number)

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
    .with_context(|| {
        format!(
            "setup operation failed scenario schema validation — {}",
            kind_hint(raw)
        )
    })?;
    serde_json::from_value(raw.clone()).context("parse setup operation")
}

/// The oneOf schema error can't name the failing arm; restate the op's own
/// kind plus its required fields so a malformed op is self-explanatory.
fn kind_hint(raw: &Json) -> String {
    let kind = raw
        .get("kind")
        .and_then(Json::as_str)
        .unwrap_or("<missing>");
    match kind {
        "fresh" => "fresh takes no other fields".to_string(),
        "useProfile" => "useProfile requires \"name\"".to_string(),
        "nav" => "nav requires \"url\" (a URI)".to_string(),
        "cookie" => "cookie requires \"name\" + \"value\"".to_string(),
        "localStorage" => "localStorage requires \"key\" + \"value\"".to_string(),
        "gql" => "gql requires \"url\" + \"query\"; \"forEach\" is a Value \
             ({\"from\":\"literal\",\"literal\":...} or from-step), \
             \"saveAs\" a bare name"
            .to_string(),
        "flag" => "flag requires \"name\" + \"enabled\"".to_string(),
        other => format!(
            "kind {other:?} is unknown — valid kinds: fresh, useProfile, nav, cookie, \
             localStorage, gql, flag"
        ),
    }
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
        // a second valid op lands in order behind the first
        run(&[r#"{"kind":"flag","name":"other-flag","enabled":false}"#.into()]).unwrap();

        let state = RecorderState::load_active().unwrap();
        assert!(matches!(
            state.env_open.as_slice(),
            [EnvOp::Flag { name: a, enabled: true, .. }, EnvOp::Flag { name: b, enabled: false, .. }]
                if a == "example-flag" && b == "other-flag"
        ));
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn record_setup_rejects_an_invalid_env_op_at_the_cli_boundary() {
        let raw: Json = serde_json::json!({ "kind": "flag", "name": "x" });
        let err = parse_env_op(&raw).unwrap_err().to_string();
        assert!(err.contains("schema"));
        assert!(err.contains("flag requires \"name\" + \"enabled\""));
    }

    #[test]
    fn kind_hint_names_unknown_kinds() {
        let raw: Json = serde_json::json!({ "kind": "bogus" });
        assert!(kind_hint(&raw).contains("valid kinds"));
    }
}
