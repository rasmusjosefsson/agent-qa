//! Recorder-side trigger shapes.
//!
//! `record-step` accepts a friendly trigger payload (`navigation` /
//! `action` / `wait` / `assert`) instead of the raw scenario/2 schema
//! shape. `flush` translates rows into scenario/2 `do` / `check` steps
//! via [`map_row`].
//!
//! Three closed allow-lists pin the surface area so typos fail at
//! record time with a copy-pasteable hint instead of three verbs later
//! inside `flush`:
//!
//!   - [`RECORDER_ACTION_METHODS`] — `method` field on `action` rows
//!   - [`RECORDER_ASSERT_KINDS`]   — `kind` field on `assert` rows
//!   - [`WAIT_CONDITION_KINDS`]    — `condition.kind` on `wait` rows
//!
//! Adding a new shape is: extend the allow-list + add a mapper arm in
//! [`map_row`]. No schema edit needed — scenario/2 already covers the
//! verb surface.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Map as JsonMap, Value as Json};

/// `kind` accepted by `record-step`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    Navigation,
    Action,
    Wait,
    Assert,
}

impl TriggerKind {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "navigation" => TriggerKind::Navigation,
            "action" => TriggerKind::Action,
            "wait" => TriggerKind::Wait,
            "assert" => TriggerKind::Assert,
            other => bail!(
                "record-step: kind must be one of [navigation, action, wait, assert]; got {other:?}"
            ),
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            TriggerKind::Navigation => "navigation",
            TriggerKind::Action => "action",
            TriggerKind::Wait => "wait",
            TriggerKind::Assert => "assert",
        }
    }
}

/// Allow-listed method names accepted on `action` rows. Anything outside
/// this set is refused at record time. Keep names matching the
/// agent-browser CLI shape so authors can copy-paste from the snapshot.
pub const RECORDER_ACTION_METHODS: &[&str] = &[
    "clickRole",
    "clickByText",
    "clickByLabel",
    "clickSelector",
    "fillByLabel",
    "fillBySelector",
    "pressKey",
    "submit",
    "selectByRole",
    "scrollIntoViewByText",
    "navigate",
];

/// Allow-listed assertion kinds. Kept tight on purpose — only the ones
/// the replay-side claim dispatcher actually implements. Widening this
/// list without wiring the matching predicate dispatch produces journeys
/// that fail on replay against themselves.
pub const RECORDER_ASSERT_KINDS: &[&str] = &["present", "absent", "url"];

/// Allow-listed wait condition kinds.
pub const WAIT_CONDITION_KINDS: &[&str] =
    &["duration", "selector", "selectorAbsent", "text", "url"];

/// Validate a trigger payload at record time. Cheap structural checks
/// only — the canonical translation lives in [`map_row`] and runs at
/// `flush`. The dual gate is deliberate: catch typos at the record
/// site (with the offending payload still in the caller's terminal
/// scrollback) AND fail loudly if a hand-edited steps.jsonl smuggles
/// in something silly.
pub fn validate_trigger(kind: TriggerKind, payload: &Json) -> Result<()> {
    let obj = payload
        .as_object()
        .ok_or_else(|| anyhow!("payload must be a JSON object"))?;
    match kind {
        TriggerKind::Navigation => {
            let route = obj
                .get("route")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("navigation payload requires 'route' (string)"))?;
            if route.is_empty() {
                bail!("navigation payload 'route' must be non-empty");
            }
        }
        TriggerKind::Action => {
            let method = obj
                .get("method")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("action payload requires 'method' (string)"))?;
            if !RECORDER_ACTION_METHODS.contains(&method) {
                bail!(
                    "action method {method:?} is not on the allow-list. Try one of: {}",
                    RECORDER_ACTION_METHODS.join(", ")
                );
            }
            if !obj.get("args").is_some_and(|v| v.is_array()) {
                bail!("action payload requires 'args' (array)");
            }
        }
        TriggerKind::Wait => {
            let cond = obj
                .get("condition")
                .and_then(|v| v.as_object())
                .ok_or_else(|| anyhow!("wait payload requires 'condition' (object)"))?;
            let ck = cond
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("wait condition requires 'kind'"))?;
            if !WAIT_CONDITION_KINDS.contains(&ck) {
                bail!(
                    "wait condition kind {ck:?} is not on the allow-list. Try one of: {}",
                    WAIT_CONDITION_KINDS.join(", ")
                );
            }
        }
        TriggerKind::Assert => {
            let k = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("assert payload requires 'kind' (string)"))?;
            if !RECORDER_ASSERT_KINDS.contains(&k) {
                bail!(
                    "assert kind {k:?} is not on the allow-list. Try one of: {}",
                    RECORDER_ASSERT_KINDS.join(", ")
                );
            }
            if !obj.get("args").is_some_and(|v| v.is_array()) {
                bail!("assert payload requires 'args' (array)");
            }
            if obj
                .get("intent")
                .and_then(|v| v.as_str())
                .map_or(true, str::is_empty)
            {
                bail!("assert payload requires a non-empty 'intent' (the contract description)");
            }
        }
    }
    Ok(())
}

