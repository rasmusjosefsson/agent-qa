//! `scenario/2` Value channel resolver + template substitution.
//!
//! Mirrors `replay/value.ts`. Three public entrypoints:
//!
//!   - [`resolve_value`] — turn a typed [`crate::scenario::Value`] into a
//!     concrete [`serde_json::Value`] using the per-run [`ValueScope`].
//!   - [`substitute_scenario_vars`] — substitute `{{vars.<name>(.path)?}}`
//!     tokens in a string. Powers the recorder's `fill-unique` literal
//!     templates and any other scenario/2 fill that embeds variable
//!     references.
//!   - [`select_json_path`] — minimal JSONPath subset (dotted, integer
//!     index incl. negative, wildcard, slice, `<json>` segment).
//!
//! This module is consumed by replay; the recorder side
//! ([`renderMintTemplate`] equivalent) re-uses the same primitives.
//!
//! Generic — no vendor vocabulary, no per-template anchors. `{{vars.*}}`
//! is the scenario/2 binding form, NOT a vendor convention.
//!
//! Types are present but `valueToString` and a couple of helpers
//! aren't called by any verb yet; suppress the dead-code warning at
//! module scope until the runner lands.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Result};
use rand::RngCore;
use regex::Regex;
use serde_json::Value as Json;

use crate::scenario::{MintScope, Value};

// ---------- ValueScope ----------

#[derive(Debug, Default)]
pub struct ValueScope {
    /// Inputs declared at root, resolved from CLI / defaults.
    pub inputs: HashMap<String, Json>,
    /// Bindings produced by previous `do` steps' `saveAs`.
    pub saved_steps: HashMap<String, Json>,
    /// Loop variable bindings (set by `loop` verb during iteration).
    pub loop_vars: HashMap<String, Json>,
    /// Cache of mint renders, keyed by `<scope>::<template>` (or by name
    /// for input-mints — see [`substitute_scenario_vars`]).
    pub mint_cache: HashMap<String, String>,
}

impl ValueScope {
    pub fn new(inputs: HashMap<String, Json>) -> Self {
        Self {
            inputs,
            saved_steps: HashMap::new(),
            loop_vars: HashMap::new(),
            mint_cache: HashMap::new(),
        }
    }
}

// ---------- resolve_value ----------

pub fn resolve_value(v: &Value, scope: &mut ValueScope) -> Result<Json> {
    match v {
        Value::Literal { literal } => {
            // Generic template substitution on literal strings: when the
            // recorder embedded `{{vars._unique}}` or any `{{vars.<name>}}`
            // token in the literal, substitute from the per-run scope.
            // Non-string literals pass through unchanged.
            if let Json::String(s) = literal {
                Ok(Json::String(substitute_scenario_vars(s, scope)))
            } else {
                Ok(literal.clone())
            }
        }
        Value::Input { input } => scope.inputs.get(input).cloned().ok_or_else(|| {
            anyhow!("Value(from=input): input '{input}' not declared / not supplied")
        }),
        Value::Step { step_id, path } => {
            let base = scope.saved_steps.get(step_id).cloned().ok_or_else(|| {
                anyhow!("Value(from=step): no saved binding for stepId '{step_id}'")
            })?;
            match path {
                Some(p) => select_json_path(&base, p),
                None => Ok(base),
            }
        }
        Value::Mint { mint } => {
            let scope_kind = mint
                .scope
                .as_ref()
                .map(format_mint_scope)
                .unwrap_or("scenario");
            let key = format!("{scope_kind}::{}", mint.template);
            if let Some(cached) = scope.mint_cache.get(&key) {
                return Ok(Json::String(cached.clone()));
            }
            let rendered = render_mint_template(&mint.template);
            scope.mint_cache.insert(key, rendered.clone());
            Ok(Json::String(rendered))
        }
        Value::Loop { loop_var } => {
            let mut parts = loop_var.split('.');
            let root = parts
                .next()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow!("Value(from=loop): empty loopVar"))?;
            let mut cur = scope
                .loop_vars
                .get(root)
                .cloned()
                .ok_or_else(|| anyhow!("Value(from=loop): no loop var '{root}'"))?;
            for seg in parts {
                cur = match cur {
                    Json::Object(mut map) => map.remove(seg).unwrap_or(Json::Null),
                    _ => bail!("Value(from=loop): cannot read '.{seg}' from non-object"),
                };
            }
            Ok(cur)
        }
    }
}

