//! `scenario/2` schema validator.
//!
//! `schema/scenario-schema.json` is the authoritative shape; this module is
//! the runtime gate. Pattern is the same as `cli/src/validate-scenario.ts`:
//! load the schema once at process start, return errors verbatim from the
//! validator. No friendly formatter yet — the raw output is returned as-is;
//! a smart formatter can land later if a downstream consumer needs one.

use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use jsonschema::JSONSchema;
use serde_json::Value as Json;

const SCHEMA_TEXT: &str = include_str!("../../schema/scenario-schema.json");

fn compiled() -> &'static JSONSchema {
    static CELL: OnceLock<JSONSchema> = OnceLock::new();
    CELL.get_or_init(|| {
        let parsed: Json =
            serde_json::from_str(SCHEMA_TEXT).expect("embedded scenario-schema.json is valid JSON");
        JSONSchema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .compile(&parsed)
            .expect("embedded scenario-schema.json compiles as a 2020-12 schema")
    })
}

/// Validate a JSON value against the embedded `scenario/2` schema.
///
/// On success returns `Ok(())`. On failure returns a single
/// `anyhow::Error` whose message is one error per line — sufficient for
/// CLI output without parsing structured error objects.
pub fn validate_value(value: &Json) -> Result<()> {
    let schema = compiled();
    let result = schema.validate(value);
    match result {
        Ok(()) => Ok(()),
        Err(errors) => {
            let lines: Vec<String> = errors
                .map(|e| format!("  at /{}: {}", e.instance_path, e))
                .collect();
            Err(anyhow!(
                "{} schema error(s):\n{}",
                lines.len(),
                lines.join("\n")
            ))
        }
    }
}

/// Convenience: parse `bytes` as JSON, then validate.
pub fn validate_bytes(bytes: &[u8]) -> Result<Json> {
    let value: Json = serde_json::from_slice(bytes).context("parse scenario JSON")?;
    validate_value(&value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_compiles() {
        // Forces lazy init.
        let _ = compiled();
    }

    #[test]
    fn minimal_valid_scenario_passes() {
        let j = json!({
            "schema": "scenario/2",
            "id": "j1",
            "intent": "smoke",
            "steps": [
                { "id": "s1", "intent": "go", "kind": "do", "verb": "goto" }
            ]
        });
        validate_value(&j).expect("minimal scenario should validate");
    }

    #[test]
    fn missing_required_fields_fail() {
        let j = json!({ "schema": "scenario/2", "id": "j1" });
        let err = validate_value(&j).unwrap_err().to_string();
        assert!(
            err.contains("intent") || err.contains("steps"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn wrong_schema_id_fails() {
        let j = json!({
            "schema": "scenario/1",
            "id": "j1",
            "intent": "smoke",
            "steps": []
        });
        validate_value(&j).unwrap_err();
    }

    #[test]
    fn unknown_step_kind_fails() {
        let j = json!({
            "schema": "scenario/2",
            "id": "j1",
            "intent": "smoke",
            "steps": [
                { "id": "s1", "intent": "x", "kind": "wat" }
            ]
        });
        validate_value(&j).unwrap_err();
    }

    /// The smoke scenario under examples/scenarios/smoke/scenario.json is a
    /// hand-authored document — schema-validate it from inside cargo
    /// test so a future schema tightening catches the doc drift before
    /// the CI smoke-install step does.
    #[test]
    fn smoke_example_scenario_validates() {
        const SMOKE: &str = include_str!("../../examples/scenarios/smoke/scenario.json");
        let value: serde_json::Value =
            serde_json::from_str(SMOKE).expect("smoke scenario is valid JSON");
        validate_value(&value).expect("smoke scenario is schema-valid");
    }
}
