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

#[derive(Debug, Default)]
struct MockState {
    rules: Vec<MockRule>,
    /// `--offline`: requests matching NO rule get a network rejection
    /// instead of passing through to the real backend.
    strict: bool,
}

static MOCKS: Mutex<Option<HashMap<String, MockState>>> = Mutex::new(None);

fn with_mocks<R>(session: &str, f: impl FnOnce(&mut MockState) -> R) -> R {
    let mut guard = MOCKS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map.entry(session.to_string()).or_default())
}

pub(crate) fn add(session: &str, rule: MockRule) {
    with_mocks(session, |s| s.rules.push(rule));
}

/// Remove rules whose url glob equals `url`, or clear all when None.
/// Clearing also drops strict mode — `unmock` without a url means "mock
/// phase over", and an orphaned catch-all would keep rejecting.
pub(crate) fn clear(session: &str, url: Option<&str>) {
    with_mocks(session, |s| match url {
        Some(g) => s.rules.retain(|r| r.url != g),
        None => {
            s.rules.clear();
            s.strict = false;
        }
    });
}

/// `--offline`: unmatched requests reject instead of reaching the backend.
pub(crate) fn set_strict(session: &str, strict: bool) {
    with_mocks(session, |s| s.strict = strict);
}

fn is_strict(session: &str) -> bool {
    with_mocks(session, |s| s.strict)
}