fn format_mint_scope(s: &MintScope) -> &'static str {
    match s {
        MintScope::Scenario => "scenario",
        MintScope::Loop => "loop",
        MintScope::Template => "template",
    }
}

// ---------- substitute_scenario_vars ----------

fn vars_re() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| {
        Regex::new(
            r"\{\{\s*vars\.([A-Za-z_][A-Za-z0-9_]*)((?:\.[A-Za-z_$][A-Za-z0-9_$]*|\[[^\]]+\])*)\s*\}\}",
        )
        .expect("vars regex compiles")
    })
}

/// Substitute scenario/2 `{{vars.<name>}}` template tokens in a string.
///
/// Resolution order per token:
///   1. `vars._unique` — per-run minted random hex (8 chars). Cached so
///      every reference within one run gets the same value.
///   2. `vars.<name>` — checks `scope.saved_steps[<name>]` first, then
///      `scope.inputs[<name>]`. Inputs that are themselves `{from: mint,
///      …}` Values get a per-name render (so two distinct mint inputs
///      with the same template produce different tokens).
///   3. Unknown — leaves the token literal so the failure is observable
///      (typing `{{vars.foo}}` into a form is easier to diagnose than
///      typing nothing).
pub fn substitute_scenario_vars(s: &str, scope: &mut ValueScope) -> String {
    let re = vars_re();
    let mut out = String::with_capacity(s.len());
    let mut last = 0usize;
    for cap in re.captures_iter(s) {
        let whole = cap.get(0).unwrap();
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let tail = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        out.push_str(&s[last..whole.start()]);
        last = whole.end();
        let resolved = resolve_var_token(name, tail, scope);
        match resolved {
            Some(v) => out.push_str(&v),
            None => out.push_str(whole.as_str()),
        }
    }
    out.push_str(&s[last..]);
    out
}

fn resolve_var_token(name: &str, tail: &str, scope: &mut ValueScope) -> Option<String> {
    if name == "_unique" {
        let key = "scenario::_unique".to_string();
        if let Some(cached) = scope.mint_cache.get(&key) {
            return Some(cached.clone());
        }
        let minted = mint_hex();
        scope.mint_cache.insert(key, minted.clone());
        return Some(minted);
    }
    let root_resolved: Json = if let Some(stored) = scope.saved_steps.get(name).cloned() {
        resolve_if_value(stored, scope)
    } else {
        let stored = scope.inputs.get(name).cloned()?;
        // Input-mint disambiguation: cache by `<scope>::vars.<name>`.
        if let Some(mint) = as_mint_value(&stored) {
            let key = format!("{}::vars.{}", mint.scope.unwrap_or("scenario"), name);
            if let Some(cached) = scope.mint_cache.get(&key) {
                Json::String(cached.clone())
            } else {
                let rendered = render_mint_template(mint.template);
                scope.mint_cache.insert(key, rendered.clone());
                Json::String(rendered)
            }
        } else {
            resolve_if_value(stored, scope)
        }
    };
    let final_value = if tail.is_empty() {
        root_resolved
    } else {
        walk_path(&root_resolved, tail)?
    };
    match final_value {
        Json::String(s) => Some(s),
        Json::Null => None,
        other => Some(value_to_string(&other)),
    }
}

/// Coerce a stored input/savedStep that may itself be a scenario/2 Value
/// (`{from: 'mint', mint: {…}}`) into its resolved value. Plain
/// primitives + arrays + non-Value objects pass through unchanged.
fn resolve_if_value(v: Json, scope: &mut ValueScope) -> Json {
    let Json::Object(ref obj) = v else { return v };
    let Some(Json::String(_)) = obj.get("from") else {
        return v;
    };
    // Try to deserialize as a typed Value; on success, recurse.
    match serde_json::from_value::<Value>(v.clone()) {
        Ok(typed) => resolve_value(&typed, scope).unwrap_or(v),
        Err(_) => v,
    }
}

#[derive(Debug)]
struct MintRef<'a> {
    template: &'a str,
    scope: Option<&'a str>,
}