// ---------- mapper ----------
//
// One `RecordedRow` (kind + payload) becomes one scenario/2 `Step`.
// Mirrors the friendly→schema translation skainet does in TypeScript,
// trimmed to the verb subset agent-qa core supports today.

/// Translate one recorded row (the JSONL line written by `record-step`)
/// into a scenario/2 step object. Used by `flush::build_steps`.
pub fn map_row(row_kind: &str, payload: &Json, step_id: &str) -> Result<Json> {
    let kind = TriggerKind::parse(row_kind)?;
    // Re-validate at flush time too — defends against hand-edited
    // steps.jsonl carrying bad rows.
    validate_trigger(kind, payload)?;
    match kind {
        TriggerKind::Navigation => map_navigation(payload, step_id),
        TriggerKind::Action => map_action(payload, step_id),
        TriggerKind::Wait => map_wait(payload, step_id),
        TriggerKind::Assert => map_assert(payload, step_id),
    }
}

fn map_navigation(p: &Json, step_id: &str) -> Result<Json> {
    let route = p["route"].as_str().expect("validated");
    Ok(make_do(
        step_id,
        "goto",
        p.get("intent")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("navigate to {route}")),
        DoBody {
            on: None,
            value: Some(literal_value(route)),
            params: None,
        },
    ))
}

fn map_action(p: &Json, step_id: &str) -> Result<Json> {
    let method = p["method"].as_str().expect("validated");
    let args = p["args"].as_array().expect("validated");
    let intent = p.get("intent").and_then(|v| v.as_str()).map(str::to_string);
    match method {
        "clickRole" => {
            let (role, name) = pair(args)?;
            Ok(make_do(
                step_id,
                "click",
                intent
                    .unwrap_or_else(|| format!("click {role} '{}'", name.as_deref().unwrap_or(""))),
                DoBody {
                    on: Some(role_locator(&role, name.as_deref())),
                    value: None,
                    params: None,
                },
            ))
        }
        "clickByText" => {
            let text = first_str(args)?;
            Ok(make_do(
                step_id,
                "click",
                intent.unwrap_or_else(|| format!("click text '{text}'")),
                DoBody {
                    on: Some(raw_locator(
                        "text",
                        &text,
                        "recorder captured clickByText (visible-text locator)",
                    )),
                    value: None,
                    params: None,
                },
            ))
        }
        "clickByLabel" => {
            let label = first_str(args)?;
            Ok(make_do(
                step_id,
                "click",
                intent.unwrap_or_else(|| format!("click element labelled '{label}'")),
                DoBody {
                    on: Some(role_locator("button", Some(&label))),
                    value: None,
                    params: None,
                },
            ))
        }
        "clickSelector" => {
            let sel = first_str(args)?;
            Ok(make_do(
                step_id,
                "click",
                intent.unwrap_or_else(|| format!("click css selector '{sel}'")),
                DoBody {
                    on: Some(raw_locator("css", &sel, "recorder captured a CSS selector")),
                    value: None,
                    params: None,
                },
            ))
        }
        "fillByLabel" => {
            let (label, value) = pair(args)?;
            let value_str = value.unwrap_or_default();
            Ok(make_do(
                step_id,
                "type",
                intent.unwrap_or_else(|| format!("fill '{label}' with '{value_str}'")),
                DoBody {
                    on: Some(role_locator("textbox", Some(&label))),
                    value: Some(literal_value(&value_str)),
                    params: None,
                },
            ))
        }
        "fillBySelector" => {
            let (sel, value) = pair(args)?;
            let value_str = value.unwrap_or_default();
            Ok(make_do(
                step_id,
                "type",
                intent.unwrap_or_else(|| format!("fill css '{sel}' with '{value_str}'")),
                DoBody {
                    on: Some(raw_locator("css", &sel, "recorder captured a CSS selector")),
                    value: Some(literal_value(&value_str)),
                    params: None,
                },
            ))
        }
        "pressKey" => {
            let key = first_str(args)?;
            Ok(make_do(
                step_id,
                "press",
                intent.unwrap_or_else(|| format!("press {key}")),
                DoBody {
                    on: None,
                    value: Some(literal_value(&key)),
                    params: None,
                },
            ))
        }
        "submit" => Ok(make_do(
            step_id,
            "press",
            intent.unwrap_or_else(|| "press Enter to submit".to_string()),
            DoBody {
                on: None,
                value: Some(literal_value("Enter")),
                params: None,
            },
        )),
        "selectByRole" => {
            let role = arg_str(args, 0)?;
            let name = arg_str(args, 1).ok();
            let option = arg_str(args, 2)?;
            Ok(make_do(
                step_id,
                "select",
                intent.unwrap_or_else(|| format!("select '{option}' from {role}")),
                DoBody {
                    on: Some(role_locator(&role, name.as_deref())),
                    value: Some(literal_value(&option)),
                    params: None,
                },
            ))
        }
        "scrollIntoViewByText" => {
            let text = first_str(args)?;
            Ok(make_do(
                step_id,
                "scrollTo",
                intent.unwrap_or_else(|| format!("scroll '{text}' into view")),
                DoBody {
                    on: Some(role_locator("generic", Some(&text))),
                    value: None,
                    params: None,
                },
            ))
        }
        "navigate" => {
            let route = first_str(args)?;
            Ok(make_do(
                step_id,
                "goto",
                intent.unwrap_or_else(|| format!("navigate to {route}")),
                DoBody {
                    on: None,
                    value: Some(literal_value(&route)),
                    params: None,
                },
            ))
        }
        other => bail!("unsupported action method {other:?}"),
    }
}