pub(crate) fn rules(session: &str) -> Vec<MockRule> {
    with_mocks(session, |s| s.rules.clone())
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
    let status = match params.get("status") {
        None => 200,
        Some(v) => {
            let n = v
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("mock: params.status must be a number (got {v})"))?;
            // u16 field — a wider literal would silently truncate
            // (99999 → 34463); anything outside the 3-digit HTTP range
            // is a typo the scenario should fail on, not a mock.
            u16::try_from(n)
                .ok()
                .filter(|s| (100..=999).contains(s))
                .ok_or_else(|| anyhow::anyhow!("mock: params.status must be 100–999 (got {n})"))?
        }
    };
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
fn install_js(rules: &[MockRule], strict: bool) -> String {
    let rules_json = serde_json::to_string(rules).unwrap_or_else(|_| "[]".to_string());
    let fetch_miss = if strict {
        // Strict misses never reach the wire — log them as aborted so the
        // request log records what the app *tried* to do.
        "logCall(m, u, null, pd, null, true); return new Promise((_, rej) => setTimeout(() => rej(new TypeError('Failed to fetch (offline)')), 0));"
    } else {
        // Passthrough requests are captured by CDP — no log entry needed.
        "return of.call(window, res, init);"
    };
    let xhr_miss = if strict {
        "{ logCall(this.__qaMethod || 'GET', this.__qaUrl, null, (a && a[0]) || null, null, true); setTimeout(() => { this.dispatchEvent(new Event('error')); this.dispatchEvent(new Event('loadend')); }, 0); return; }"
    } else {
        "return S.apply(this, a);"
    };
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
  // In-page stubs never reach the wire, so CDP capture can't see them.
  // Mirror every intercepted call into `__aqMockLog` — network claims and
  // the run's network.json merge it so mocked traffic stays assertable.
  window.__aqMockLog = window.__aqMockLog || [];
  window.__aqMockSeq = window.__aqMockSeq || 0;
  const logCall = (method, u, status, postData, responseBody, aborted) => {{
    window.__aqMockLog.push({{
      id: 'mock-' + (++window.__aqMockSeq), url: abs(u), method,
      status, postData: postData || null, responseBody: responseBody ?? null,
      mocked: true, aborted: !!aborted, t: Date.now(),
    }});
  }};
  const of = window.fetch;
  window.fetch = (res, init) => {{
    const u = typeof res === 'string' ? res : ((res && res.url) || '');
    const r = match(u);
    const m = ((init && init.method) || (res && res.method) || 'GET').toUpperCase();
    const pd = (init && typeof init.body === 'string') ? init.body : ((res && typeof res.body === 'string') ? res.body : null);
    if (!r) {{ {fetch_miss} }}
    if (r.abort) {{ logCall(m, u, 0, pd, null, true); return new Promise((_, rej) => setTimeout(() => rej(new TypeError('Failed to fetch')), r.delayMs || r.delay_ms || 0)); }}
    logCall(m, u, r.status, pd, r.body, false);
    return new Promise(done => setTimeout(() => done(respond(r)), r.delayMs || r.delay_ms || 0));
  }};
  const O = XMLHttpRequest.prototype.open, S = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function(m, u, ...rest) {{ this.__qaUrl = u; this.__qaMethod = m; return O.call(this, m, u, ...rest); }};
  XMLHttpRequest.prototype.send = function(...a) {{
    const r = this.__qaUrl && match(this.__qaUrl);
    if (!r) {xhr_miss}
    const self = this;
    if (r.abort) {{
      logCall(self.__qaMethod || 'GET', self.__qaUrl, 0, (a && typeof a[0] === 'string' ? a[0] : null), null, true);
      setTimeout(() => {{
        self.dispatchEvent(new Event('error'));
        self.dispatchEvent(new Event('loadend'));
      }}, r.delayMs || r.delay_ms || 0);
      return;
    }}
    logCall(self.__qaMethod || 'GET', self.__qaUrl, r.status, (a && typeof a[0] === 'string' ? a[0] : null), r.body, false);
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
    if rs.is_empty() && !is_strict(session) {
        return Ok(());
    }
    crate::browser::eval_expression(session, &install_js(&rs, is_strict(session)))?;
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
    if har_is_stale(&har_path, &scenario_dir.join("scenario.json")) {
        eprintln!(
            "[v2-replay] --mock-from {run_id:?}: scenario.json is newer than the recording — re-record with `replay --har` if steps changed"
        );
    }
    seed_from_har_file(session, &har_path)
}

/// Like `seed_from_har` but takes the HAR file's path directly — `start
/// --mock-from <path>` records against stubs without a scenario dir.
pub(crate) fn seed_from_har_path(session: &str, har_path: &Path) -> Result<usize> {
    seed_from_har_file(session, har_path)
}

fn seed_from_har_file(session: &str, har_path: &Path) -> Result<usize> {
    let text = fs::read_to_string(har_path).with_context(|| {
        format!(
            "--mock-from: no network.har at {} (record one with `replay --har`)",
            har_path.display()
        )
    })?;
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
    let body = install_js(&rules(session), is_strict(session));
    fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Parse a mock step and apply it immediately (register + install).
pub(crate) fn apply_mock(session: &str, params: &BTreeMap<String, Json>) -> Result<()> {
    let rule = rule_from_params(params)?;
    add(session, rule.clone());
    crate::browser::eval_expression(session, &install_js(&rules(session), is_strict(session)))?;
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
    apply_unmock_without_browser(session, params)?;
    let rs = rules(session);
    crate::browser::eval_expression(session, &install_js(&rs, is_strict(session)))?;
    Ok(())
}

/// Registry side of `unmock` — split out so tests exercise it without a
/// live page. A present-but-non-string `url` must NOT fall through to the
/// clear-all branch: `{"url": true}` wiping every rule silently is the
/// worst possible outcome for a typo.
fn apply_unmock_without_browser(
    session: &str,
    params: Option<&BTreeMap<String, Json>>,
) -> Result<()> {
    let url = match params.and_then(|p| p.get("url")) {
        None => None,
        Some(v) => Some(
            v.as_str()
                .ok_or_else(|| anyhow::anyhow!("unmock: params.url must be a string glob"))?,
        ),
    };
    if let Some(g) = url {
        if !rules(session).iter().any(|r| r.url == g) {
            eprintln!(
                "[v2-replay] unmock: no rule matches {g:?} — nothing removed ({} rule(s) active)",
                rules(session).len()
            );
        }
    }
    clear(session, url);
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
    fn rule_parse_rejects_out_of_range_status_instead_of_truncating() {
        // `as u16` used to silently wrap: 99999 → 34463.
        assert!(rule_from_params(&p(json!({"url": "*", "status": 99999}))).is_err());
        assert!(rule_from_params(&p(json!({"url": "*", "status": 0}))).is_err());
        assert!(rule_from_params(&p(json!({"url": "*", "status": 99}))).is_err());
        assert!(rule_from_params(&p(json!({"url": "*", "status": "503"}))).is_err());
        // Non-standard 3-digit codes (nginx 499, CF 5xx) stay legal.
        assert_eq!(
            rule_from_params(&p(json!({"url": "*", "status": 499})))
                .unwrap()
                .status,
            499
        );
    }

    #[test]
    fn unmock_with_a_nonstring_url_errors_instead_of_clearing_all() {
        let s = "unmock-typo-sess";
        clear(s, None);
        add(
            s,
            MockRule {
                url: "*/keep".into(),
                status: 200,
                body: "{}".into(),
                delay_ms: 0,
                abort: false,
            },
        );
        // `{"url": true}` must not wipe the registry.
        let params = p(json!({"url": true}));
        let err = apply_unmock_without_browser(s, Some(&params)).unwrap_err();
        assert!(err.to_string().contains("string glob"));
        assert_eq!(rules(s).len(), 1);
        clear(s, None);
    }

    #[test]
    fn unmock_with_an_unmatched_glob_warns_but_keeps_rules() {
        let s = "unmock-miss-sess";
        clear(s, None);
        add(
            s,
            MockRule {
                url: "*/keep".into(),
                status: 200,
                body: "{}".into(),
                delay_ms: 0,
                abort: false,
            },
        );
        let params = p(json!({"url": "*/typo"}));
        apply_unmock_without_browser(s, Some(&params)).unwrap();
        assert_eq!(rules(s).len(), 1, "unmatched glob removes nothing");
        clear(s, None);
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
        let js = install_js(
            &[MockRule {
                url: "*/api/x*".into(),
                status: 503,
                body: "{\"e\":1}".into(),
                delay_ms: 10,
                abort: false,
            }],
            false,
        );
        assert!(js.contains("*/api/x*"));
        assert!(js.contains("503"));
        assert!(js.contains("window.fetch ="));
        assert!(js.contains("XMLHttpRequest.prototype.send"));
        assert!(js.contains("__qaMocksInstalled"));
    }

    #[test]
    fn strict_mode_rejects_unmatched_requests() {
        let js = install_js(&[], true);
        assert!(js.contains("Failed to fetch (offline)"));
        // non-strict keeps the real-backend passthrough
        let js = install_js(&[], false);
        assert!(js.contains("return of.call(window, res, init)"));
    }

    #[test]
    fn abort_rule_rejects_fetch_and_errors_xhr() {
        let r = rule_from_params(&p(json!({"url": "*/api/*", "abort": true}))).unwrap();
        assert!(r.abort);
        let js = install_js(&[r], false);
        assert!(js.contains("Failed to fetch"));
        assert!(js.contains("new Event('error')"));
    }

    #[test]
    fn seed_from_har_path_accepts_a_direct_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let har = tmp.path().join("network.har");
        std::fs::write(
            &har,
            r#"{"log":{"entries":[
              {"request":{"url":"https://x/api/u","method":"GET"},
               "response":{"status":201,"content":{"text":"{\"ok\":true}"}}}
            ]}}"#,
        )
        .unwrap();
        let n = seed_from_har_path("s-direct", &har).unwrap();
        assert_eq!(n, 1);
        let rs = rules("s-direct");
        assert_eq!(rs[0].url, "https://x/api/u");
        assert_eq!(rs[0].status, 201);
        assert_eq!(rs[0].body, "{\"ok\":true}");
        assert!(seed_from_har_path("s-miss", &tmp.path().join("none.har")).is_err());
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