fn as_mint_value(v: &Json) -> Option<MintRef<'_>> {
    let obj = v.as_object()?;
    if obj.get("from")?.as_str()? != "mint" {
        return None;
    }
    let mint = obj.get("mint")?.as_object()?;
    Some(MintRef {
        template: mint.get("template")?.as_str()?,
        scope: mint.get("scope").and_then(|v| v.as_str()),
    })
}

// ---------- mint helpers ----------

fn mint_hex() -> String {
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(s, "{byte:02x}").unwrap();
    }
    s
}

pub fn render_mint_template(template: &str) -> String {
    let unique = mint_hex();
    template.replace("{{unique}}", &unique)
}

// ---------- string coercion ----------

/// Coerce a resolved Value to a string for verbs that need a string.
pub fn value_to_string(raw: &Json) -> String {
    match raw {
        Json::String(s) => s.clone(),
        Json::Null => String::new(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// ---------- walk_path (limited dotted+bracket path used by vars.<name>.tail) ----------

/// Walk a dotted/bracketed tail (`.x.y[0].z`) against a JSON-shaped root.
/// Returns `None` when any segment dereferences null / non-object /
/// non-array — caller treats this as "unresolved" and leaves the
/// template literal in the string. Bracket syntax supports integer
/// indices only; richer JSONPath lives in [`select_json_path`].
fn walk_path(root: &Json, path: &str) -> Option<Json> {
    let bytes = path.as_bytes();
    let mut i = 0usize;
    let mut cur: Json = root.clone();
    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
                    i += 1;
                }
                let key = &path[start..i];
                if key.is_empty() {
                    return None;
                }
                cur = match cur {
                    Json::Object(mut map) => map.remove(key)?,
                    _ => return None,
                };
            }
            b'[' => {
                let end = path[i..].find(']').map(|p| i + p)?;
                let inside = &path[i + 1..end];
                i = end + 1;
                let idx: i64 = inside.parse().ok()?;
                let arr = cur.as_array()?;
                let len = arr.len() as i64;
                let resolved_idx = if idx < 0 { len + idx } else { idx };
                if resolved_idx < 0 || resolved_idx >= len {
                    return None;
                }
                cur = arr[resolved_idx as usize].clone();
            }
            _ => return None,
        }
    }
    Some(cur)
}

// ---------- select_json_path (the richer JSONPath subset) ----------