fn map_wait(p: &Json, step_id: &str) -> Result<Json> {
    let cond = p["condition"].as_object().expect("validated");
    let ck = cond["kind"].as_str().expect("validated");
    let intent = p.get("intent").and_then(|v| v.as_str()).map(str::to_string);
    match ck {
        "duration" => {
            let ms = cond
                .get("ms")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow!("wait condition duration requires integer 'ms'"))?;
            Ok(make_do(
                step_id,
                "wait",
                intent.unwrap_or_else(|| format!("wait {ms}ms")),
                DoBody {
                    on: None,
                    value: None,
                    params: Some(json!({ "ms": ms })),
                },
            ))
        }
        "selector" => {
            let sel = cond
                .get("selector")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("wait condition selector requires 'selector' string"))?;
            Ok(make_check(
                step_id,
                intent.unwrap_or_else(|| format!("wait for css selector '{sel}' to be visible")),
                json!({
                    "subject": { "element": raw_locator("css", sel, "recorder waited on selector") },
                    "predicate": "isVisible",
                }),
            ))
        }
        "selectorAbsent" => {
            let sel = cond
                .get("selector")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    anyhow!("wait condition selectorAbsent requires 'selector' string")
                })?;
            Ok(make_check(
                step_id,
                intent.unwrap_or_else(|| format!("wait for css selector '{sel}' to be hidden")),
                json!({
                    "subject": { "element": raw_locator("css", sel, "recorder waited on selector absence") },
                    "predicate": "isHidden",
                }),
            ))
        }
        "text" => {
            let text = cond
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("wait condition text requires 'text' string"))?;
            Ok(make_check(
                step_id,
                intent.unwrap_or_else(|| format!("wait for text '{text}' to be visible")),
                json!({
                    "subject": { "element": role_locator("generic", Some(text)) },
                    "predicate": "isVisible",
                }),
            ))
        }
        "url" => {
            let pattern = cond
                .get("pattern")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("wait condition url requires 'pattern' string"))?;
            Ok(make_check(
                step_id,
                intent.unwrap_or_else(|| format!("wait for URL to match /{pattern}/")),
                json!({
                    "subject": { "url": true },
                    "predicate": "matches",
                    "value": pattern,
                }),
            ))
        }
        other => bail!("unsupported wait condition kind {other:?}"),
    }
}

