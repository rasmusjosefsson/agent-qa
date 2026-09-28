//! Network mocks for replay — `do/mock` registers a stub, `do/unmock`
//! removes one or clears them all.
//!
//! Rules live in a per-session registry and are (re)installed into the live
//! page by wrapping `fetch` and `XMLHttpRequest`. Navigation wipes the
//! page's JS world, so the runner re-applies registered rules after
//! goto/reload/back/forward — mocks therefore survive SPA-free flows too.
//! A click that navigates still drops rules (documented: put mock steps
//! after navigation you can't predict, or replay accepts the miss).

use anyhow::Result;
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MockRule {
    /// Glob matched against the request URL — `*` is the only wildcard.
    pub url: String,
    /// HTTP status the stub responds with (default 200).
    pub status: u16,
    /// Response body — serialized JSON when `params.json` is given, else
    /// the raw `params.body` string, else `{}`.
    pub body: String,
    /// Optional artificial latency.
    pub delay_ms: u64,
}

static MOCKS: Mutex<Option<HashMap<String, Vec<MockRule>>>> = Mutex::new(None);

fn with_mocks<R>(session: &str, f: impl FnOnce(&mut Vec<MockRule>) -> R) -> R {
    let mut guard = MOCKS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map.entry(session.to_string()).or_default())
}

pub(crate) fn add(session: &str, rule: MockRule) {
    with_mocks(session, |v| v.push(rule));
}

/// Remove rules whose url glob equals `url`, or clear all when None.
pub(crate) fn clear(session: &str, url: Option<&str>) {
    with_mocks(session, |v| match url {
        Some(g) => v.retain(|r| r.url != g),
        None => v.clear(),
    });
}

pub(crate) fn rules(session: &str) -> Vec<MockRule> {
    with_mocks(session, |v| v.clone())
}

/// Parse a `do/mock` step's params into a rule.
/// `params`: `{ "url": "*/api/x*", "status": 503, "json": {...}|"body": "…",
/// "delayMs": 50 }`. `url` is required.
pub(crate) fn rule_from_params(params: &BTreeMap<String, Json>) -> Result<MockRule> {
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("mock: params.url is required (glob, e.g. \"*/api/*\")"))?;
    let status = params.get("status").and_then(|v| v.as_u64()).unwrap_or(200) as u16;
    let body = if let Some(j) = params.get("json") {
        serde_json::to_string(j)?
    } else if let Some(b) = params.get("body") {
        b.as_str()
            .ok_or_else(|| anyhow::anyhow!("mock: params.body must be a string"))?
            .to_string()
    } else {
        "{}".to_string()
    };
    let delay_ms = params.get("delayMs").and_then(|v| v.as_u64()).unwrap_or(0);
    Ok(MockRule {
        url: url.to_string(),
        status,
        body,
        delay_ms,
    })
}

/// The page-side stub: wraps `window.fetch` + `XMLHttpRequest` against a
/// `window.__qaMocks` rules array. Idempotent — re-install only refreshes
/// the rules array once the wrapper is in place.
fn install_js(rules: &[MockRule]) -> String {
    let rules_json = serde_json::to_string(rules).unwrap_or_else(|_| "[]".to_string());
    format!(
        r#"(() => {{
  const rules = {rules_json};
  window.__qaMocks = rules;
  if (window.__qaMocksInstalled) return rules.length;
  window.__qaMocksInstalled = true;
  const toRe = (g) => new RegExp('^' + g.split('*').map(s => s.replace(/[.*+?^${{}}()|[\]\\]/g, '\\$&')).join('.*') + '$');
  const match = (u) => (window.__qaMocks || []).find(r => toRe(r.url).test(u));
  const respond = (r) => new Response(r.body, {{ status: r.status, headers: {{ 'content-type': 'application/json' }} }});
  const of = window.fetch;
  window.fetch = (res, init) => {{
    const u = typeof res === 'string' ? res : ((res && res.url) || '');
    const r = match(u);
    if (!r) return of.call(window, res, init);
    return new Promise(done => setTimeout(() => done(respond(r)), r.delayMs || r.delay_ms || 0));
  }};
  const O = XMLHttpRequest.prototype.open, S = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function(m, u, ...rest) {{ this.__qaUrl = u; return O.call(this, m, u, ...rest); }};
  XMLHttpRequest.prototype.send = function(...a) {{
    const r = this.__qaUrl && match(this.__qaUrl);
    if (!r) return S.apply(this, a);
    const self = this;
    setTimeout(() => {{
      Object.defineProperty(self, 'status', {{ value: r.status }});
      Object.defineProperty(self, 'statusText', {{ value: String(r.status) }});
      Object.defineProperty(self, 'responseText', {{ value: r.body }});
      Object.defineProperty(self, 'response', {{ value: r.body }});
      Object.defineProperty(self, 'readyState', {{ value: 4 }});
      self.dispatchEvent(new Event('readystatechange'));
      self.dispatchEvent(new Event('load'));
      self.dispatchEvent(new Event('loadend'));
    }}, r.delayMs || r.delay_ms || 0);
  }};
  return rules.length;
}})()"#
    )
}

