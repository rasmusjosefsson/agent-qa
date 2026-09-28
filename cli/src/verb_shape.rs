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
        Verb::Type | Verb::Select | Verb::Upload | Verb::Download => VerbRule {
            required: &[DoField::On, DoField::Value],
            ..VerbRule::default()
        },
        Verb::DblClick => VerbRule {
            required: &[DoField::On],
            forbidden: &[DoField::Value],
            ..VerbRule::default()
        },
        Verb::Tab => VerbRule {
            required: &[DoField::Value],
            forbidden: &[DoField::On],
            value_kinds: &["literal"],
            ..VerbRule::default()
        },
        Verb::Viewport => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On, DoField::Value],
            params_required: &["width", "height"],
            ..VerbRule::default()
        },
        Verb::FileChooser => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On, DoField::Value],
            params_required: &["files"],
            ..VerbRule::default()
        },
        Verb::Mock => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On, DoField::Value],
            params_required: &["url"],
            ..VerbRule::default()
        },
        Verb::Unmock => VerbRule {
            forbidden: &[DoField::On, DoField::Value],
            ..VerbRule::default()
        },
        // `params` carries the state spec; which keys are valid is checked
        // in dispatch (localStorage / sessionStorage / cookies / clears).
        Verb::State => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On, DoField::Value],
            ..VerbRule::default()
        },
        // `on` is the drag source; `params.to` is the drop target locator.
        Verb::Drag => VerbRule {
            required: &[DoField::On, DoField::Params],
            forbidden: &[DoField::Value],
            params_required: &["to"],
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
        // `dialog accept|dismiss`; `params.text` optionally carries prompt input.
        Verb::Dialog => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::On, DoField::Value],
            params_required: &["action"],
            ..VerbRule::default()
        },
        // `on` is the element to hold; `params.ms` optional (default 500).
        Verb::Hold => VerbRule {
            required: &[DoField::On],
            forbidden: &[DoField::Value],
            ..VerbRule::default()
        },
        // `on` optional (default: viewport center); `params.direction` required.
        Verb::Swipe => VerbRule {
            required: &[DoField::Params],
            forbidden: &[DoField::Value],
            params_required: &["direction"],
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

    #[test]
    fn dialog_requires_params_action_and_forbids_on_value() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "dialog" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'params'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "dialog",
            "params": {}
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("params requires 'action'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "dialog",
            "params": { "action": "accept" },
            "on": { "role": "button" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'on'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "dialog",
            "params": { "action": "accept" },
            "value": { "from": "literal", "literal": "x" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'value'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "dialog",
            "params": { "action": "accept", "text": "John Doe" }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn state_requires_params_and_rejects_on_value() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "state" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'params'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "state",
            "params": { "localStorage": { "token": "abc" } },
            "value": { "from": "literal", "literal": "x" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'value'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "state",
            "params": {
                "localStorage": { "token": "abc" },
                "sessionStorage": { "cart": "{}" },
                "cookies": [{ "name": "session", "value": "v", "path": "/" }],
                "clearCookies": true
            }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn viewport_requires_width_and_height_params() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "viewport" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'params'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "viewport",
            "params": { "width": 375 }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("params requires 'height'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "viewport",
            "params": { "width": 375, "height": 812 },
            "on": { "role": "button" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'on'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "viewport",
            "params": { "width": 375, "height": 812 }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn file_chooser_requires_files_param() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "fileChooser" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'params'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "fileChooser",
            "params": { "mode": "set" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("params requires 'files'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "fileChooser",
            "params": { "files": ["a.png"] },
            "on": { "role": "button" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'on'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "fileChooser",
            "params": { "files": ["a.png"] }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn download_requires_on_and_value() {
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "download",
            "value": { "from": "literal", "literal": "out/report.pdf" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'on'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "download",
            "on": { "raw": { "kind": "css", "value": "#dl" }, "reason": "test" },
            "value": { "from": "literal", "literal": "out/report.pdf" }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn drag_requires_on_and_to_locator() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "drag" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'on'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "drag",
            "on": { "role": "listitem", "name": "Card A" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'params'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "drag",
            "on": { "role": "listitem", "name": "Card A" },
            "params": { "into": "Done" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("params requires 'to'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "drag",
            "on": { "role": "listitem", "name": "Card A" },
            "params": { "to": { "role": "list", "name": "Done" } },
            "value": { "from": "literal", "literal": "x" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'value'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "drag",
            "on": { "role": "listitem", "name": "Card A" },
            "params": { "to": { "role": "list", "name": "Done" } }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn hold_requires_on_forbids_value() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "hold" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'on'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "hold",
            "on": { "raw": { "kind": "css", "value": "#btn" }, "reason": "r" },
            "params": { "ms": 800 }
        }));
        assert_verb_shape(&s).unwrap();
    }

    #[test]
    fn swipe_requires_direction_forbids_value_allows_no_on() {
        let s = parse(json!({ "id": "s1", "intent": "x", "kind": "do", "verb": "swipe" }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("requires 'params'"));

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "swipe",
            "params": { "distance": 200 }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("params requires 'direction'"));

        // no `on` — viewport swipe
        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "swipe",
            "params": { "direction": "up" }
        }));
        assert_verb_shape(&s).unwrap();

        let s = parse(json!({
            "id": "s1", "intent": "x", "kind": "do", "verb": "swipe",
            "on": { "raw": { "kind": "css", "value": ".card" }, "reason": "r" },
            "params": { "direction": "left", "distance": 150 },
            "value": { "from": "literal", "literal": "x" }
        }));
        assert!(assert_verb_shape(&s)
            .unwrap_err()
            .to_string()
            .contains("must not carry 'value'"));
    }
}