/// Minimal JSONPath subset. Supported:
///   - `$.a.b`            — dotted property access
///   - `$.a[0].b`         — non-negative integer index
///   - `$.a[-1].b`        — negative index
///   - `$.a[*].b`         — wildcard (returns array, recurses on tail)
///   - `$.a[N:M]`         — slice (half-open, like Python)
///   - `$.a[N:]` / `$.a[:M]` / `$.a[-N:]`
///   - `$.a[N:M].b`       — slice + dotted tail (recurses on each element)
///   - `$.a.<json>.b`     — parse current string as JSON and continue
///
/// Explicitly NOT supported (returns Err): filter expressions
/// (`[?…]`), recursive descent (`..`), union (`[0,2]`), 3-part slice,
/// script expressions.
pub fn select_json_path(root: &Json, path: &str) -> Result<Json> {
    if !path.starts_with('$') {
        bail!("JSONPath must start with '$': {path}");
    }
    let mut cur = root.clone();
    let bytes = path.as_bytes();
    let mut i = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
                    i += 1;
                }
                let key = &path[start..i];
                if key.is_empty() {
                    bail!("empty segment in JSONPath: {path}");
                }
                if key == "<json>" {
                    let s = cur
                        .as_str()
                        .ok_or_else(|| anyhow!("<json> segment requires string-valued current node in JSONPath: {path}"))?;
                    cur = serde_json::from_str(s).map_err(|e| {
                        anyhow!("<json> segment: failed to parse string as JSON in JSONPath: {path} ({e})")
                    })?;
                    continue;
                }
                let obj = cur.as_object().ok_or_else(|| {
                    anyhow!("cannot read '.{key}' from non-object in JSONPath: {path}")
                })?;
                cur = obj.get(key).cloned().unwrap_or(Json::Null);
            }
            b'[' => {
                let end = path[i..]
                    .find(']')
                    .map(|p| i + p)
                    .ok_or_else(|| anyhow!("unterminated [...] in JSONPath: {path}"))?;
                let inside = &path[i + 1..end];
                i = end + 1;
                if inside == "*" {
                    let arr = cur
                        .as_array()
                        .ok_or_else(|| anyhow!("[*] requires array in JSONPath: {path}"))?
                        .clone();
                    let tail = &path[i..];
                    if tail.is_empty() {
                        return Ok(Json::Array(arr));
                    }
                    let sub = format!("${tail}");
                    let mut acc: Vec<Json> = Vec::with_capacity(arr.len());
                    for el in arr {
                        let r = select_json_path(&el, &sub)?;
                        match r {
                            Json::Array(many) => acc.extend(many),
                            other => acc.push(other),
                        }
                    }
                    return Ok(Json::Array(acc));
                }
                if inside.contains(['?', '@', '(', ')']) {
                    bail!("filter / script expressions not supported in JSONPath: {path}");
                }
                if inside.contains(':') {
                    let arr = cur
                        .as_array()
                        .ok_or_else(|| {
                            anyhow!("slice [{inside}] requires array in JSONPath: {path}")
                        })?
                        .clone();
                    let parts: Vec<&str> = inside.split(':').collect();
                    if parts.len() != 2 {
                        bail!("step / 3-part slice not supported in JSONPath: {path}");
                    }
                    let len = arr.len() as i64;
                    let parse_bound = |raw: &str, dflt: i64| -> Result<i64> {
                        if raw.is_empty() {
                            return Ok(dflt);
                        }
                        let n: i64 = raw.parse().map_err(|_| {
                            anyhow!("non-numeric slice bound '{raw}' in JSONPath: {path}")
                        })?;
                        Ok(if n < 0 {
                            (0i64).max(len + n)
                        } else {
                            len.min(n)
                        })
                    };
                    let lo = parse_bound(parts[0], 0)?;
                    let hi = parse_bound(parts[1], len)?;
                    let lo = lo.max(0).min(len) as usize;
                    let hi = hi.max(0).min(len) as usize;
                    let sliced: Vec<Json> = if lo >= hi {
                        Vec::new()
                    } else {
                        arr[lo..hi].to_vec()
                    };
                    let tail = &path[i..];
                    if tail.is_empty() {
                        return Ok(Json::Array(sliced));
                    }
                    let sub = format!("${tail}");
                    let out: Vec<Json> = sliced
                        .iter()
                        .map(|el| select_json_path(el, &sub))
                        .collect::<Result<Vec<_>>>()?;
                    return Ok(Json::Array(out));
                }
                let idx: i64 = inside
                    .parse()
                    .map_err(|_| anyhow!("non-numeric index '{inside}' in JSONPath: {path}"))?;
                let arr = cur
                    .as_array()
                    .ok_or_else(|| anyhow!("[{idx}] requires array in JSONPath: {path}"))?;
                let len = arr.len() as i64;
                let resolved = if idx < 0 { len + idx } else { idx };
                if resolved < 0 || resolved >= len {
                    cur = Json::Null;
                } else {
                    cur = arr[resolved as usize].clone();
                }
            }
            ch => bail!("unexpected '{}' in JSONPath: {path}", ch as char),
        }
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{MintScope, MintSpec, Value};
    use serde_json::json;

    fn scope_with(inputs: serde_json::Map<String, Json>) -> ValueScope {
        let map: HashMap<String, Json> = inputs.into_iter().collect();
        ValueScope::new(map)
    }

    // ----- select_json_path -----

    fn data() -> Json {
        json!({
            "users": [
                { "id": "u1", "name": "a" },
                { "id": "u2", "name": "b" },
                { "id": "u3", "name": "c" },
                { "id": "u4", "name": "d" },
            ]
        })
    }

    #[test]
    fn jsonpath_dotted_and_index() {
        assert_eq!(
            select_json_path(&data(), "$.users[0].id").unwrap(),
            json!("u1")
        );
        assert_eq!(
            select_json_path(&data(), "$.users[2].name").unwrap(),
            json!("c")
        );
    }

    #[test]
    fn jsonpath_wildcard() {
        assert_eq!(
            select_json_path(&data(), "$.users[*].id").unwrap(),
            json!(["u1", "u2", "u3", "u4"])
        );
    }

    #[test]
    fn jsonpath_slice() {
        assert_eq!(
            select_json_path(&data(), "$.users[0:2]").unwrap(),
            json!([{ "id": "u1", "name": "a" }, { "id": "u2", "name": "b" }])
        );
        assert_eq!(
            select_json_path(&data(), "$.users[1:3].id").unwrap(),
            json!(["u2", "u3"])
        );
    }

    #[test]
    fn jsonpath_half_open_slice() {
        assert_eq!(
            select_json_path(&data(), "$.users[2:].id").unwrap(),
            json!(["u3", "u4"])
        );
        assert_eq!(
            select_json_path(&data(), "$.users[:2].id").unwrap(),
            json!(["u1", "u2"])
        );
    }

    #[test]
    fn jsonpath_negative_slice_and_index() {
        assert_eq!(
            select_json_path(&data(), "$.users[-1:].id").unwrap(),
            json!(["u4"])
        );
        assert_eq!(
            select_json_path(&data(), "$.users[-2:].id").unwrap(),
            json!(["u3", "u4"])
        );
        assert_eq!(
            select_json_path(&data(), "$.users[-1].id").unwrap(),
            json!("u4")
        );
    }

    #[test]
    fn jsonpath_clamps_out_of_range() {
        assert_eq!(
            select_json_path(&data(), "$.users[0:99].id").unwrap(),
            json!(["u1", "u2", "u3", "u4"])
        );
        assert_eq!(
            select_json_path(&data(), "$.users[10:20]").unwrap(),
            json!([])
        );
    }

    #[test]
    fn jsonpath_rejects_filter_and_step_slice() {
        assert!(select_json_path(&data(), "$.users[?(@.id==\"u1\")]").is_err());
        assert!(select_json_path(&data(), "$.users[0:4:2]").is_err());
        assert!(select_json_path(&data(), "$.users[a:b]").is_err());
        assert!(select_json_path(&json!({ "x": 1 }), "$.x[0:1]").is_err());
    }

    #[test]
    fn jsonpath_json_segment() {
        let d = json!({
            "roleGroupsHierarchy": {
                "collection": "[{\"guid\":\"a\",\"name\":\"X\"},{\"guid\":\"b\",\"name\":\"Y\"}]"
            }
        });
        assert_eq!(
            select_json_path(&d, "$.roleGroupsHierarchy.collection.<json>[0].guid").unwrap(),
            json!("a")
        );
        assert_eq!(
            select_json_path(&d, "$.roleGroupsHierarchy.collection.<json>[*].guid").unwrap(),
            json!(["a", "b"])
        );
    }

    #[test]
    fn jsonpath_json_segment_errors() {
        assert!(select_json_path(&json!({ "x": { "y": 1 } }), "$.x.<json>.y").is_err());
        assert!(select_json_path(&json!({ "x": "not json" }), "$.x.<json>").is_err());
    }

    // ----- resolve_value -----

    #[test]
    fn resolve_step_value_with_slice() {
        let mut sc = ValueScope::default();
        sc.saved_steps.insert(
            "things".into(),
            json!({ "collection": [{ "id": "1" }, { "id": "2" }, { "id": "3" }] }),
        );
        let v = Value::Step {
            step_id: "things".into(),
            path: Some("$.collection[0:1]".into()),
        };
        let out = resolve_value(&v, &mut sc).unwrap();
        assert_eq!(out, json!([{ "id": "1" }]));
    }

    #[test]
    fn resolve_input_missing_errors() {
        let mut sc = ValueScope::default();
        let v = Value::Input {
            input: "nope".into(),
        };
        assert!(resolve_value(&v, &mut sc).is_err());
    }

    #[test]
    fn resolve_mint_caches_per_scope_template() {
        let mut sc = ValueScope::default();
        let v = Value::Mint {
            mint: MintSpec {
                template: "x-{{unique}}".into(),
                scope: Some(MintScope::Scenario),
            },
        };
        let a = resolve_value(&v, &mut sc).unwrap();
        let b = resolve_value(&v, &mut sc).unwrap();
        assert_eq!(a, b, "same scope+template caches");
        assert!(a.as_str().unwrap().starts_with("x-"));
    }

    #[test]
    fn resolve_loop_dotted() {
        let mut sc = ValueScope::default();
        sc.loop_vars.insert("u".into(), json!({ "first": "alice" }));
        let v = Value::Loop {
            loop_var: "u.first".into(),
        };
        assert_eq!(resolve_value(&v, &mut sc).unwrap(), json!("alice"));
    }

    // ----- substitute_scenario_vars -----

    #[test]
    fn substitute_from_saved_steps() {
        let mut sc = ValueScope::default();
        sc.saved_steps.insert("foo".into(), json!("bar"));
        assert_eq!(
            substitute_scenario_vars("hello {{vars.foo}} world", &mut sc),
            "hello bar world"
        );
    }

    #[test]
    fn substitute_from_inputs() {
        let mut sc = scope_with([("user".into(), json!("alice"))].into_iter().collect());
        assert_eq!(
            substitute_scenario_vars("hi {{vars.user}}", &mut sc),
            "hi alice"
        );
    }

    #[test]
    fn substitute_unknown_leaves_token_literal() {
        let mut sc = ValueScope::default();
        assert_eq!(
            substitute_scenario_vars("{{vars.unknown}}", &mut sc),
            "{{vars.unknown}}"
        );
    }

    #[test]
    fn substitute_dotted_path_walk() {
        let mut sc = ValueScope::default();
        sc.saved_steps.insert(
            "createTeamGroup_3".into(),
            json!({ "createGroup2": { "group": { "guid": "abc-123" } } }),
        );
        assert_eq!(
            substitute_scenario_vars(
                "parent {{vars.createTeamGroup_3.createGroup2.group.guid}}",
                &mut sc
            ),
            "parent abc-123"
        );
    }

    #[test]
    fn substitute_bracket_index() {
        let mut sc = ValueScope::default();
        sc.saved_steps.insert(
            "list".into(),
            json!({ "items": [{ "id": "1" }, { "id": "2" }] }),
        );
        assert_eq!(
            substitute_scenario_vars("{{vars.list.items[1].id}}", &mut sc),
            "2"
        );
    }

    #[test]
    fn substitute_unique_is_scenario_stable() {
        let mut sc = ValueScope::default();
        let a = substitute_scenario_vars("{{vars._unique}}", &mut sc);
        let b = substitute_scenario_vars("{{vars._unique}}", &mut sc);
        assert_eq!(a, b);
        assert_eq!(a.len(), 8);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn substitute_input_mint_resolves_lazily() {
        let mut sc = scope_with(
            [(
                "mint_0".into(),
                json!({ "from": "mint", "mint": { "template": "{{unique}}", "scope": "scenario" } }),
            )]
            .into_iter()
            .collect(),
        );
        let out = substitute_scenario_vars("Child {{vars.mint_0}}", &mut sc);
        assert!(
            regex::Regex::new(r"^Child [a-f0-9]{8}$")
                .unwrap()
                .is_match(&out),
            "got {out:?}"
        );
        let out2 = substitute_scenario_vars("also {{vars.mint_0}}", &mut sc);
        assert_eq!(
            out.split_whitespace().nth(1),
            out2.split_whitespace().nth(1)
        );
    }

    #[test]
    fn substitute_two_distinct_mint_inputs_get_different_tokens() {
        let mut sc = scope_with(
            [
                (
                    "mint_0".into(),
                    json!({ "from": "mint", "mint": { "template": "{{unique}}", "scope": "scenario" } }),
                ),
                (
                    "mint_1".into(),
                    json!({ "from": "mint", "mint": { "template": "{{unique}}", "scope": "scenario" } }),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let a = substitute_scenario_vars("{{vars.mint_0}}", &mut sc);
        let b = substitute_scenario_vars("{{vars.mint_1}}", &mut sc);
        assert_eq!(a.len(), 8);
        assert_eq!(b.len(), 8);
        assert_ne!(a, b, "per-name cache must distinguish mint inputs");
        let a2 = substitute_scenario_vars("{{vars.mint_0}}", &mut sc);
        assert_eq!(a, a2);
    }
}
