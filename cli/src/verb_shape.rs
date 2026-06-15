//! Per-verb shape rules for scenario/2 `do` steps.
//!
//! The schema's `VerbShapeRules` slot is a no-op `{ "type": "object" }`;
//! the real per-verb shape checks live here as runtime validation:
//!
//!   1. Friendlier error surface — one named failure per verb-shape
//!      violation instead of a long Ajv `anyOf` cascade.
//!   2. Adding a verb is one entry here + one dispatch arm — no
//!      JSON Schema `allOf` graph to edit.
//!
//! Each rule names required / forbidden fields plus an optional
//! `value_kinds` allowlist (which `Value.from` discriminator the verb
//! accepts when it accepts a Value at all). `goto`, for example, only
//! accepts a `literal` URL.

use anyhow::{bail, Result};

use crate::scenario::{Step, Value, Verb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DoField {
    On,
    Value,
    Params,
    SaveAs,
}

#[derive(Debug, Clone, Default)]
pub struct VerbRule {
    pub required: &'static [DoField],
    pub forbidden: &'static [DoField],
    pub value_kinds: &'static [&'static str],
    pub params_required: &'static [&'static str],
}

fn rule_for(verb: &Verb) -> VerbRule {
    match verb {
        Verb::Goto => VerbRule {
            required: &[DoField::Value],
            forbidden: &[DoField::On],
            value_kinds: &["literal"],
            params_required: &[],
        },
        Verb::Reload | Verb::Back | Verb::Forward => VerbRule {
            forbidden: &[DoField::On, DoField::Value],
            ..VerbRule::default()
        },
        Verb::Click
        | Verb::Clear
        | Verb::Hover
        | Verb::Check
        | Verb::Uncheck
        | Verb::Focus
        | Verb::Blur => VerbRule {
            required: &[DoField::On],
            forbidden: &[DoField::Value],
            ..VerbRule::default()
        },
        Verb::Type | Verb::Select | Verb::Upload => VerbRule {
            required: &[DoField::On, DoField::Value],
            ..VerbRule::default()
        },
        Verb::Press => VerbRule {
            required: &[DoField::Value],
            value_kinds: &["literal"],
            ..VerbRule::default()
        },
        Verb::ScrollTo => VerbRule {
            forbidden: &[],
            ..VerbRule::default()
        },
        Verb::Read => VerbRule {
            required: &[DoField::On],
            ..VerbRule::default()
        },
        Verb::CallGql => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On],
            params_required: &["url", "query"],
            ..VerbRule::default()
        },
        Verb::Wait => VerbRule {
            forbidden: &[DoField::On],
            ..VerbRule::default()
        },
        Verb::Loop => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On, DoField::Value],
            params_required: &["over", "as", "do"],
            ..VerbRule::default()
        },
        Verb::Group => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On, DoField::Value],
            params_required: &["steps"],
            ..VerbRule::default()
        },
        Verb::UseTemplate => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On],
            params_required: &["template"],
            ..VerbRule::default()
        },
    }
}

/// Validate a `do` step against its verb's shape. Returns an error on
/// the first violation; the runner surfaces the message as the step's
/// failure reason.
pub fn assert_verb_shape(step: &Step) -> Result<()> {
    let (id, verb, on_present, value, save_as_present, params_present, params_keys) = match step {
        Step::Do {
            id,
            verb,
            on,
            value,
            save_as,
            params,
            ..
        } => (
            id.as_str(),
            verb,
            on.is_some(),
            value.as_ref(),
            save_as.is_some(),
            params.is_some(),
            params
                .as_ref()
                .map(|m| m.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
        ),
        Step::Check { id, .. } => bail!("assert_verb_shape: step '{id}' is a check, not a do"),
    };
    let rule = rule_for(verb);
    let verb_dbg = format!("{verb:?}").to_ascii_lowercase();

    for field in rule.required {
        let present = match field {
            DoField::On => on_present,
            DoField::Value => value.is_some(),
            DoField::Params => params_present,
            DoField::SaveAs => save_as_present,
        };
        if !present {
            bail!(
                "step '{id}' (verb={verb_dbg}) requires '{}'",
                field_name(*field)
            );
        }
    }
    for field in rule.forbidden {
        let present = match field {
            DoField::On => on_present,
            DoField::Value => value.is_some(),
            DoField::Params => params_present,
            DoField::SaveAs => save_as_present,
        };
        if present {
            bail!(
                "step '{id}' (verb={verb_dbg}) must not carry '{}'",
                field_name(*field)
            );
        }
    }
    if !rule.value_kinds.is_empty() {
        if let Some(v) = value {
            let got = value_kind(v);
            if !rule.value_kinds.contains(&got) {
                bail!(
                    "step '{id}' (verb={verb_dbg}) requires value.from ∈ [{}], got '{got}'",
                    rule.value_kinds.join("|")
                );
            }
        }
    }
    if !rule.params_required.is_empty() {
        for key in rule.params_required {
            if !params_keys.iter().any(|k| k == key) {
                bail!("step '{id}' (verb={verb_dbg}) params requires '{key}'");
            }
        }
    }
    Ok(())
}

fn field_name(f: DoField) -> &'static str {
    match f {
        DoField::On => "on",
        DoField::Value => "value",
        DoField::Params => "params",
        DoField::SaveAs => "saveAs",
    }
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Literal { .. } => "literal",
        Value::Input { .. } => "input",
        Value::Step { .. } => "step",
        Value::Mint { .. } => "mint",
        Value::Loop { .. } => "loop",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(j: serde_json::Value) -> Step {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn goto_requires_literal_value() {
        let s = parse(json!({ "id": "s1", "intent": "go", "kind": "do", "verb": "goto" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'value'"));

        let s = parse(json!({
            "id": "s1", "intent": "go", "kind": "do", "verb": "goto",
            "value": { "from": "input", "input": "url" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("value.from ∈ [literal]"));

        let s = parse(json!({
            "id": "s1", "intent": "go", "kind": "do", "verb": "goto",
            "value": { "from": "literal", "literal": "https://x" }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn goto_must_not_carry_on() {
        let s = parse(json!({
            "id": "s1", "intent": "go", "kind": "do", "verb": "goto",
            "value": { "from": "literal", "literal": "x" },
            "on": { "role": "link" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'on'"));
    }

    #[test]
    fn click_requires_on_forbids_value() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "click" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'on'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "click",
            "on": { "role": "button", "name": "Save" }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn type_requires_both_on_and_value() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "type",
            "on": { "role": "textbox", "name": "Email" },
            "value": { "from": "literal", "literal": "a@b" }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn press_requires_literal_value_only() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "press",
            "value": { "from": "literal", "literal": "Enter" }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn loop_requires_params_with_over_as_do() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "loop",
            "params": { "over": [], "as": "i" }
        }));
        let err = assert_verb_shape(&s).unwrap_err().to_string();
        assert!(err.contains("params requires 'do'"), "got: {err}");
    }

    #[test]
    fn call_gql_requires_params_url_and_query() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "callGql",
            "params": { "url": "https://api" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("params requires 'query'"));
    }
}