fn map_assert(p: &Json, step_id: &str) -> Result<Json> {
    let kind = p["kind"].as_str().expect("validated");
    let args = p["args"].as_array().expect("validated");
    let intent = p["intent"].as_str().expect("validated").to_string();
    match kind {
        "present" => {
            let (role, name) = pair(args)?;
            Ok(make_check(
                step_id,
                intent,
                json!({
                    "subject": { "element": role_locator(&role, name.as_deref()) },
                    "predicate": "isVisible",
                }),
            ))
        }
        "absent" => {
            let (role, name) = pair(args)?;
            Ok(make_check(
                step_id,
                intent,
                json!({
                    "subject": { "element": role_locator(&role, name.as_deref()) },
                    "predicate": "isHidden",
                }),
            ))
        }
        "url" => {
            let pattern = first_str(args)?;
            Ok(make_check(
                step_id,
                intent,
                json!({
                    "subject": { "url": true },
                    "predicate": "matches",
                    "value": pattern,
                }),
            ))
        }
        other => bail!("unsupported assert kind {other:?}"),
    }
}

// ---------- builders ----------

struct DoBody {
    on: Option<Json>,
    value: Option<Json>,
    params: Option<Json>,
}

fn make_do(step_id: &str, verb: &str, intent: String, body: DoBody) -> Json {
    let mut o = JsonMap::new();
    o.insert("id".into(), Json::String(step_id.to_string()));
    o.insert("kind".into(), Json::String("do".into()));
    o.insert("verb".into(), Json::String(verb.to_string()));
    o.insert("intent".into(), Json::String(intent));
    if let Some(on) = body.on {
        o.insert("on".into(), on);
    }
    if let Some(v) = body.value {
        o.insert("value".into(), v);
    }
    if let Some(p) = body.params {
        o.insert("params".into(), p);
    }
    Json::Object(o)
}

fn make_check(step_id: &str, intent: String, claim: Json) -> Json {
    json!({
        "id": step_id,
        "kind": "check",
        "intent": intent,
        "claim": claim,
    })
}

fn role_locator(role: &str, name: Option<&str>) -> Json {
    let mut o = JsonMap::new();
    o.insert("role".into(), Json::String(role.to_string()));
    if let Some(n) = name {
        o.insert("name".into(), Json::String(n.to_string()));
    }
    Json::Object(o)
}

fn raw_locator(kind: &str, value: &str, reason: &str) -> Json {
    json!({
        "raw": { "kind": kind, "value": value },
        "reason": reason,
    })
}

fn literal_value(s: &str) -> Json {
    json!({ "from": "literal", "literal": s })
}

// ---------- arg helpers ----------

fn first_str(args: &[Json]) -> Result<String> {
    arg_str(args, 0)
}

fn arg_str(args: &[Json], i: usize) -> Result<String> {
    args.get(i)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing args[{i}] (string)"))
}

