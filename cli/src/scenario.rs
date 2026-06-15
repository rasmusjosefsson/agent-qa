//! `scenario/2` contract types — Rust port of `cli/src/scenario.ts`.
//!
//! These structs round-trip through `serde_json` byte-for-byte against any
//! valid `scenario.json`. They are the *typed* view; the *authoritative*
//! shape lives in `schema/scenario-schema.json`, validated at load time
//! (see [`crate::schema`]).
//!
//! Conventions (mirrors the TS source):
//!   - tagged unions on `kind` for [`Step`], [`EnvOp`]
//!   - tagged union on `from` for [`Value`]
//!   - untagged union for [`Locator`] / [`ClaimSubject`] (TS used
//!     "shape-distinguished" unions there)
//!   - every field is optional unless the schema marks it required
//!   - extra fields are rejected at the schema layer; serde ignores them
//!     here so we don't double-reject
//!
//! Types are present but not yet consumed by a runner. Allowing
//! dead_code at module scope keeps the warning surface clean while later
//! slices wire each variant into replay / record / heal.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

pub const SCENARIO_SCHEMA_ID: &str = "scenario/2";

// ---------- supporting unions ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Producer {
    AutomatedCapture,
    LlmAuthor,
    AgentRecorder,
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    Abort,
    Continue,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MintScope {
    Scenario,
    Loop,
    Template,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NameMatchMode {
    Exact,
    Contains,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RawLocatorKind {
    Css,
    Xpath,
    TestId,
    /// Visible text content. Maps to `agent-browser find text <v> <act>`
    /// at replay time. Use when role+name lookup is unreliable (e.g.
    /// Radix portals, virtualised lists) but a unique visible label
    /// exists. The recorder emits this kind for `clickByText`.
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Predicate {
    IsVisible,
    IsHidden,
    IsEnabled,
    IsDisabled,
    IsChecked,
    IsUnchecked,
    Exists,
    NotExists,
    Equals,
    Contains,
    Matches,
    StartsWith,
    EndsWith,
    Gt,
    Gte,
    Lt,
    Lte,
    CountEquals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verb {
    Goto,
    Reload,
    Back,
    Forward,
    Click,
    Type,
    Clear,
    Press,
    Hover,
    Select,
    Check,
    Uncheck,
    Upload,
    ScrollTo,
    Focus,
    Blur,
    Read,
    CallGql,
    Wait,
    #[serde(rename = "loop")]
    Loop,
    Group,
    UseTemplate,
}

// ---------- Locator ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NameMatch {
    Plain(String),
    Pattern {
        pattern: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r#match: Option<NameMatchMode>,
    },
    I18n {
        #[serde(rename = "i18nKey")]
        i18n_key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocatorTolerance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_suffix: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_augmented_label: Option<bool>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocatorRole {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<NameMatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<Locator>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerate: Option<LocatorTolerance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocatorRawSpec {
    pub kind: RawLocatorKind,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocatorRaw {
    pub raw: LocatorRawSpec,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Locator {
    Role(LocatorRole),
    Raw(LocatorRaw),
}

// ---------- Value ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintSpec {
    pub template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MintScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "lowercase")]
pub enum Value {
    Literal {
        literal: Json,
    },
    Input {
        input: String,
    },
    #[serde(rename_all = "camelCase")]
    Step {
        step_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Mint {
        mint: MintSpec,
    },
    #[serde(rename_all = "camelCase")]
    Loop {
        loop_var: String,
    },
}

// ---------- Claim ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_matches: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<HttpMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElementClaimKind {
    Text,
    Value,
    Count,
    Attribute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkClaimKind {
    Status,
    ResponseJsonPath,
    Fired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaimSubject {
    #[serde(rename_all = "camelCase")]
    Element {
        element: Locator,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attribute: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of_kind: Option<ElementClaimKind>,
    },
    Url {
        url: bool,
    },
    #[serde(rename_all = "camelCase")]
    Network {
        network: NetworkMatcher,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of_kind: Option<NetworkClaimKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Data {
        data: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    Flag {
        flag: String,
    },
    Var {
        kind: String, // always "var" — kept literal to disambiguate untagged
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub subject: ClaimSubject,
    pub predicate: Predicate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<BTreeMap<String, Json>>,
}

// ---------- Step ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opens_from_step_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_as: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_for: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_for: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect_fail_for: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabMatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Step {
    #[serde(rename_all = "camelCase")]
    Do {
        id: String,
        intent: String,
        verb: Verb,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on: Option<Locator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save_as: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<BTreeMap<String, Json>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<StepContext>,
    },
    #[serde(rename_all = "camelCase")]
    Check {
        id: String,
        intent: String,
        claim: Claim,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context: Option<StepContext>,
    },
}

impl Step {
    pub fn id(&self) -> &str {
        match self {
            Step::Do { id, .. } | Step::Check { id, .. } => id,
        }
    }
    pub fn intent(&self) -> &str {
        match self {
            Step::Do { intent, .. } | Step::Check { intent, .. } => intent,
        }
    }
}

// ---------- Env ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvOpPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_run: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_abort: Option<OnAbort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailureContinue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnAbort {
    Run,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailureContinue {
    Abort,
    Continue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EnvOp {
    /// Reset browser state for the active session: clear cookies +
    /// localStorage + sessionStorage. Default `env.open[0]` for
    /// anonymous recordings — makes replay reproduce the same blank
    /// starting slate the recording ran against.
    Fresh {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    /// Re-bootstrap a named profile. Used as `env.open[0]` for
    /// recordings made under `--profile <name>` — replay re-runs the
    /// same bootstrap so the session reaches the same authenticated
    /// baseline regardless of cookies left over from earlier runs.
    UseProfile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Nav {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Cookie {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        domain: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    LocalStorage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Gql {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        url: String,
        query: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variables: Option<BTreeMap<String, Json>>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "forEach")]
        for_each: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "saveAs")]
        save_as: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
    Flag {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        intent: Option<String>,
        name: String,
        enabled: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<EnvOpPolicy>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Env {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<Vec<EnvOp>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<Vec<EnvOp>>,
}

// ---------- Inputs / Provenance / Template ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDecl {
    #[serde(rename = "type")]
    pub ty: InputType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<BTreeMap<String, Json>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, Json>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub producer: Producer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produced_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<BTreeMap<String, InputDecl>>,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<BTreeMap<String, Template>>,
}

// ---------- Scenario root ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub schema: String, // must equal "scenario/2"; enforced by JSON Schema
    pub id: String,
    pub intent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<BTreeMap<String, InputDecl>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Env>,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<BTreeMap<String, Template>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produced_by: Option<Provenance>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn minimal_scenario_roundtrips() {
        let j = json!({
            "schema": "scenario/2",
            "id": "j1",
            "intent": "open the homepage",
            "steps": [
                {
                    "id": "s1",
                    "intent": "go to /",
                    "kind": "do",
                    "verb": "goto"
                },
                {
                    "id": "s2",
                    "intent": "url is set",
                    "kind": "check",
                    "claim": {
                        "subject": { "url": true },
                        "predicate": "exists"
                    }
                }
            ]
        });
        let parsed: Scenario = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(parsed.id, "j1");
        assert_eq!(parsed.steps.len(), 2);
        assert!(matches!(parsed.steps[0], Step::Do { .. }));
        assert!(matches!(parsed.steps[1], Step::Check { .. }));

        let back = serde_json::to_value(&parsed).unwrap();
        // Round-trip preserves the meaningful subset (we strip None fields).
        assert_eq!(back["id"], j["id"]);
        assert_eq!(back["steps"][0]["verb"], "goto");
    }

    #[test]
    fn value_tagged_union_roundtrips() {
        let cases = [
            json!({ "from": "literal", "literal": "hi" }),
            json!({ "from": "input", "input": "email" }),
            json!({ "from": "step", "stepId": "s1", "path": "data.id" }),
            json!({ "from": "mint", "mint": { "template": "qa-{{vars._unique}}", "scope": "scenario" } }),
            json!({ "from": "loop", "loopVar": "row" }),
        ];
        for c in cases {
            let v: Value = serde_json::from_value(c.clone()).unwrap();
            let back = serde_json::to_value(&v).unwrap();
            assert_eq!(back, c, "value did not round-trip cleanly: {c}");
        }
    }

    #[test]
    fn locator_role_and_raw_distinguish() {
        let role: Locator = serde_json::from_value(json!({
            "role": "button",
            "name": "Save"
        }))
        .unwrap();
        assert!(matches!(role, Locator::Role(_)));

        let raw: Locator = serde_json::from_value(json!({
            "raw": { "kind": "css", "value": "button.save" },
            "reason": "design system uses non-accessible buttons"
        }))
        .unwrap();
        assert!(matches!(raw, Locator::Raw(_)));
    }

    #[test]
    fn env_op_kind_roundtrips() {
        let op: EnvOp = serde_json::from_value(json!({
            "kind": "nav",
            "url": "https://app.example.com/"
        }))
        .unwrap();
        match &op {
            EnvOp::Nav { url, .. } => assert_eq!(url.as_deref(), Some("https://app.example.com/")),
            _ => panic!("expected Nav variant"),
        }
    }
}
