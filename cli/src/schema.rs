//! `scenario/2` schema validator.
//!
//! `schema/scenario-schema.json` is the authoritative shape; this module is
//! the runtime gate. Pattern is the same as `cli/src/validate-scenario.ts`:
//! load the schema once at process start, return errors verbatim from the
//! validator. No friendly formatter yet — the raw output is returned as-is;
//! a smart formatter can land later if a downstream consumer needs one.

use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use jsonschema::Validator;
use serde_json::Value as Json;

const SCHEMA_TEXT: &str = include_str!("../../schema/scenario-schema.json");

fn compiled() -> &'static Validator {
    static CELL: OnceLock<Validator> = OnceLock::new();
    CELL.get_or_init(|| {
        let parsed: Json =
            serde_json::from_str(SCHEMA_TEXT).expect("embedded scenario-schema.json is valid JSON");
        jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&parsed)
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
                .map(|e| {
                    let path = e.instance_path.to_string();
                    // An env.open op failing its arm of the oneOf gets its
                    // required fields named — the bare oneOf message never does.
                    let hint = path
                        .strip_prefix("/env/open/")
                        .and_then(|i| i.parse::<usize>().ok())
                        .and_then(|i| value.pointer(&format!("/env/open/{i}")))
                        .map(|op| format!(" — {}", env_op_hint(op)))
                        .unwrap_or_default();
                    format!("  at /{path}: {e}{hint}")
                })
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

/// The oneOf schema error can't name the failing arm; restate the op's own
/// kind plus its required fields so a malformed env op is self-explanatory.
/// Used by `record-setup` and by validate error annotation for env.open ops.
pub fn env_op_hint(raw: &Json) -> String {
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

    /// `{"console": …}` claims shipped in the runner before the schema
    /// listed the subject — crawled drafts (console checks on by default)
    /// failed validation. Pin both matcher shapes.
    #[test]
    fn console_claim_subject_validates() {
        for console in [
            json!(true),
            json!({"type": "error"}),
            json!({"type": "error", "text": "boom"}),
        ] {
            let j = json!({
                "schema": "scenario/2", "id": "j1", "intent": "smoke",
                "steps": [{
                    "id": "s1", "intent": "quiet", "kind": "check",
                    "claim": {"subject": {"console": console}, "predicate": "notExists"}
                }]
            });
            validate_value(&j)
                .unwrap_or_else(|e| panic!("console={console:?} should validate: {e}"));
        }
    }

    #[test]
    fn clipboard_claim_subject_validates() {
        let j = json!({
            "schema": "scenario/2", "id": "j1", "intent": "smoke",
            "steps": [{
                "id": "s1", "intent": "copied", "kind": "check",
                "claim": {"subject": {"clipboard": true}, "predicate": "contains", "value": "x"}
            }]
        });
        validate_value(&j).expect("clipboard subject should validate");
    }

    /// `{"indexeddb": {"db","store","key"?}}` claims validate — record
    /// presence via exists/notExists, `path` walks a JSON value.
    #[test]
    fn indexeddb_claim_subject_validates() {
        for subject in [
            json!({"indexeddb": {"db": "cart", "store": "items"}}),
            json!({"indexeddb": {"db": "cart", "store": "items", "key": "sku-1"}}),
            json!({"indexeddb": {"db": "cart", "store": "items", "key": "sku-1"}, "path": "$.qty"}),
        ] {
            let j = json!({
                "schema": "scenario/2", "id": "j1", "intent": "smoke",
                "steps": [{
                    "id": "s1", "intent": "x", "kind": "check",
                    "claim": {"subject": subject, "predicate": "exists"}
                }]
            });
            validate_value(&j)
                .unwrap_or_else(|e| panic!("subject={subject:?} should validate: {e}"));
        }
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

    #[test]
    fn env_open_error_names_the_ops_required_fields() {
        let j = json!({
            "schema": "scenario/2",
            "id": "j1",
            "intent": "smoke",
            "env": { "open": [
                { "kind": "fresh" },
                { "kind": "useProfile", "profile": "admin-user" }
            ]},
            "steps": [
                { "id": "s1", "intent": "go", "kind": "do", "verb": "goto" }
            ]
        });
        let err = validate_value(&j).unwrap_err().to_string();
        assert!(
            err.contains("useProfile requires"),
            "expected field hint in error: {err}"
        );
    }
}
