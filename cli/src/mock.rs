//! Network mocks for replay — `do/mock` registers a stub, `do/unmock`
//! removes one or clears them all.
//!
//! Rules live in a per-session registry and are (re)installed into the live
//! page by wrapping `fetch` and `XMLHttpRequest`. Navigation wipes the
//! page's JS world, so the runner re-applies registered rules after
//! goto/reload/back/forward — mocks therefore survive SPA-free flows too.
//! A click that navigates still drops rules (documented: put mock steps
//! after navigation you can't predict, or replay accepts the miss).

use anyhow::{Context, Result};
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
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
    /// Reject instead of respond — models an unreachable backend:
    /// fetch gets a `TypeError: Failed to fetch`, XHR fires `error`.
    pub abort: bool,
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

/// Stale-recording guard: scenario.json edited after the HAR was captured
/// means the stub set may predate the requests the scenario now makes.
fn har_is_stale(har_path: &Path, scenario_json: &Path) -> bool {
    let (Ok(h), Ok(s)) = (fs::metadata(har_path), fs::metadata(scenario_json)) else {
        return false;
    };
    let (Ok(ht), Ok(st)) = (h.modified(), s.modified()) else {
        return false;
    };
    st > ht
}

/// Parse a `do/mock` step's params into a rule.
/// `params`: `{ "url": "*/api/x*", "status": 503, "json": {...}|"body": "…",
/// "delayMs": 50, "abort": true }`. `url` is required; `abort: true`
/// rejects the request instead of responding (offline / backend-down).
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
    let abort = params
        .get("abort")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok(MockRule {
        url: url.to_string(),
        status,
        body,
        delay_ms,
        abort,
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
  const abs = (u) => {{ try {{ return new URL(u, location.href).href; }} catch {{ return u; }} }};
  const match = (u) => (window.__qaMocks || []).find(r => toRe(r.url).test(u) || toRe(r.url).test(abs(u)));
  const respond = (r) => new Response(r.body, {{ status: r.status, headers: {{ 'content-type': 'application/json' }} }});
  const of = window.fetch;
  window.fetch = (res, init) => {{
    const u = typeof res === 'string' ? res : ((res && res.url) || '');
    const r = match(u);
    if (!r) return of.call(window, res, init);
    if (r.abort) return new Promise((_, rej) => setTimeout(() => rej(new TypeError('Failed to fetch')), r.delayMs || r.delay_ms || 0));
    return new Promise(done => setTimeout(() => done(respond(r)), r.delayMs || r.delay_ms || 0));
  }};
  const O = XMLHttpRequest.prototype.open, S = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function(m, u, ...rest) {{ this.__qaUrl = u; return O.call(this, m, u, ...rest); }};
  XMLHttpRequest.prototype.send = function(...a) {{
    const r = this.__qaUrl && match(this.__qaUrl);
    if (!r) return S.apply(this, a);
    const self = this;
    if (r.abort) {{
      setTimeout(() => {{
        self.dispatchEvent(new Event('error'));
        self.dispatchEvent(new Event('loadend'));
      }}, r.delayMs || r.delay_ms || 0);
      return;
    }}
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

/// Seed mock rules from a previous run's `network.har` (written by
/// `replay --har`). Each entry becomes an exact-URL rule carrying the
/// recorded status + body — replay then answers the app's fetch/XHR
/// calls from that recording instead of the real backend (hermetic,
/// deterministic, offline-capable replay).
///
/// Caveats of the in-page stub apply: only `fetch`/`XMLHttpRequest`
/// traffic is covered (document/sub-resource loads still hit the
/// network); a URL requested several times replays the FIRST recorded
/// response (the stub has no per-call sequence).
pub(crate) fn seed_from_har(session: &str, scenario_dir: &Path, run_id: &str) -> Result<usize> {
    let har_path = scenario_dir
        .join("replays")
        .join(run_id)
        .join("network.har");
    let text = fs::read_to_string(&har_path).with_context(|| {
        format!(
            "--mock-from {run_id:?}: no network.har at {} (record one with `replay --har`)",
            har_path.display()
        )
    })?;
    if har_is_stale(&har_path, &scenario_dir.join("scenario.json")) {
        eprintln!(
            "[v2-replay] --mock-from {run_id:?}: scenario.json is newer than the recording — re-record with `replay --har` if steps changed"
        );
    }
    let har: Json = serde_json::from_str(&text)
        .with_context(|| format!("--mock-from: parse {}", har_path.display()))?;
    let entries = har["log"]["entries"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("--mock-from: {} has no log.entries", har_path.display()))?;
    let mut seeded = 0usize;
    let mut seen: HashMap<String, ()> = HashMap::new();
    for e in entries {
        let url = match e["request"]["url"].as_str() {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => continue,
        };
        // First response wins for a repeated URL.
        if seen.insert(url.clone(), ()).is_some() {
            continue;
        }
        let status = e["response"]["status"].as_i64().unwrap_or(200);
        let status = u16::try_from(status).unwrap_or(200);
        let content = &e["response"]["content"];
        let body = match content["text"].as_str() {
            Some(t) if content["encoding"].as_str() == Some("base64") => {
                use base64::Engine;
                String::from_utf8_lossy(
                    &base64::engine::general_purpose::STANDARD
                        .decode(t)
                        .unwrap_or_default(),
                )
                .into_owned()
            }
            Some(t) => t.to_string(),
            None => String::new(),
        };
        add(
            session,
            MockRule {
                url,
                status,
                body,
                delay_ms: 0,
                abort: false,
            },
        );
        seeded += 1;
    }
    Ok(seeded)
}

/// Write the session's current rules as a page-init script at
/// `<dir>/mock-init.js`. Registered via `AGENT_BROWSER_INIT_SCRIPTS`, it
/// runs before every navigation — covering page-load fetches the
/// eval-install path can't reach.
pub(crate) fn write_init_script(session: &str, dir: &Path) -> Result<std::path::PathBuf> {
    let path = dir.join("mock-init.js");
    let body = install_js(&rules(session));
    fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Parse a mock step and apply it immediately (register + install).
pub(crate) fn apply_mock(session: &str, params: &BTreeMap<String, Json>) -> Result<()> {
    let rule = rule_from_params(params)?;
    add(session, rule.clone());
    crate::browser::eval_expression(session, &install_js(&rules(session)))?;
    eprintln!(
        "[v2-replay] mock {} → {} ({} rule(s) active)",
        rule.url,
        if rule.abort {
            "abort".to_string()
        } else {
            rule.status.to_string()
        },
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
                abort: false,
            },
        );
        add(
            s,
            MockRule {
                url: "*/b".into(),
                status: 200,
                body: "{}".into(),
                delay_ms: 0,
                abort: false,
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
            abort: false,
        }]);
        assert!(js.contains("*/api/x*"));
        assert!(js.contains("503"));
        assert!(js.contains("window.fetch ="));
        assert!(js.contains("XMLHttpRequest.prototype.send"));
        assert!(js.contains("__qaMocksInstalled"));
    }

    #[test]
    fn abort_rule_rejects_fetch_and_errors_xhr() {
        let r = rule_from_params(&p(json!({"url": "*/api/*", "abort": true}))).unwrap();
        assert!(r.abort);
        let js = install_js(&[r]);
        assert!(js.contains("Failed to fetch"));
        assert!(js.contains("new Event('error')"));
    }

    #[test]
    fn har_is_stale_flags_scenario_newer_than_recording() {
        let tmp = tempfile::TempDir::new().unwrap();
        let har = tmp.path().join("network.har");
        let scn = tmp.path().join("scenario.json");
        std::fs::write(&har, "{}").unwrap();
        std::fs::write(&scn, "{}").unwrap();
        // Same-mtime is racy — force order via filetime-free check: touch scn
        // by rewriting after a tick.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&scn, "{\"a\":1}").unwrap();
        assert!(har_is_stale(&har, &scn));
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&har, "{\"b\":1}").unwrap();
        assert!(!har_is_stale(&har, &scn));
        // Missing files never flag stale.
        assert!(!har_is_stale(&har, &tmp.path().join("nope.json")));
    }

    #[test]
    fn seed_from_har_registers_first_response_per_url() {
        let tmp = tempfile::TempDir::new().unwrap();
        let run_dir = tmp.path().join("replays").join("rA");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(
            run_dir.join("network.har"),
            r#"{"log":{"entries":[
              {"request":{"url":"https://x/api/user","method":"GET"},
               "response":{"status":200,"content":{"text":"{\"name\":\"ada\"}"}}},
              {"request":{"url":"https://x/api/user","method":"GET"},
               "response":{"status":500,"content":{"text":"later"}}},
              {"request":{"url":"https://x/bin","method":"GET"},
               "response":{"status":200,"content":{"text":"aGk=","encoding":"base64"}}}
            ]}}"#,
        )
        .unwrap();
        let n = seed_from_har("s-seed", tmp.path(), "rA").unwrap();
        assert_eq!(n, 2, "the repeated URL seeds once (first response wins)");
        let rs = rules("s-seed");
        let user = rs.iter().find(|r| r.url == "https://x/api/user").unwrap();
        assert_eq!(user.status, 200);
        assert_eq!(user.body, "{\"name\":\"ada\"}");
        let bin = rs.iter().find(|r| r.url == "https://x/bin").unwrap();
        assert_eq!(bin.body, "hi", "base64 bodies decode to text");
        clear("s-seed", None);
    }
}