/// Install the current rules into the live page. No-op eval when empty —
/// cheap enough that the runner calls it after every navigation-capable
/// verb.
pub(crate) fn reapply_if_any(session: &str) -> Result<()> {
    let rs = rules(session);
    if rs.is_empty() {
        return Ok(());
    }
    crate::browser::eval_expression(session, &install_js(&rs))?;
    Ok(())
}

/// Parse a mock step and apply it immediately (register + install).
pub(crate) fn apply_mock(session: &str, params: &BTreeMap<String, Json>) -> Result<()> {
    let rule = rule_from_params(params)?;
    add(session, rule.clone());
    crate::browser::eval_expression(session, &install_js(&rules(session)))?;
    eprintln!(
        "[v2-replay] mock {} → {} ({} rule(s) active)",
        rule.url,
        rule.status,
        rules(session).len()
    );
    Ok(())
}

/// `unmock` — `params.url` (optional glob) removes that rule; absent clears
/// all. Re-installs so the live page sees the shrunken set.
pub(crate) fn apply_unmock(session: &str, params: Option<&BTreeMap<String, Json>>) -> Result<()> {
    let url = params.and_then(|p| p.get("url")).and_then(|v| v.as_str());
    clear(session, url);
    let rs = rules(session);
    if !rs.is_empty() {
        crate::browser::eval_expression(session, &install_js(&rs))?;
    } else {
        // Leave the inert wrapper installed with an empty rule set —
        // uninstalling mid-page risks racing in-flight code paths.
        crate::browser::eval_expression(session, &install_js(&[]))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn p(j: Json) -> BTreeMap<String, Json> {
        j.as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[test]
    fn rule_parse_defaults_and_full() {
        let r = rule_from_params(&p(json!({"url": "*/api/*"}))).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, "{}");
        assert_eq!(r.delay_ms, 0);

        let r = rule_from_params(&p(
            json!({"url": "*/u/1", "status": 503, "json": {"e": "down"}, "delayMs": 40}),
        ))
        .unwrap();
        assert_eq!(r.status, 503);
        assert!(r.body.contains("\"e\":\"down\""));
        assert_eq!(r.delay_ms, 40);

        assert!(rule_from_params(&p(json!({}))).is_err());
    }

    #[test]
    fn registry_adds_filters_and_clears() {
        let s = "mock-test-sess";
        clear(s, None);
        add(
            s,
            MockRule {
                url: "*/a".into(),
                status: 500,
                body: "{}".into(),
                delay_ms: 0,
            },
        );
        add(
            s,
            MockRule {
                url: "*/b".into(),
                status: 200,
                body: "{}".into(),
                delay_ms: 0,
            },
        );
        assert_eq!(rules(s).len(), 2);
        clear(s, Some("*/a"));
        assert_eq!(rules(s).len(), 1);
        assert_eq!(rules(s)[0].url, "*/b");
        clear(s, None);
        assert!(rules(s).is_empty());
    }

    #[test]
    fn install_js_embeds_rules_and_wraps_both_paths() {
        let js = install_js(&[MockRule {
            url: "*/api/x*".into(),
            status: 503,
            body: "{\"e\":1}".into(),
            delay_ms: 10,
        }]);
        assert!(js.contains("*/api/x*"));
        assert!(js.contains("503"));
        assert!(js.contains("window.fetch ="));
        assert!(js.contains("XMLHttpRequest.prototype.send"));
        assert!(js.contains("__qaMocksInstalled"));
    }
}