/// Pull `(args[0] as string, Option<args[1] as string>)`. Used for the
/// `(role, name)` and `(label, value)` shapes.
fn pair(args: &[Json]) -> Result<(String, Option<String>)> {
    let a = arg_str(args, 0)?;
    let b = args.get(1).and_then(|v| v.as_str()).map(str::to_string);
    Ok((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_kind_rejected() {
        assert!(TriggerKind::parse("do").is_err());
        assert!(TriggerKind::parse("check").is_err());
        assert!(TriggerKind::parse("nope").is_err());
    }

    #[test]
    fn navigation_validates_route() {
        assert!(validate_trigger(TriggerKind::Navigation, &json!({})).is_err());
        assert!(validate_trigger(TriggerKind::Navigation, &json!({"route": ""})).is_err());
        assert!(validate_trigger(TriggerKind::Navigation, &json!({"route": "https://x/"})).is_ok());
    }

    #[test]
    fn action_method_must_be_on_allow_list() {
        let bad = json!({"method": "doTheThing", "args": []});
        let err = validate_trigger(TriggerKind::Action, &bad).unwrap_err();
        assert!(err.to_string().contains("allow-list"), "{err}");
        let good = json!({"method": "clickRole", "args": ["link", "Sport"]});
        validate_trigger(TriggerKind::Action, &good).unwrap();
    }

    #[test]
    fn assert_kind_must_be_on_allow_list() {
        let bad = json!({"kind": "count", "args": [1], "intent": "x"});
        assert!(validate_trigger(TriggerKind::Assert, &bad).is_err());
        let good = json!({"kind": "url", "args": ["/sport"], "intent": "loaded"});
        validate_trigger(TriggerKind::Assert, &good).unwrap();
    }

    #[test]
    fn assert_requires_non_empty_intent() {
        let bad = json!({"kind": "url", "args": ["/x"], "intent": ""});
        assert!(validate_trigger(TriggerKind::Assert, &bad).is_err());
        let bad2 = json!({"kind": "url", "args": ["/x"]});
        assert!(validate_trigger(TriggerKind::Assert, &bad2).is_err());
    }

    #[test]
    fn map_navigation_emits_goto_with_literal_value() {
        let row = json!({"route": "https://example.com/"});
        let step = map_row("navigation", &row, "s0").unwrap();
        assert_eq!(step["verb"], "goto");
        assert_eq!(step["value"]["literal"], "https://example.com/");
        assert!(step["intent"].as_str().unwrap().contains("navigate"));
    }

    #[test]
    fn map_action_clickrole_emits_click_with_role_locator() {
        let row = json!({
            "method": "clickRole",
            "args": ["link", "Sport"],
            "intent": "open sport",
        });
        let step = map_row("action", &row, "s1").unwrap();
        assert_eq!(step["verb"], "click");
        assert_eq!(step["on"]["role"], "link");
        assert_eq!(step["on"]["name"], "Sport");
        assert_eq!(step["intent"], "open sport");
    }

    #[test]
    fn map_action_fill_by_label_emits_type() {
        let row = json!({"method": "fillByLabel", "args": ["Email", "a@b.com"]});
        let step = map_row("action", &row, "s2").unwrap();
        assert_eq!(step["verb"], "type");
        assert_eq!(step["on"]["role"], "textbox");
        assert_eq!(step["on"]["name"], "Email");
        assert_eq!(step["value"]["literal"], "a@b.com");
    }

    #[test]
    fn map_wait_duration_emits_do_wait_with_params_ms() {
        let row = json!({"condition": {"kind": "duration", "ms": 250}});
        let step = map_row("wait", &row, "s3").unwrap();
        assert_eq!(step["verb"], "wait");
        assert_eq!(step["params"]["ms"], 250);
    }

    #[test]
    fn map_wait_url_emits_check_with_url_matches() {
        let row = json!({"condition": {"kind": "url", "pattern": "/sport"}});
        let step = map_row("wait", &row, "s4").unwrap();
        assert_eq!(step["kind"], "check");
        assert_eq!(step["claim"]["subject"]["url"], true);
        assert_eq!(step["claim"]["predicate"], "matches");
        assert_eq!(step["claim"]["value"], "/sport");
    }

    #[test]
    fn map_wait_selector_absent_emits_raw_css_ishidden() {
        let row = json!({"condition": {"kind": "selectorAbsent", "selector": "[data-testid=add-account-button]"}});
        let step = map_row("wait", &row, "s4").unwrap();
        assert_eq!(step["kind"], "check");
        assert_eq!(step["claim"]["subject"]["element"]["raw"]["kind"], "css");
        assert_eq!(
            step["claim"]["subject"]["element"]["raw"]["value"],
            "[data-testid=add-account-button]"
        );
        assert_eq!(step["claim"]["predicate"], "isHidden");
    }

    #[test]
    fn map_assert_url_emits_check_with_matches() {
        let row = json!({"kind": "url", "args": ["/sport"], "intent": "loaded"});
        let step = map_row("assert", &row, "s5").unwrap();
        assert_eq!(step["kind"], "check");
        assert_eq!(step["claim"]["predicate"], "matches");
        assert_eq!(step["claim"]["value"], "/sport");
        assert_eq!(step["intent"], "loaded");
    }

    #[test]
    fn map_assert_present_emits_isvisible_on_role_subject() {
        let row =
            json!({"kind": "present", "args": ["heading", "Welcome"], "intent": "page loaded"});
        let step = map_row("assert", &row, "s6").unwrap();
        assert_eq!(step["claim"]["subject"]["element"]["role"], "heading");
        assert_eq!(step["claim"]["subject"]["element"]["name"], "Welcome");
        assert_eq!(step["claim"]["predicate"], "isVisible");
    }
}
