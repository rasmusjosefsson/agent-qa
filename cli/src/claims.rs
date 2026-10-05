//! scenario/2 `check`-step Claim evaluator.
//!
//! Claim = `{ subject, predicate, value?, tolerance? }`. Subject arms:
//!
//!   - `element` — `{ element: Locator, attribute?, ofKind? }`
//!   - `url`     — `{ url: true }`
//!   - `network` — `{ network: NetworkMatcher, ofKind?, path? }`
//!   - `data`    — `{ data: <savedName>, path? }`
//!   - `flag`    — `{ flag: <name> }`
//!   - `var`     — `{ kind: 'var', name: <savedName>, path? }`
//!
//! Scope:
//!   - `element`: `isVisible` / `exists` (locator resolves) and
//!     `isHidden` / `notExists` (locator stops resolving) with timeout
//!     polling. Other element predicates raise not-yet-implemented.
//!   - `url`: `equals`, `matches`, `contains`, `startsWith`, `endsWith`
//!     against both `location.href` and `location.pathname` with
//!     polling (either matches).
//!   - `data` / `var`: `exists`, `notExists`, `equals`, `contains`,
//!     `matches`, `startsWith`, `endsWith`.
//!   - `network`: `fired` (exists/notExists on any captured request matching
//!     `urlMatches`/`operationName`/`method`), `status` (predicates on the
//!     latest match's HTTP status), `responseJsonPath` (`path` + predicates on
//!     the latest match's JSON response body). All forms poll.
//!   - `flag`: structured not-yet-implemented boundary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde_json::Value as Json;

use crate::browser::{self, CapturedRequest, RoleAct};
use crate::scenario::{
    Claim, ClaimSubject, ElementClaimKind, HttpMethod, Locator, NetworkClaimKind, NetworkMatcher,
    Predicate, RawLocatorKind,
};
use crate::value::{select_json_path, substitute_scenario_vars, value_to_string, ValueScope};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const MAX_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct CheckContext<'a> {
    pub session: &'a str,
    /// Scenario directory — `{"file": ...}` claim subjects resolve relative
    /// paths against it (download steps save there by default).
    pub scenario_dir: &'a Path,
    /// Replay run directory — `{"shot": ...}` claims read the current
    /// screenshot from `<run>/screenshots/<stepId>.png` and write the diff
    /// map under `<run>/shots-diff/`. `None` outside a replay (run-step
    /// debugging) — shot claims bail there.
    pub run_dir: Option<&'a Path>,
}

pub fn dispatch_check(
    claim: &Claim,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Option<Duration>,
) -> Result<()> {
    let timeout = timeout.unwrap_or(DEFAULT_TIMEOUT).min(MAX_TIMEOUT);
    match &claim.subject {
        ClaimSubject::Element {
            element,
            attribute,
            of_kind,
        } => match of_kind {
            None => check_element(
                element,
                attribute.as_deref(),
                &claim.predicate,
                claim.value.as_ref(),
                ctx,
                scope,
                timeout,
            ),
            Some(ElementClaimKind::Count) => check_element_count(
                element,
                &claim.predicate,
                claim.value.as_ref(),
                ctx,
                scope,
                timeout,
            ),
            // text/value/attribute are friendly spellings for the property/
            // attribute reads the plain element claim already supports —
            // `ofKind:"text"` ≡ `attribute:"text"` (textContent),
            // `ofKind:"value"` ≡ `attribute:"value"` (live IDL property),
            // `ofKind:"attribute"` just makes the `attribute:` field required.
            Some(ElementClaimKind::Text) => check_element(
                element,
                Some("text"),
                &claim.predicate,
                claim.value.as_ref(),
                ctx,
                scope,
                timeout,
            ),
            Some(ElementClaimKind::Value) => check_element(
                element,
                Some("value"),
                &claim.predicate,
                claim.value.as_ref(),
                ctx,
                scope,
                timeout,
            ),
            Some(ElementClaimKind::Attribute) => {
                let attr = attribute.as_deref().ok_or_else(|| {
                    anyhow!("element claim ofKind 'attribute' requires the 'attribute' field")
                })?;
                check_element(
                    element,
                    Some(attr),
                    &claim.predicate,
                    claim.value.as_ref(),
                    ctx,
                    scope,
                    timeout,
                )
            }
        },
        ClaimSubject::Url { url: _ } => {
            check_url(&claim.predicate, claim.value.as_ref(), ctx, scope, timeout)
        }
        ClaimSubject::File { file, attribute } => check_file(
            file,
            attribute.as_deref(),
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
            timeout,
        ),
        ClaimSubject::Data { data, path } => {
            let actual = read_saved(scope, data, path.as_deref())?;
            check_value(&actual, &claim.predicate, claim.value.as_ref(), scope)
        }
        ClaimSubject::Var { kind, name, path } => {
            if kind != "var" {
                bail!("var subject requires kind='var', got {kind:?}");
            }
            let actual = read_saved(scope, name, path.as_deref())?;
            check_value(&actual, &claim.predicate, claim.value.as_ref(), scope)
        }
        ClaimSubject::Network {
            network,
            of_kind,
            path,
        } => check_network(
            network,
            of_kind.as_ref(),
            path.as_deref(),
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
            timeout,
        ),
        ClaimSubject::Flag { flag } => {
            check_flag(flag, &claim.predicate, claim.value.as_ref(), ctx)
        }
        ClaimSubject::Shot { shot, clip, .. } => check_shot(
            shot,
            clip.as_ref(),
            &claim.predicate,
            claim.tolerance.as_ref(),
            ctx,
            scope,
        ),
        ClaimSubject::Domshot { domshot, skip } => {
            check_domshot(domshot, skip, &claim.predicate, ctx)
        }
        ClaimSubject::Layout { layout } => {
            check_layout(layout, &claim.predicate, claim.tolerance.as_ref(), ctx)
        }

        ClaimSubject::Dialog { dialog } => {
            if !*dialog {
                bail!("dialog subject requires dialog=true");
            }
            check_dialog(&claim.predicate, claim.value.as_ref(), ctx, scope, timeout)
        }
        ClaimSubject::Console { console } => check_console(
            console,
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
            timeout,
        ),
        ClaimSubject::PageError { page_error } => check_page_error(
            page_error,
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
            timeout,
        ),
        ClaimSubject::Storage { storage, path } => check_storage(
            storage,
            path.as_deref(),
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
        ),
        ClaimSubject::Cookie { cookie } => {
            check_cookie(cookie, &claim.predicate, claim.value.as_ref(), ctx, scope)
        }
        ClaimSubject::Clipboard { clipboard } => {
            if !*clipboard {
                bail!("clipboard subject requires clipboard=true");
            }
            check_clipboard(&claim.predicate, claim.value.as_ref(), ctx, scope)
        }
        ClaimSubject::IndexedDb { indexeddb, path } => check_indexeddb(
            indexeddb,
            path.as_deref(),
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
        ),
        ClaimSubject::Timing { timing } => {
            check_timing(timing, &claim.predicate, claim.value.as_ref(), ctx, scope)
        }
        ClaimSubject::A11y { a11y } => check_a11y(
            a11y,
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
            timeout,
        ),
        ClaimSubject::Perf { perf } => check_perf(
            perf,
            &claim.predicate,
            claim.value.as_ref(),
            ctx,
            scope,
            timeout,
        ),
    }
}

// ---------- console ----------

/// Check console messages captured this session via
/// `agent-browser console --json`. The matcher filters which messages count:
/// `{"console": true}` → all; `{"type": "error"}` → that level;
/// `{"text": "<substring>"}` → message text contains it.
///
/// Predicates:
///   `exists`/`isVisible`      ≥1 matching message
///   `notExists`/`isHidden`    zero matching — the "page logged no errors" gate
///   `countEquals`/`gt`/`gte`/`lt`/`lte`  compare the matching count to `value`
///   text predicates           ANY matching message's text satisfies them
fn check_console(
    subject: &crate::scenario::ConsoleSubject,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    use crate::scenario::ConsoleSubject;
    let (want_type, want_text) = match subject {
        ConsoleSubject::Flag(true) => (None, None),
        ConsoleSubject::Flag(false) => bail!("console subject requires console=true or a matcher"),
        ConsoleSubject::Matcher(m) => (
            m.r#type
                .as_deref()
                .map(|t| substitute_scenario_vars(t, scope)),
            m.text
                .as_deref()
                .map(|t| substitute_scenario_vars(t, scope)),
        ),
    };
    let deadline = Instant::now() + timeout;
    loop {
        let msgs = browser::console_messages(ctx.session).unwrap_or_default();
        let matched: Vec<&crate::browser::ConsoleMessage> = msgs
            .iter()
            .filter(|m| {
                want_type.as_deref().map(|t| m.level == t).unwrap_or(true)
                    && want_text
                        .as_deref()
                        .map(|t| m.text.contains(t))
                        .unwrap_or(true)
            })
            .collect();
        let done = match predicate {
            Predicate::Exists | Predicate::IsVisible => !matched.is_empty(),
            Predicate::NotExists | Predicate::IsHidden => matched.is_empty(),
            Predicate::CountEquals
            | Predicate::Gt
            | Predicate::Gte
            | Predicate::Lt
            | Predicate::Lte => {
                let need = expected.and_then(|v| v.as_u64()).ok_or_else(|| {
                    anyhow!(
                        "console claim with predicate '{predicate:?}' requires a numeric 'value'"
                    )
                })?;
                let n = matched.len() as u64;
                match predicate {
                    Predicate::CountEquals => n == need,
                    Predicate::Gt => n > need,
                    Predicate::Gte => n >= need,
                    Predicate::Lt => n < need,
                    _ => n <= need,
                }
            }
            other @ (Predicate::Equals
            | Predicate::Contains
            | Predicate::Matches
            | Predicate::StartsWith
            | Predicate::EndsWith) => {
                let need = expected.ok_or_else(|| {
                    anyhow!("console claim with predicate '{other:?}' requires 'value'")
                })?;
                let need = substitute_scenario_vars(&value_to_string(need), scope);
                matched
                    .iter()
                    .any(|m| compare_string(other, &m.text, &need).is_ok())
            }
            other => bail!("console subject does not support predicate '{other:?}'"),
        };
        if done {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let sample: Vec<String> = msgs
                .iter()
                .take(3)
                .map(|m| format!("[{}] {}", m.level, m.text))
                .collect();
            bail!(
                "console claim timed out ({} messages; {} matched; latest: {})",
                msgs.len(),
                matched.len(),
                if sample.is_empty() {
                    "<none>".to_string()
                } else {
                    sample.join(" | ")
                }
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
}

// ---------- page errors (uncaught exceptions) ----------

/// Check uncaught exceptions captured this session via
/// `agent-browser errors --json` — a different channel from `console`:
/// uncaught throws (window.onerror / Runtime.exceptionThrown) land here,
/// `console.*` calls do not. Matcher `{"pageError": {"text": "s",
/// "url": "u"}}` filters on the error text / raising document URL.
///
/// Predicates:
///   `exists`/`isVisible`      ≥1 matching error
///   `notExists`/`isHidden`    zero matching — the "page threw nothing" gate
///   `countEquals`/`gt`/`gte`/`lt`/`lte`  compare the matching count to `value`
///   text predicates           ANY matching error's text satisfies them
fn check_page_error(
    subject: &crate::scenario::PageErrorSubject,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    use crate::scenario::PageErrorSubject;
    let (want_text, want_url) = match subject {
        PageErrorSubject::Flag(true) => (None, None),
        PageErrorSubject::Flag(false) => {
            bail!("pageError subject requires pageError=true or a matcher")
        }
        PageErrorSubject::Matcher(m) => (
            m.text
                .as_deref()
                .map(|t| substitute_scenario_vars(t, scope)),
            m.url.as_deref().map(|t| substitute_scenario_vars(t, scope)),
        ),
    };
    let deadline = Instant::now() + timeout;
    loop {
        let errs = browser::page_errors(ctx.session).unwrap_or_default();
        let matched: Vec<&crate::browser::PageError> = errs
            .iter()
            .filter(|e| {
                want_text
                    .as_deref()
                    .map(|t| e.text.contains(t))
                    .unwrap_or(true)
                    && want_url
                        .as_deref()
                        .map(|u| e.url.as_deref().map(|eu| eu.contains(u)).unwrap_or(false))
                        .unwrap_or(true)
            })
            .collect();
        let done = match predicate {
            Predicate::Exists | Predicate::IsVisible => !matched.is_empty(),
            Predicate::NotExists | Predicate::IsHidden => matched.is_empty(),
            Predicate::CountEquals
            | Predicate::Gt
            | Predicate::Gte
            | Predicate::Lt
            | Predicate::Lte => {
                let need = expected.and_then(|v| v.as_u64()).ok_or_else(|| {
                    anyhow!(
                        "pageError claim with predicate '{predicate:?}' requires a numeric 'value'"
                    )
                })?;
                let n = matched.len() as u64;
                match predicate {
                    Predicate::CountEquals => n == need,
                    Predicate::Gt => n > need,
                    Predicate::Gte => n >= need,
                    Predicate::Lt => n < need,
                    _ => n <= need,
                }
            }
            other @ (Predicate::Equals
            | Predicate::Contains
            | Predicate::Matches
            | Predicate::StartsWith
            | Predicate::EndsWith) => {
                let need = expected.ok_or_else(|| {
                    anyhow!("pageError claim with predicate '{other:?}' requires 'value'")
                })?;
                let need = substitute_scenario_vars(&value_to_string(need), scope);
                matched
                    .iter()
                    .any(|e| compare_string(other, &e.text, &need).is_ok())
            }
            other => bail!("pageError subject does not support predicate '{other:?}'"),
        };
        if done {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let sample: Vec<String> = errs
                .iter()
                .take(3)
                .map(|e| e.text.lines().next().unwrap_or_default().to_string())
                .collect();
            bail!(
                "pageError claim timed out ({} errors; {} matched; latest: {})",
                errs.len(),
                matched.len(),
                if sample.is_empty() {
                    "<none>".to_string()
                } else {
                    sample.join(" | ")
                }
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
}

// ---------- web performance ----------

/// Read a Performance-API metric at claim time. Observer-buffered
/// types (lcp/layout-shift/longtask) flush as a task, so the eval
/// polls until entries arrive or the dwell expires.
fn perf_metric_js(metric: &str, dwell_ms: u64) -> String {
    format!(
        r#"(async () => {{
  const metric = {metric:?};
  const dwell = {dwell_ms};
  if (metric === 'fcp') {{
    const e = performance.getEntriesByName('first-contentful-paint')[0];
    return JSON.stringify(e ? e.startTime : null);
  }}
  if (metric === 'ttfb' || metric === 'load') {{
    const n = performance.getEntriesByType('navigation')[0];
    if (!n) return 'null';
    return JSON.stringify(metric === 'ttfb' ? n.responseStart : n.duration);
  }}
  if (metric === 'lcp' || metric === 'cls' || metric === 'tbt') {{
    const type = metric === 'lcp' ? 'largest-contentful-paint' : metric === 'cls' ? 'layout-shift' : 'longtask';
    return await new Promise((resolve) => {{
      let entries = [];
      const calc = () => {{
        if (metric === 'lcp') return entries.length ? entries[entries.length - 1].startTime : null;
        if (metric === 'cls') return entries.filter(e => !e.hadRecentInput).reduce((s, e) => s + e.value, 0);
        return entries.reduce((s, e) => s + Math.max(0, e.duration - 50), 0);
      }};
      const po = new PerformanceObserver((list) => {{ entries = entries.concat(list.getEntries()); }});
      try {{ po.observe({{ type, buffered: true }}); }} catch (e) {{ return resolve('null'); }}
      const deadline = performance.now() + dwell;
      const tick = () => {{
        if (entries.length || performance.now() >= deadline) {{
          try {{ po.disconnect(); }} catch (e) {{}}
          return resolve(JSON.stringify(calc()));
        }}
        setTimeout(tick, 100);
      }};
      setTimeout(tick, 100);
    }});
  }}
  return JSON.stringify('__aq_unknown__');
}})()"#
    )
}

/// `{"perf": "lcp"}` / `{"perf": {"metric": "lcp", "maxDwellMs": 1500}}` —
/// a Web Performance budget as a gateable claim. `exists`/`notExists`
/// test whether the metric was recorded (lcp with no paint → absent);
/// numeric predicates compare the value (ms; cls is the unitless score).
fn check_perf(
    subject: &crate::scenario::PerfSubject,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    use crate::scenario::PerfSubject;
    let (metric, dwell_ms) = match subject {
        PerfSubject::Metric(m) => (
            substitute_scenario_vars(m, scope).to_ascii_lowercase(),
            1_500u64,
        ),
        PerfSubject::Matcher(m) => (
            substitute_scenario_vars(&m.metric, scope).to_ascii_lowercase(),
            m.max_dwell_ms.unwrap_or(1_500),
        ),
    };
    const METRICS: &[&str] = &["fcp", "lcp", "cls", "tbt", "ttfb", "load"];
    if !METRICS.contains(&metric.as_str()) {
        bail!(
            "perf metric {metric:?} unknown — expected one of {}",
            METRICS.join(", ")
        );
    }
    let js = perf_metric_js(&metric, dwell_ms);
    // Poll until the claim's condition is met — a metric that isn't
    // there yet (lcp before the user-idle dwell) may still land.
    let deadline = Instant::now() + timeout;
    loop {
        let raw = browser::eval_expression(ctx.session, &js)?;
        let text: String =
            serde_json::from_str(raw.trim()).unwrap_or_else(|_| raw.trim().to_string());
        let value: Option<f64> = serde_json::from_str::<Json>(&text)
            .ok()
            .and_then(|v| v.as_f64());
        let done = match predicate {
            Predicate::Exists | Predicate::IsVisible => value.is_some(),
            Predicate::NotExists | Predicate::IsHidden => value.is_none(),
            Predicate::Equals
            | Predicate::CountEquals
            | Predicate::Gt
            | Predicate::Gte
            | Predicate::Lt
            | Predicate::Lte => {
                let need = expected.and_then(|v| v.as_f64()).ok_or_else(|| {
                    anyhow!("perf claim with predicate '{predicate:?}' requires a numeric 'value'")
                })?;
                match (value, predicate) {
                    (Some(v), Predicate::Equals | Predicate::CountEquals) => v == need,
                    (Some(v), Predicate::Gt) => v > need,
                    (Some(v), Predicate::Gte) => v >= need,
                    (Some(v), Predicate::Lt) => v < need,
                    (Some(v), Predicate::Lte) => v <= need,
                    (None, _) => false,
                    _ => unreachable!(),
                }
            }
            other => bail!("perf subject does not support predicate '{other:?}'"),
        };
        if done {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "perf check failed: {metric} = {}",
                value
                    .map(|v| format!("{v:.1}"))
                    .unwrap_or_else(|| "absent".to_string())
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
}

// ---------- accessibility (axe-core via `agent-browser a11y`) ----------

/// Check the page's axe audit (`agent-browser a11y --json`). The matcher
/// filters which findings count:
///   `{"a11y": true}`                     every violation
///   `{"a11y": {"impact": "serious"}}`    violations at that impact or worse
///   `{"a11y": {"rule": "color-contrast"}}`  only that axe rule id
///   `{"a11y": {"within": "#app"}}`       audit scoped to a subtree
///   `{"a11y": {"incomplete": true}}`     also count axe's incomplete results
///
/// Predicates:
///   `exists`/`isVisible`     ≥1 matching finding
///   `notExists`/`isHidden`   zero matching — the "page is clean" gate
///   numeric predicates       compare the matching count to `value`
fn check_a11y(
    subject: &crate::scenario::A11ySubject,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    use crate::scenario::A11ySubject;
    let (floor, within, rule, incomplete) = match subject {
        A11ySubject::Flag(true) => (None, None, None, false),
        A11ySubject::Flag(false) => bail!("a11y subject requires a11y=true or a matcher object"),
        A11ySubject::Matcher(m) => (
            m.impact
                .as_deref()
                .map(|s| substitute_scenario_vars(s, scope)),
            m.within
                .as_deref()
                .map(|s| substitute_scenario_vars(s, scope)),
            m.rule
                .as_deref()
                .map(|s| substitute_scenario_vars(s, scope)),
            m.incomplete,
        ),
    };
    // axe impact ordering — a floor keeps that level and everything worse.
    let rank = |impact: &str| match impact {
        "minor" => 0,
        "moderate" => 1,
        "serious" => 2,
        "critical" => 3,
        _ => -1,
    };
    let floor_n = match floor.as_deref() {
        Some(f) => {
            let n = rank(f);
            if n < 0 {
                bail!("a11y matcher 'impact' must be minor|moderate|serious|critical; got {f:?}");
            }
            Some(n)
        }
        None => None,
    };
    let deadline = Instant::now() + timeout;
    loop {
        let data = browser::a11y_audit(ctx.session, within.as_deref()).unwrap_or_default();
        let mut matched: Vec<Json> = Vec::new();
        if let Some(vs) = data.get("violations").and_then(|v| v.as_array()) {
            matched.extend(vs.iter().cloned());
        }
        if incomplete {
            if let Some(is) = data.get("incomplete").and_then(|v| v.as_array()) {
                matched.extend(is.iter().cloned());
            }
        }
        matched.retain(|v| {
            let impact_ok = floor_n
                .map(|f| {
                    v.get("impact")
                        .and_then(|i| i.as_str())
                        .map(|i| rank(i) >= f)
                        .unwrap_or(false)
                })
                .unwrap_or(true);
            let rule_ok = rule
                .as_deref()
                .map(|r| v.get("id").and_then(|i| i.as_str()) == Some(r))
                .unwrap_or(true);
            impact_ok && rule_ok
        });
        let n = matched.len() as u64;
        let done = match predicate {
            Predicate::Exists | Predicate::IsVisible => n > 0,
            Predicate::NotExists | Predicate::IsHidden => n == 0,
            Predicate::CountEquals
            | Predicate::Gt
            | Predicate::Gte
            | Predicate::Lt
            | Predicate::Lte => {
                let need = expected.and_then(|v| v.as_u64()).ok_or_else(|| {
                    anyhow!("a11y claim with predicate '{predicate:?}' requires a numeric 'value'")
                })?;
                match predicate {
                    Predicate::CountEquals => n == need,
                    Predicate::Gt => n > need,
                    Predicate::Gte => n >= need,
                    Predicate::Lt => n < need,
                    _ => n <= need,
                }
            }
            other => bail!("a11y subject does not support predicate '{other:?}'"),
        };
        if done {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let ids: Vec<String> = matched
                .iter()
                .filter_map(|v| v.get("id").and_then(|i| i.as_str()).map(str::to_string))
                .collect();
            bail!(
                "a11y check failed: {} matching violation(s){}",
                n,
                if ids.is_empty() {
                    String::new()
                } else {
                    format!(" — rules: {}", ids.join(", "))
                }
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
}

// ---------- native dialog ----------

/// Check a pending native dialog (alert/confirm/prompt/beforeunload) via
/// `agent-browser dialog status`.
///
/// Predicates supported:
///   `exists` / `isVisible`   a dialog is currently open
///   `notExists` / `isHidden` no dialog is open
///   text predicates (`equals`, `contains`, `matches`, `startsWith`,
///   `endsWith`) compare against the dialog's `message` — they require a
///   dialog to be pending.
fn check_dialog(
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    match predicate {
        Predicate::Exists | Predicate::IsVisible => {
            poll_until(timeout, |_| match browser::dialog_status(ctx.session) {
                Ok(s) if s.open => Ok(()),
                Ok(_) => bail!("no dialog is currently open"),
                Err(err) => bail!("dialog status: {err}"),
            })
        }
        Predicate::NotExists | Predicate::IsHidden => {
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                match browser::dialog_status(ctx.session) {
                    Ok(s) if !s.open => return Ok(()),
                    _ => thread::sleep(POLL_INTERVAL),
                }
            }
            bail!("expected no pending dialog, but one is still open");
        }
        Predicate::Equals
        | Predicate::Contains
        | Predicate::Matches
        | Predicate::StartsWith
        | Predicate::EndsWith => {
            let expected = expected.ok_or_else(|| {
                anyhow!("dialog claim with predicate '{predicate:?}' requires 'value'")
            })?;
            let need = substitute_scenario_vars(&value_to_string(expected), scope);
            let deadline = Instant::now() + timeout;
            let mut last_err: Option<anyhow::Error> = None;
            let mut last_status = String::new();
            while Instant::now() < deadline {
                match browser::dialog_status(ctx.session) {
                    Ok(s) if s.open => {
                        last_status = format!("{} {:?}", s.kind, s.message);
                        match compare_string(predicate, &s.message, &need) {
                            Ok(()) => return Ok(()),
                            Err(err) => last_err = Some(err),
                        }
                    }
                    Ok(_) => {
                        last_status = "no dialog open".to_string();
                        last_err = Some(anyhow!("no dialog is currently open"));
                    }
                    Err(err) => last_err = Some(anyhow!("dialog status: {err}")),
                }
                thread::sleep(POLL_INTERVAL);
            }
            Err(last_err
                .unwrap_or_else(|| anyhow!("dialog claim timed out; last status={last_status}")))
        }
        other => bail!("dialog subject does not support predicate '{other:?}'"),
    }
}

// ---------- flag ----------

/// Check a flag value stored in `localStorage['devtools-flag-overrides']`
/// (the same key written by [`crate::env_ops::EnvOp::Flag`]). The map
/// shape matches: `{ "<flag>": true|false, … }`.
///
/// Predicates supported:
///   `equals`              expected `value` is true / false
///   `exists`              flag key is present (boolean true OR false)
///   `notExists`           flag key absent
///   `isVisible`           alias for `equals true`
///   `isHidden`            alias for `equals false`
fn check_flag(
    flag: &str,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
) -> Result<()> {
    let raw = browser::eval_expression(
        ctx.session,
        "(() => { try { return JSON.parse(localStorage.getItem('devtools-flag-overrides') || '{}'); } catch { return {}; } })()",
    )?;
    // agent-browser eval double-encodes; the IIFE returns a real object
    // so we get one level of JSON.
    let parsed: Json =
        serde_json::from_str(raw.trim()).unwrap_or(Json::Object(serde_json::Map::new()));
    let entry = parsed.get(flag).cloned();
    match predicate {
        Predicate::Exists => {
            if entry.is_none() {
                bail!("expected flag {flag:?} to exist in localStorage devtools-flag-overrides");
            }
            Ok(())
        }
        Predicate::NotExists => {
            if entry.is_some() {
                bail!("expected flag {flag:?} to be absent, got {:?}", entry);
            }
            Ok(())
        }
        Predicate::IsVisible | Predicate::IsHidden | Predicate::Equals => {
            let want_true = match predicate {
                Predicate::IsVisible => true,
                Predicate::IsHidden => false,
                Predicate::Equals => match expected {
                    Some(Json::Bool(b)) => *b,
                    other => bail!(
                        "flag claim with predicate equals requires boolean value, got {other:?}"
                    ),
                },
                _ => unreachable!(),
            };
            let actual = entry
                .as_ref()
                .and_then(|v| v.as_bool())
                .ok_or_else(|| {
                    anyhow!(
                        "flag {flag:?} not present (or non-boolean) in localStorage devtools-flag-overrides"
                    )
                })?;
            if actual != want_true {
                bail!("expected flag {flag:?} = {want_true}, got {actual}");
            }
            Ok(())
        }
        other => bail!("flag subject does not support predicate '{other:?}'"),
    }
}

// ---------- storage / cookie ----------

fn check_storage(
    storage: &crate::scenario::StorageSubject,
    path: Option<&str>,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
) -> Result<()> {
    let (key, store) = match storage {
        crate::scenario::StorageSubject::Key(k) => (k.clone(), "local"),
        crate::scenario::StorageSubject::Matcher(m) => {
            let scope_name = m.scope.as_deref().unwrap_or("local");
            if !matches!(scope_name, "local" | "session") {
                bail!("storage scope must be \"local\" or \"session\", got {scope_name:?}");
            }
            (m.key.clone(), scope_name)
        }
    };
    let key = substitute_scenario_vars(&key, scope);
    let expr = format!(
        "(() => {{ try {{ return {}.getItem({}); }} catch {{ return null; }} }})()",
        if store == "session" {
            "sessionStorage"
        } else {
            "localStorage"
        },
        serde_json::to_string(&key)?,
    );
    let raw = browser::eval_expression(ctx.session, &expr)?;
    let raw = raw.trim();
    let actual: Json = if raw.is_empty() || raw == "null" {
        Json::Null
    } else {
        // getItem returns a string; the eval layer JSON-encodes it once,
        // so decode first, then try parsing the stored text as JSON so
        // `path` can walk structured values.
        let decoded: String = serde_json::from_str(raw).unwrap_or_else(|_| raw.to_string());
        serde_json::from_str(&decoded).unwrap_or(Json::String(decoded))
    };
    let actual = match path {
        Some(p) if !actual.is_null() => select_json_path(&actual, p)?,
        _ => actual,
    };
    check_value(&actual, predicate, expected, scope)
        .map_err(|e| anyhow!("storage {store} key {key:?}: {e:#}"))
}

fn check_cookie(
    cookie: &str,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
) -> Result<()> {
    let name = substitute_scenario_vars(cookie, scope);
    // `agent-browser cookies` reads the CDP cookie jar — `document.cookie`
    // would miss HttpOnly cookies like session/auth tokens.
    let jar = browser::cookies(ctx.session)?;
    let actual: Json = jar
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(n, _)| n.trim() == name)
        .map(|(_, v)| Json::String(v.trim().to_string()))
        .unwrap_or(Json::Null);
    check_value(&actual, predicate, expected, scope).map_err(|e| anyhow!("cookie {name:?}: {e:#}"))
}

/// `{"indexeddb": {"db","store","key"?}}` — read an IndexedDB record in the
/// live page. The db is probed via `indexedDB.databases()` first: opening a
/// name that doesn't exist would create it, which would turn `notExists`
/// into a false pass (and leave junk behind).
fn check_indexeddb(
    matcher: &crate::scenario::IndexedDbMatcher,
    path: Option<&str>,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
) -> Result<()> {
    let db = substitute_scenario_vars(&matcher.db, scope);
    let store = substitute_scenario_vars(&matcher.store, scope);
    let key = matcher
        .key
        .as_ref()
        .map(|k| substitute_scenario_vars(k, scope));
    let key_js = match &key {
        Some(k) => serde_json::to_string(k)?,
        None => "null".into(),
    };
    let expr = format!(
        "(async () => {{ \
           const DB = {db}, STORE = {store}, KEY = {key}; \
           if (!indexedDB.databases) return '__aq_idb_unsupported__'; \
           const names = (await indexedDB.databases()).map(d => d.name); \
           if (!names.includes(DB)) return null; \
           const db = await new Promise((res, rej) => {{ \
             const r = indexedDB.open(DB); \
             r.onsuccess = () => res(r.result); r.onerror = () => rej(r.error); \
           }}); \
           try {{ \
             if (!db.objectStoreNames.contains(STORE)) return null; \
             if (KEY === null) return '__aq_idb_store__'; \
             const v = await new Promise((res, rej) => {{ \
               const rq = db.transaction(STORE).objectStore(STORE).get(KEY); \
               rq.onsuccess = () => res(rq.result === undefined ? null : rq.result); \
               rq.onerror = () => rej(rq.error); \
             }}); \
             return v; \
           }} finally {{ db.close(); }} \
         }})()",
        db = serde_json::to_string(&db)?,
        store = serde_json::to_string(&store)?,
        key = key_js,
    );
    let raw = browser::eval_expression(ctx.session, &expr)?;
    let raw = raw.trim();
    if raw == "\"__aq_idb_unsupported__\"" {
        bail!("indexeddb claim requires IndexedDB.databases() support in the page");
    }
    let actual: Json = if raw.is_empty() || raw == "null" {
        Json::Null
    } else {
        let decoded: String = serde_json::from_str(raw).unwrap_or_else(|_| raw.to_string());
        if decoded == "__aq_idb_store__" {
            Json::Bool(true)
        } else {
            serde_json::from_str(&decoded).unwrap_or(Json::String(decoded))
        }
    };
    let actual = match path {
        Some(p) if !actual.is_null() => select_json_path(&actual, p)?,
        _ => actual,
    };
    let label = match &key {
        Some(k) => format!("indexeddb {db}.{store}[{k:?}]"),
        None => format!("indexeddb {db}.{store}"),
    };
    check_value(&actual, predicate, expected, scope).map_err(|e| anyhow!("{label}: {e:#}"))
}

/// `{"clipboard": true}` — read the clipboard's text via
/// `navigator.clipboard.readText()`. That API needs `clipboard-read`
/// granted for the page's origin plus a focused document —
/// `cdp::ensure_clipboard_access` does both over the pooled connection
/// (headless pages report no focus otherwise). An empty clipboard reads
/// as `""` and maps to Null, so `exists` means "holds text".
fn check_clipboard(
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
) -> Result<()> {
    crate::cdp::ensure_clipboard_access(ctx.session, false)?;
    let raw = browser::eval_expression(
        ctx.session,
        "(async () => { if (!navigator.clipboard) return '__aq_clip_unsupported__'; \
         try { return await navigator.clipboard.readText(); } catch (e) { return '__aq_clip_err__' + e.name; } })()",
    )?;
    let raw = raw.trim();
    if raw == "\"__aq_clip_unsupported__\"" {
        bail!("clipboard: navigator.clipboard is not available (insecure context?)");
    }
    if let Some(err) = raw
        .strip_prefix("\"__aq_clip_err__")
        .and_then(|s| s.strip_suffix('"'))
    {
        bail!("clipboard readText rejected: {err}");
    }
    let actual: Json = if raw.is_empty() || raw == "null" || raw == "\"\"" {
        Json::Null
    } else {
        Json::String(serde_json::from_str(raw).unwrap_or_else(|_| raw.to_string()))
    };
    check_value(&actual, predicate, expected, scope).map_err(|e| anyhow!("clipboard: {e:#}"))
}

// ---------- timing ----------

/// `{"timing": "<stepId>"}` — read the run's `events.jsonl` and compare the
/// latest timed row (status pass/fail, `ms` present) for that step against
/// `value` via the numeric predicates. `exists`/`notExists` test whether a
/// timing row exists at all. Requires a run dir — bails outside replay.
fn check_timing(
    step_id: &str,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
) -> Result<()> {
    let step_id = substitute_scenario_vars(step_id, scope);
    let run_dir = ctx
        .run_dir
        .ok_or_else(|| anyhow!("timing claims require a replay run directory"))?;
    let body = std::fs::read_to_string(run_dir.join("events.jsonl")).unwrap_or_default();
    // Latest terminal row for the step wins — an earlier `running` row for
    // the same id carries no `ms`.
    let mut actual: Option<u64> = None;
    for line in body.lines() {
        let row: Json = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if row.get("id").and_then(|v| v.as_str()) != Some(step_id.as_str()) {
            continue;
        }
        let status = row.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status != "pass" && status != "fail" {
            continue;
        }
        if let Some(ms) = row.get("ms").and_then(|v| v.as_u64()) {
            actual = Some(ms);
        }
    }
    let n = match (actual, predicate) {
        (None, Predicate::NotExists | Predicate::IsHidden) => return Ok(()),
        (None, _) => bail!("timing {step_id:?}: no timed step row in this run's events"),
        (Some(n), Predicate::Exists | Predicate::IsVisible) => {
            let _ = n;
            return Ok(());
        }
        (Some(_), Predicate::NotExists | Predicate::IsHidden) => {
            bail!("timing {step_id:?}: expected no timing row, found one")
        }
        (Some(n), _) => n,
    };
    let need = expected.and_then(|v| v.as_u64()).ok_or_else(|| {
        anyhow!("timing claim with predicate '{predicate:?}' requires a numeric 'value'")
    })?;
    let ok = match predicate {
        Predicate::Equals | Predicate::CountEquals => n == need,
        Predicate::Gt => n > need,
        Predicate::Gte => n >= need,
        Predicate::Lt => n < need,
        Predicate::Lte => n <= need,
        other => bail!("timing claim does not support predicate '{other:?}'"),
    };
    if !ok {
        bail!("timing {step_id:?}: {n}ms failed predicate {predicate:?} {need}ms")
    }
    Ok(())
}

// ---------- file ----------

/// `{"shot": "<stepId>"}` claims — pixel-compare the current run's
/// screenshot for that step against `<scenario>/baselines/<stepId>.png`.
/// Only `matches` is supported: pass when the differing-pixel fraction is
/// within `tolerance.pixels` (default 1%). On a miss the delta map writes
/// to `<run>/shots-diff/<stepId>.diff.png` before the claim fails, so the
/// diff is inspectable in the run artifacts. A missing baseline bails with
/// the `shot-accept` mint command rather than silently passing.
fn check_shot(
    shot: &str,
    clip: Option<&Locator>,
    predicate: &Predicate,
    tolerance: Option<&std::collections::BTreeMap<String, Json>>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
) -> Result<()> {
    if *predicate != Predicate::Matches {
        bail!("shot subject only supports predicate 'matches', got '{predicate:?}'");
    }
    let run_dir = ctx
        .run_dir
        .ok_or_else(|| anyhow!("shot claim needs a replay run (not available via run-step)"))?;
    let baseline = ctx
        .scenario_dir
        .join("baselines")
        .join(format!("{shot}.png"));
    if !baseline.is_file() {
        bail!(
            "no baseline for shot '{shot}' at {} — mint one with: agent-qa shot-accept <sid> --steps {shot}",
            baseline.display()
        );
    }
    let current = run_dir.join("screenshots").join(format!("{shot}.png"));
    if !current.is_file() {
        bail!(
            "no screenshot for step '{shot}' in this run ({}) — check the step id and that sidecars are on",
            current.display()
        );
    }
    // `tolerance.preset` = strict | balanced | relaxed — named presets
    // over (pixels, aa) so a claim can say how sensitive the diff is
    // without picking numbers. `tolerance.pixels`/`tolerance.aa`
    // override the preset's value individually.
    let (mut tol, mut aa) = match tolerance
        .and_then(|t| t.get("preset"))
        .and_then(|v| v.as_str())
    {
        None | Some("balanced") => (0.01, 32),
        Some("strict") => (0.0, 16),
        Some("relaxed") => (0.05, 64),
        Some(other) => bail!(
            "shot '{shot}' tolerance.preset {other:?} — expected strict, balanced, or relaxed"
        ),
    };
    if let Some(v) = tolerance
        .and_then(|t| t.get("pixels"))
        .and_then(|v| v.as_f64())
    {
        tol = v;
    }
    if let Some(v) = tolerance.and_then(|t| t.get("aa")).and_then(|v| v.as_i64()) {
        aa = v as i16;
    }
    let mut a = crate::compare::screenshots::decode_png(&baseline)?;
    let mut b = crate::compare::screenshots::decode_png(&current)?;
    // Remember the clip origin (image px) so a shot-miss RCA can map the
    // cropped diff region back to page CSS coordinates.
    let mut clip_origin_img: (u32, u32) = (0, 0);
    let full_img_w = b.width();
    if let Some(loc) = clip {
        // Crop BOTH images to the element's live box (CSS px → image px via
        // the screenshot's device-pixel scale). The rect is read now, at
        // claim time — keep the viewport pinned so record ≈ replay rects.
        let rect = clip_rect(ctx.session, loc, scope, b.width())?;
        clip_origin_img = (rect.0, rect.1);
        a = crop_to_rect(&a, rect).with_context(|| {
            format!(
                "shot '{shot}' clip rect {:?} outside baseline {:?}",
                rect,
                a.dimensions()
            )
        })?;
        b = crop_to_rect(&b, rect).with_context(|| {
            format!(
                "shot '{shot}' clip rect {:?} outside current {:?}",
                rect,
                b.dimensions()
            )
        })?;
    }
    if a.dimensions() != b.dimensions() {
        // No pixel compare across mismatched bounds — but still leave a
        // visual diff for reviewers: pad both to the larger bounds with a
        // loud fill and diff that, so the size change shows up in red.
        let max_w = a.width().max(b.width());
        let max_h = a.height().max(b.height());
        let pad = |img: &image::RgbaImage| {
            let mut out =
                image::RgbaImage::from_pixel(max_w, max_h, image::Rgba([255, 0, 255, 255]));
            image::imageops::overlay(&mut out, img, 0, 0);
            out
        };
        let a_pad = pad(&a);
        let b_pad = pad(&b);
        let (_, diff_img) = crate::compare::screenshots::pixel_diff_with(&a_pad, &b_pad, aa);
        let diff_dir = run_dir.join("shots-diff");
        std::fs::create_dir_all(&diff_dir).ok();
        let diff_path = diff_dir.join(format!("{shot}.diff.png"));
        let _ = diff_img.save(&diff_path);
        bail!(
            "shot '{shot}' changed size — baseline {:?} vs current {:?}; re-mint with shot-accept if intentional — diff at {}",
            a.dimensions(),
            b.dimensions(),
            diff_path.display()
        );
    }
    let (frac, diff_img) = crate::compare::screenshots::pixel_diff_with(&a, &b, aa);
    if frac <= tol {
        return Ok(());
    }
    let diff_dir = run_dir.join("shots-diff");
    std::fs::create_dir_all(&diff_dir).ok();
    let diff_path = diff_dir.join(format!("{shot}.diff.png"));
    let _ = diff_img.save(&diff_path);
    let regions = crate::compare::screenshots::diff_regions(&diff_img);
    let region_note = if regions.is_empty() {
        String::new()
    } else {
        let (rx, ry, rw, rh, _) = regions[0];
        format!(
            " in {} region(s), largest {}x{} at {},{}",
            regions.len(),
            rw,
            rh,
            rx,
            ry
        )
    };
    // RCA-lite: map the diff's pixel bounds back to CSS space and list the
    // elements intersecting it — "which element moved" without a human
    // squinting at the diff map. Best-effort: an eval failure never masks
    // the underlying miss.
    let rca_note = shot_rca(
        ctx.session,
        shot,
        run_dir,
        &diff_img,
        clip_origin_img,
        full_img_w,
        frac,
        tol,
    );
    bail!(
        "shot '{shot}' differs from baseline: {:.2}% pixels changed (tolerance {:.2}%){} — diff at {}{}",
        frac * 100.0,
        tol * 100.0,
        region_note,
        diff_path.display(),
        rca_note
    )
}

/// Shot-miss RCA-lite: find the diff map's red-pixel bounds, convert them
/// to page CSS coordinates (accounting for a clip origin and the
/// device-pixel scale), then ask the page which elements intersect that
/// region — the "what moved" answer a reviewer otherwise has to eyeball
/// off the diff. Writes `<run>/rca/<shot>.rca.json` with the suspect list
/// (top-12 by intersection area, DOM-path key + rect + text snippet) and
/// returns a short " — suspects: a, b, c" suffix for the claim error.
/// Returns "" when no suspects are found or the eval fails.
#[allow(clippy::too_many_arguments)]
fn shot_rca(
    session: &str,
    shot: &str,
    run_dir: &Path,
    diff_img: &image::RgbaImage,
    clip_origin_img: (u32, u32),
    full_img_w: u32,
    frac: f64,
    tol: f64,
) -> String {
    // Bounding box of the diff map's red pixels (image px).
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let (w, h) = diff_img.dimensions();
    for y in 0..h {
        for x in 0..w {
            let p = diff_img.get_pixel(x, y);
            if p[0] == 255 && p[1] == 0 && p[2] == 0 && p[3] == 255 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if min_x > max_x {
        return String::new();
    }
    let params = serde_json::json!({
        "x": clip_origin_img.0 + min_x,
        "y": clip_origin_img.1 + min_y,
        "w": max_x - min_x + 1,
        "h": max_y - min_y + 1,
        "iw": full_img_w,
    });
    let js = format!(
        r#"(() => {{
  const P = {params};
  const scale = window.innerWidth > 0 ? P.iw / window.innerWidth : 1;
  const box = {{x: P.x / scale, y: P.y / scale, w: P.w / scale, h: P.h / scale}};
  const keyOf = (e) => {{
    const parts = [];
    for (let n = e; n && n !== document.documentElement && parts.length < 8; n = n.parentElement) {{
      let seg = n.tagName.toLowerCase();
      if (n.id) seg += '#' + CSS.escape(n.id);
      else if (typeof n.className === 'string' && n.className.trim()) {{
        seg += '.' + n.className.trim().split(/\s+/).filter(Boolean).slice(0, 3).map(c => CSS.escape(c)).join('.');
      }}
      const same = n.parentElement ? Array.from(n.parentElement.children).filter(c => c.tagName === n.tagName) : [n];
      if (same.length > 1) seg += ':nth(' + same.indexOf(n) + ')';
      parts.unshift(seg);
    }}
    return parts.join('>');
  }};
  const inter = (a, b) => {{
    const x = Math.max(a.x, b.x), y = Math.max(a.y, b.y);
    const r = Math.min(a.x + a.w, b.x + b.w), t = Math.min(a.y + a.h, b.y + b.h);
    return r > x && t > y ? (r - x) * (t - y) : 0;
  }};
  const out = [];
  for (const el of document.querySelectorAll('body *')) {{
    if (el.closest('[id^="__qa_"]')) continue;
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) continue;
    const area = inter(box, {{x: r.x, y: r.y, w: r.width, h: r.height}});
    if (area <= 0) continue;
    const text = (el.textContent || '').trim().slice(0, 80);
    out.push({{k: keyOf(el), area, x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height), text}});
  }}
  // Suspects ranked by how much of the element sits inside the diff
  // (most-specific first), then by absolute overlap.
  out.sort((a, b) => (b.area / (b.w * b.h)) - (a.area / (a.w * a.h)) || b.area - a.area);
  const suspects = out.slice(0, 12).map(s => ({{...s, area: Math.round(s.area)}}));
  return JSON.stringify({{diffBoxCss: {{x: Math.round(box.x), y: Math.round(box.y), w: Math.round(box.w), h: Math.round(box.h)}}, suspects}});
}})()"#
    );
    let suspects: Vec<Json> = match browser::eval_expression(session, &js) {
        Ok(raw) => {
            let text: String =
                serde_json::from_str(raw.trim()).unwrap_or_else(|_| raw.trim().to_string());
            match serde_json::from_str::<Json>(&text) {
                Ok(doc) => {
                    let report = serde_json::json!({
                        "shot": shot,
                        "pixelFrac": frac,
                        "tolerance": tol,
                        "diffBoxCss": doc.get("diffBoxCss").cloned().unwrap_or(Json::Null),
                        "suspects": doc.get("suspects").cloned().unwrap_or(Json::Null),
                    });
                    let rca_dir = run_dir.join("rca");
                    std::fs::create_dir_all(&rca_dir).ok();
                    let rca_path = rca_dir.join(format!("{shot}.rca.json"));
                    let _ = std::fs::write(
                        &rca_path,
                        serde_json::to_string_pretty(&report).unwrap_or_default(),
                    );
                    doc.get("suspects")
                        .and_then(|s| s.as_array())
                        .cloned()
                        .unwrap_or_default()
                }
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    };
    if suspects.is_empty() {
        return String::new();
    }
    let top: Vec<String> = suspects
        .iter()
        .filter_map(|s| s.get("k").and_then(|k| k.as_str()).map(str::to_string))
        .take(3)
        .collect();
    format!(" — suspects: {}", top.join(", "))
}

/// `{"domshot": "<stepId>"}` — compare the run's ARIA snapshot for that
/// do-step (`<run>/snapshots/<stepId>.txt`) against `<scenario>/baselines/
/// <stepId>.snap.txt`. Only `matches` is supported: pass when the two
/// texts are identical after normalization — `@eN` refs collapse to `@e`
/// (ref numbering shifts across runs) and lines matching a `skip` regex
/// are dropped from both sides. On a miss a unified diff writes to
/// `<run>/domshots-diff/<stepId>.diff.txt` before the claim fails, so the
/// structural drift is inspectable in the run artifacts and pastes
/// cleanly into an LLM prompt. A missing baseline bails with the
/// `domshot-accept` mint command.
fn check_domshot(
    domshot: &str,
    skip: &[String],
    predicate: &Predicate,
    ctx: &CheckContext,
) -> Result<()> {
    if *predicate != Predicate::Matches {
        bail!("domshot subject only supports predicate 'matches', got '{predicate:?}'");
    }
    let run_dir = ctx
        .run_dir
        .ok_or_else(|| anyhow!("domshot claim needs a replay run (not available via run-step)"))?;
    let baseline = ctx
        .scenario_dir
        .join("baselines")
        .join(format!("{domshot}.snap.txt"));
    if !baseline.is_file() {
        bail!(
            "no baseline for domshot '{domshot}' at {} — mint one with: agent-qa domshot-accept <sid> --steps {domshot}",
            baseline.display()
        );
    }
    let current = run_dir.join("snapshots").join(format!("{domshot}.txt"));
    if !current.is_file() {
        bail!(
            "no snapshot for step '{domshot}' in this run ({}) — check the step id and that sidecars are on",
            current.display()
        );
    }
    let a = normalize_snapshot(&std::fs::read_to_string(&baseline)?, skip)?;
    let b = normalize_snapshot(&std::fs::read_to_string(&current)?, skip)?;
    if a == b {
        return Ok(());
    }
    let diff = similar::TextDiff::from_lines(&a, &b)
        .unified_diff()
        .missing_newline_hint(true)
        .header(
            &format!("baselines/{domshot}.snap.txt"),
            &format!("snapshots/{domshot}.txt"),
        )
        .to_string();
    let changed = diff
        .lines()
        .filter(|l| l.starts_with('+') || l.starts_with('-'))
        .filter(|l| !l.starts_with("+++") && !l.starts_with("---"))
        .count();
    let diff_dir = run_dir.join("domshots-diff");
    std::fs::create_dir_all(&diff_dir).ok();
    let diff_path = diff_dir.join(format!("{domshot}.diff.txt"));
    let _ = std::fs::write(&diff_path, &diff);
    bail!(
        "domshot '{domshot}' differs from baseline ({changed} changed lines) — diff at {} ; re-mint with domshot-accept if the change is intentional",
        diff_path.display()
    )
}

/// `{"layout": "<stepId>"}` — compare the run's element-geometry capture
/// (`<run>/layouts/<stepId>.json`, a flat key→rect map of every visible
/// element's bounding box keyed by a stable DOM path) against
/// `baselines/<stepId>.layout.json`. Only `matches`: every baseline key
/// must exist in the run and each of x/y/w/h may drift at most
/// `tolerance.px` (default 4). Keys appearing/disappearing are added/
/// removed churn, tolerated up to `tolerance.added`/`tolerance.removed`
/// counts (default 0); `tolerance.moved` bounds how many keys may move
/// before failing (default 0). On a miss the moved/added/removed detail
/// lands at `<run>/layouts-diff/<stepId>.diff.json`. Mint baselines with
/// `layout-accept`.
fn check_layout(
    layout: &str,
    predicate: &Predicate,
    tolerance: Option<&BTreeMap<String, serde_json::Value>>,
    ctx: &CheckContext,
) -> Result<()> {
    if *predicate != Predicate::Matches {
        bail!("layout subject only supports predicate 'matches', got '{predicate:?}'");
    }
    let run_dir = ctx
        .run_dir
        .ok_or_else(|| anyhow!("layout claim needs a replay run (not available via run-step)"))?;
    let baseline = ctx
        .scenario_dir
        .join("baselines")
        .join(format!("{layout}.layout.json"));
    if !baseline.is_file() {
        bail!(
            "no baseline for layout '{layout}' at {} — mint one with: agent-qa layout-accept <sid> --steps {layout}",
            baseline.display()
        );
    }
    let current = run_dir.join("layouts").join(format!("{layout}.json"));
    if !current.is_file() {
        bail!(
            "no layout capture for step '{layout}' in this run ({}) — check the step id and that sidecars are on",
            current.display()
        );
    }

    #[derive(serde::Deserialize)]
    struct LayoutDoc {
        els: Vec<LayoutEl>,
    }
    #[derive(serde::Deserialize, serde::Serialize, Clone)]
    struct LayoutEl {
        k: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    let read_doc = |p: &std::path::Path| -> Result<BTreeMap<String, LayoutEl>> {
        let doc: LayoutDoc = serde_json::from_slice(&std::fs::read(p)?)
            .with_context(|| format!("layout capture {} is not valid JSON", p.display()))?;
        Ok(doc.els.into_iter().map(|e| (e.k.clone(), e)).collect())
    };
    let base_map = read_doc(&baseline)?;
    let run_map = read_doc(&current)?;

    let tol_px = |k: &str, d: f64| -> f64 {
        tolerance
            .and_then(|t| t.get(k))
            .and_then(|v| v.as_f64())
            .unwrap_or(d)
    };
    let px = tol_px("px", 4.0);
    let allow_added = tol_px("added", 0.0) as usize;
    let allow_removed = tol_px("removed", 0.0) as usize;
    let allow_moved = tol_px("moved", 0.0) as usize;

    let mut moved: Vec<serde_json::Value> = Vec::new();
    let mut added: Vec<String> = Vec::new();
    let mut removed: Vec<String> = Vec::new();
    for (k, a) in &base_map {
        match run_map.get(k) {
            None => removed.push(k.clone()),
            Some(b) => {
                let d = [
                    (b.x - a.x).abs(),
                    (b.y - a.y).abs(),
                    (b.w - a.w).abs(),
                    (b.h - a.h).abs(),
                ];
                if d.iter().any(|v| *v > px) {
                    moved.push(serde_json::json!({
                        "key": k,
                        "baseline": {"x": a.x, "y": a.y, "w": a.w, "h": a.h},
                        "current": {"x": b.x, "y": b.y, "w": b.w, "h": b.h},
                        "delta": d,
                    }));
                }
            }
        }
    }
    for k in run_map.keys() {
        if !base_map.contains_key(k) {
            added.push(k.clone());
        }
    }
    if moved.len() <= allow_moved && added.len() <= allow_added && removed.len() <= allow_removed {
        return Ok(());
    }
    let diff_dir = run_dir.join("layouts-diff");
    std::fs::create_dir_all(&diff_dir).ok();
    let diff_path = diff_dir.join(format!("{layout}.diff.json"));
    let _ = std::fs::write(
        &diff_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "px": px,
            "moved": moved,
            "added": added,
            "removed": removed,
        }))?,
    );
    bail!(
        "layout '{layout}' differs from baseline ({} moved, {} added, {} removed; px={px}) — diff at {} ; re-mint with layout-accept if the change is intentional",
        moved.len(),
        added.len(),
        removed.len(),
        diff_path.display()
    )
}

/// Normalize an ARIA snapshot for comparison: drop lines matching any
/// `skip` regex, strip element refs (numbering shifts across runs —
/// both the `ref=eN` spelling the daemon emits and the `ref=@eN` /
/// standalone `@eN` spellings in older captures), and trim trailing
/// whitespace.
fn normalize_snapshot(text: &str, skip: &[String]) -> Result<String> {
    let ref_re = regex::Regex::new(r"ref=@{0,2}e\d+")?;
    let at_re = regex::Regex::new(r"@{1,2}e\d+")?;
    let skip_res: Vec<regex::Regex> = skip
        .iter()
        .map(|p| regex::Regex::new(p).map_err(|e| anyhow!("domshot skip pattern {p:?}: {e}")))
        .collect::<Result<_>>()?;
    let mut out = String::new();
    for line in text.lines() {
        let line = line.trim_end();
        if skip_res.iter().any(|r| r.is_match(line)) {
            continue;
        }
        let line = ref_re.replace_all(line, "ref=e");
        out.push_str(&at_re.replace_all(&line, "@e"));
        out.push('\n');
    }
    Ok(out)
}

/// A pixel-space rectangle `(x, y, w, h)` in the screenshot's own
/// coordinate system (CSS px × device scale).
type ImgRect = (u32, u32, u32, u32);

/// Resolve the clip locator to its element's box, scaled from CSS px into
/// the screenshot's pixel space (`css_width` = `window.innerWidth`, so the
/// scale factor is `img.width / innerWidth`). Reads the rect live — a shot
/// claim pairs with the step that just ran, so the element should still be
/// on screen.
fn clip_rect(
    session: &str,
    loc: &Locator,
    scope: &mut ValueScope,
    img_width: u32,
) -> Result<ImgRect> {
    let selector = match loc {
        Locator::Raw(raw) => {
            let v = substitute_scenario_vars(&raw.raw.value, scope);
            match &raw.raw.kind {
                RawLocatorKind::Css => v,
                RawLocatorKind::TestId => {
                    format!("[data-testid=\"{}\"]", v.replace('"', "\\\""))
                }
                other => bail!("shot clip does not support raw locator kind {other:?}"),
            }
        }
        _ => bail!("shot clip currently requires a raw css or testId locator"),
    };
    let expr = format!(
        "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('clip selector not found: ' + {q}); const r = el.getBoundingClientRect(); return JSON.stringify({{x: r.x, y: r.y, w: r.width, h: r.height, iw: window.innerWidth}}); }})()",
        q = serde_json::to_string(&selector).expect("string serializes")
    );
    let raw = browser::eval_expression(session, &expr)?;
    let v: Json = serde_json::from_str(&decode_json_string(raw.trim()))
        .with_context(|| format!("clip rect eval returned {raw}"))?;
    let f = |k: &str| -> Result<f64> {
        v.get(k)
            .and_then(|n| n.as_f64())
            .ok_or_else(|| anyhow!("clip rect eval missing '{k}' in {v}"))
    };
    let iw = f("iw")?;
    let scale = if iw > 0.0 { img_width as f64 / iw } else { 1.0 };
    Ok((
        (f("x")? * scale).round().max(0.0) as u32,
        (f("y")? * scale).round().max(0.0) as u32,
        (f("w")? * scale).round().max(0.0) as u32,
        (f("h")? * scale).round().max(0.0) as u32,
    ))
}

/// Crop an image to `(x, y, w, h)` in its own pixel space, clamped to its
/// bounds. Bails when the requested box is empty or fully outside.
fn crop_to_rect(img: &image::RgbaImage, (x, y, w, h): ImgRect) -> Result<image::RgbaImage> {
    if w == 0 || h == 0 {
        bail!("clip rect is empty ({x},{y} {w}x{h}) — element has zero size or is offscreen");
    }
    let (iw, ih) = img.dimensions();
    let x = x.min(iw.saturating_sub(1));
    let y = y.min(ih.saturating_sub(1));
    let w = w.min(iw - x);
    let h = h.min(ih - y);
    Ok(image::imageops::crop_imm(img, x, y, w, h).to_image())
}

/// `{"file": "<name-or-path>"}` claims: poll the filesystem so a check step
/// right after `do/download` sees the file as soon as the browser flushes it.
fn check_file(
    file: &str,
    attribute: Option<&str>,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    // `attribute: "name"` (default) — string predicates match the file name;
    // `"content"` — they match the file's UTF-8 text (reads are size-capped).
    match attribute {
        None | Some("name") | Some("content") => {}
        Some(other) => bail!("file subject does not support attribute '{other}'"),
    }
    let raw = substitute_scenario_vars(file, scope);
    let path = {
        let p = PathBuf::from(&raw);
        if p.is_absolute() {
            p
        } else {
            ctx.scenario_dir.join(&p)
        }
    };
    let deadline = Instant::now() + timeout;
    loop {
        let meta = std::fs::metadata(&path).ok();
        let done = match predicate {
            Predicate::Exists | Predicate::IsVisible => meta.map(|m| m.is_file()).unwrap_or(false),
            Predicate::NotExists | Predicate::IsHidden => meta.is_none(),
            Predicate::Gt | Predicate::Gte | Predicate::Lt | Predicate::Lte => {
                let need = expected
                    .ok_or_else(|| {
                        anyhow!("file size claim with predicate '{predicate:?}' requires 'value'")
                    })?
                    .as_u64()
                    .ok_or_else(|| anyhow!("file size claim requires a numeric 'value' (bytes)"))?;
                match meta {
                    Some(m) => match predicate {
                        Predicate::Gt => m.len() > need,
                        Predicate::Gte => m.len() >= need,
                        Predicate::Lt => m.len() < need,
                        _ => m.len() <= need,
                    },
                    None => false,
                }
            }
            // String predicates compare the file name once it exists — or
            // the file's UTF-8 text when `attribute` is `"content"`.
            other @ (Predicate::Equals
            | Predicate::Contains
            | Predicate::Matches
            | Predicate::StartsWith
            | Predicate::EndsWith) => match meta {
                Some(m) if m.is_file() => {
                    let need = expected.ok_or_else(|| {
                        anyhow!("file claim with predicate '{other:?}' requires 'value'")
                    })?;
                    let need = substitute_scenario_vars(&value_to_string(need), scope);
                    let actual = if attribute == Some("content") {
                        const CONTENT_CAP: u64 = 1024 * 1024;
                        if m.len() > CONTENT_CAP {
                            bail!(
                                "file content claim on {raw:?}: {} bytes exceeds the 1 MiB content cap",
                                m.len()
                            );
                        }
                        String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default())
                            .into_owned()
                    } else {
                        path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    };
                    compare_string(other, &actual, &need).is_ok()
                }
                _ => false,
            },
            other => bail!("file subject does not support predicate '{other:?}'"),
        };
        if done {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let exists = path.is_file();
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            bail!("file claim timed out: {raw:?} (exists={exists}, bytes={size})");
        }
        thread::sleep(POLL_INTERVAL);
    }
}

// ---------- network ----------

/// Check captured network traffic (`agent-browser network requests`).
///
/// Matcher fields AND together: `urlMatches` is a regex on the request URL,
/// `operationName` is a substring match on the URL (GraphQL-style
/// `?operationName=`/path segments), `method` is the HTTP verb.
///
/// `ofKind`:
///   `fired` (default)     `exists`/`isVisible` pass once ≥1 request matched;
///                         `notExists`/`isHidden` require zero matches.
///   `status`              compares the latest matching request's HTTP status —
///                         string predicates on "200", `gt`/`gte`/`lt`/`lte`
///                         numerically.
///   `responseJsonPath`    fetches the latest match's response body via
///                         `network request <id>` and evaluates `path` through
///                         the data-subject JSON path + predicate machinery.
///
/// All forms poll until `timeout` (a request may land after the check step
/// starts); on timeout the error carries the last observed state.
#[allow(clippy::too_many_arguments)]
fn check_network(
    matcher: &NetworkMatcher,
    of_kind: Option<&NetworkClaimKind>,
    path: Option<&str>,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    let url_re = match &matcher.url_matches {
        Some(p) => {
            let p = substitute_scenario_vars(p, scope);
            Some(
                Regex::new(&p)
                    .map_err(|e| anyhow!("network urlMatches is not a valid regex {p:?}: {e}"))?,
            )
        }
        None => None,
    };
    let op_name = matcher
        .operation_name
        .as_deref()
        .map(|s| substitute_scenario_vars(s, scope));
    let kind = match of_kind {
        None | Some(NetworkClaimKind::Fired) => NetworkClaimKind::Fired,
        Some(NetworkClaimKind::Status) => NetworkClaimKind::Status,
        Some(NetworkClaimKind::ResponseJsonPath) => NetworkClaimKind::ResponseJsonPath,
    };
    // Hard validation — these never resolve by polling, bail immediately.
    match (&kind, predicate) {
        (
            NetworkClaimKind::Fired,
            Predicate::Exists | Predicate::IsVisible | Predicate::NotExists | Predicate::IsHidden,
        ) => {}
        (
            NetworkClaimKind::Status,
            Predicate::Exists
            | Predicate::IsVisible
            | Predicate::Equals
            | Predicate::Contains
            | Predicate::Matches
            | Predicate::StartsWith
            | Predicate::EndsWith
            | Predicate::Gt
            | Predicate::Gte
            | Predicate::Lt
            | Predicate::Lte,
        ) => {}
        (NetworkClaimKind::ResponseJsonPath, _) => {
            if path.is_none() {
                bail!("network responseJsonPath claim requires 'path'");
            }
        }
        (kind, other) => bail!("network {kind:?} claim does not support predicate '{other:?}'"),
    }

    let mut fetch_body = |id: &str| -> Result<Json> {
        let detail = browser::network_request(ctx.session, id)
            .map_err(|e| anyhow!("network request {id}: {e}"))?;
        let body = detail
            .get("responseBody")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        serde_json::from_str(body).map_err(|e| anyhow!("response body of {id} is not JSON ({e})"))
    };
    let post_filter = matcher
        .post_data_contains
        .as_deref()
        .map(|s| substitute_scenario_vars(s, scope));
    let ws_filter = matcher
        .ws_payload_contains
        .as_deref()
        .map(|s| substitute_scenario_vars(s, scope));

    let deadline = Instant::now() + timeout;
    let mut pending: String;
    loop {
        match browser::network_requests(ctx.session) {
            Ok(reqs) => {
                let matches = filter_by_ws_payload(
                    filter_by_post(
                        matching_requests(
                            url_re.as_ref(),
                            op_name.as_deref(),
                            matcher.method.as_ref(),
                            &reqs,
                        ),
                        post_filter.as_deref(),
                        ctx.session,
                    ),
                    ws_filter.as_deref(),
                );
                match evaluate_network(
                    &kind,
                    &matches,
                    path,
                    predicate,
                    expected,
                    scope,
                    &mut fetch_body,
                )? {
                    NetEval::Pass => return Ok(()),
                    NetEval::Pending(reason) => pending = reason,
                }
            }
            Err(e) => pending = format!("network requests: {e}"),
        }
        if Instant::now() >= deadline {
            bail!("network claim timed out ({timeout:?}): {pending}");
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Keep candidates whose POST body carries the substring. The request
/// list has no bodies — `network request <id>` fetches per candidate,
/// so this runs only on the already-narrowed set.
fn filter_by_post<'r>(
    matches: Vec<&'r CapturedRequest>,
    needle: Option<&str>,
    session: &str,
) -> Vec<&'r CapturedRequest> {
    let Some(needle) = needle else {
        return matches;
    };
    matches
        .into_iter()
        .filter(|r| {
            if let Some(pd) = r.post_data.as_deref() {
                return pd.contains(needle);
            }
            browser::network_request(session, &r.request_id)
                .ok()
                .and_then(|d| {
                    d.get("postData")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .map(|pd| pd.contains(needle))
                .unwrap_or(false)
        })
        .collect()
}

/// Keep candidates carrying a WebSocket frame whose payload contains the
/// needle — only `cdpws-*` entries have `ws_frames`, so this narrows to
/// sockets.
fn filter_by_ws_payload<'r>(
    matches: Vec<&'r CapturedRequest>,
    needle: Option<&str>,
) -> Vec<&'r CapturedRequest> {
    let Some(needle) = needle else {
        return matches;
    };
    matches
        .into_iter()
        .filter(|r| {
            r.ws_frames.iter().any(|f| {
                f.get("payload")
                    .and_then(|p| p.as_str())
                    .map(|p| p.contains(needle))
                    .unwrap_or(false)
            })
        })
        .collect()
}

enum NetEval {
    Pass,
    Pending(String),
}

fn http_method_str(m: &HttpMethod) -> &'static str {
    match m {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Head => "HEAD",
    }
}

/// Requests matching every populated matcher field, in capture order.
fn matching_requests<'a>(
    url_re: Option<&Regex>,
    op_name: Option<&str>,
    method: Option<&HttpMethod>,
    reqs: &'a [CapturedRequest],
) -> Vec<&'a CapturedRequest> {
    reqs.iter()
        .filter(|r| {
            url_re.map(|re| re.is_match(&r.url)).unwrap_or(true)
                && op_name.map(|op| r.url.contains(op)).unwrap_or(true)
                && method
                    .map(|m| r.method.eq_ignore_ascii_case(http_method_str(m)))
                    .unwrap_or(true)
        })
        .collect()
}

fn evaluate_network(
    kind: &NetworkClaimKind,
    matches: &[&CapturedRequest],
    path: Option<&str>,
    predicate: &Predicate,
    expected: Option<&Json>,
    scope: &mut ValueScope,
    fetch_body: &mut dyn FnMut(&str) -> Result<Json>,
) -> Result<NetEval> {
    match kind {
        NetworkClaimKind::Fired => match predicate {
            Predicate::Exists | Predicate::IsVisible => Ok(if matches.is_empty() {
                NetEval::Pending("no request matched".to_string())
            } else {
                NetEval::Pass
            }),
            Predicate::NotExists | Predicate::IsHidden => Ok(if matches.is_empty() {
                NetEval::Pass
            } else {
                NetEval::Pending(format!("{} request(s) still match", matches.len()))
            }),
            other => bail!("network fired claim does not support predicate '{other:?}'"),
        },
        NetworkClaimKind::Status => {
            let Some(latest) = matches.last() else {
                return Ok(NetEval::Pending("no request matched".to_string()));
            };
            let Some(status) = latest.status else {
                return Ok(NetEval::Pending(format!(
                    "no response yet for {}",
                    latest.url
                )));
            };
            match predicate {
                Predicate::Exists | Predicate::IsVisible => Ok(NetEval::Pass),
                Predicate::Gt | Predicate::Gte | Predicate::Lt | Predicate::Lte => {
                    let need = expected
                        .ok_or_else(|| {
                            anyhow!("network status claim with '{predicate:?}' requires 'value'")
                        })?
                        .as_i64()
                        .ok_or_else(|| {
                            anyhow!("network status claim requires a numeric 'value'")
                        })?;
                    let ok = match predicate {
                        Predicate::Gt => status > need,
                        Predicate::Gte => status >= need,
                        Predicate::Lt => status < need,
                        _ => status <= need,
                    };
                    Ok(if ok {
                        NetEval::Pass
                    } else {
                        NetEval::Pending(format!(
                            "status {status} does not satisfy {predicate:?} {need}"
                        ))
                    })
                }
                other => {
                    let need = expected.ok_or_else(|| {
                        anyhow!("network status claim with '{other:?}' requires 'value'")
                    })?;
                    let need = substitute_scenario_vars(&value_to_string(need), scope);
                    match compare_string(other, &status.to_string(), &need) {
                        Ok(()) => Ok(NetEval::Pass),
                        Err(e) => Ok(NetEval::Pending(e.to_string())),
                    }
                }
            }
        }
        NetworkClaimKind::ResponseJsonPath => {
            let Some(latest) = matches.last() else {
                return Ok(NetEval::Pending("no request matched".to_string()));
            };
            let path = path.expect("responseJsonPath validated");
            let body = match fetch_body(&latest.request_id) {
                Ok(b) => b,
                Err(e) => return Ok(NetEval::Pending(e.to_string())),
            };
            let actual = select_json_path(&body, path)?;
            match check_value(&actual, predicate, expected, scope) {
                Ok(()) => Ok(NetEval::Pass),
                Err(e) => Ok(NetEval::Pending(format!("{path}: {e}"))),
            }
        }
    }
}

// ---------- element ----------

fn check_element(
    loc: &Locator,
    attribute: Option<&str>,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    if let Some(attribute) = attribute {
        let expected = expected.ok_or_else(|| {
            anyhow!("element attribute claim with predicate '{predicate:?}' requires 'value'")
        })?;
        let need = substitute_scenario_vars(&value_to_string(expected), scope);
        let deadline = Instant::now() + timeout;
        let mut last_actual = String::new();
        let mut last_err: Option<anyhow::Error> = None;
        while Instant::now() < deadline {
            match read_element_attribute(ctx.session, loc, attribute, scope, ctx.scenario_dir) {
                Ok(actual) => {
                    last_actual = actual.clone();
                    match compare_string(predicate, &actual, &need) {
                        Ok(()) => return Ok(()),
                        Err(err) => last_err = Some(err),
                    }
                }
                Err(err) => last_err = Some(err),
            }
            thread::sleep(POLL_INTERVAL);
        }
        return Err(last_err.unwrap_or_else(|| {
            anyhow!("element {attribute} claim timed out; last seen value={last_actual:?}")
        }));
    }

    match predicate {
        Predicate::IsVisible | Predicate::Exists => poll_until(timeout, |_| {
            locator_resolves(ctx.session, loc, scope, ctx.scenario_dir)
        }),
        Predicate::IsHidden | Predicate::NotExists => {
            // Inverse: poll until the locator no longer resolves.
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                match locator_resolves(ctx.session, loc, scope, ctx.scenario_dir) {
                    Ok(()) => thread::sleep(POLL_INTERVAL),
                    Err(_) => return Ok(()),
                }
            }
            bail!("expected {predicate:?}, but element still present");
        }
        other => bail!("element subject does not yet support predicate '{other:?}'"),
    }
}

/// `{"element": <locator>, "ofKind": "count"}` — how many nodes the locator
/// resolves to right now. Numeric predicates compare the match count to
/// `value`; the count is polled (lists may render incrementally). Raw
/// css/testId locators only — the count is a `querySelectorAll`.
fn check_element_count(
    loc: &Locator,
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    let need = expected.and_then(|v| v.as_u64()).ok_or_else(|| {
        anyhow!("element count claim with predicate '{predicate:?}' requires a numeric 'value'")
    })?;
    match predicate {
        Predicate::Equals
        | Predicate::CountEquals
        | Predicate::Gt
        | Predicate::Gte
        | Predicate::Lt
        | Predicate::Lte => {}
        other => bail!("element count claim does not support predicate '{other:?}'"),
    }
    let deadline = Instant::now() + timeout;
    let mut last: Option<u64> = None;
    let mut last_err: Option<anyhow::Error> = None;
    while Instant::now() < deadline {
        match count_elements(ctx.session, loc, scope) {
            Ok(n) => {
                last = Some(n);
                let ok = match predicate {
                    Predicate::Equals | Predicate::CountEquals => n == need,
                    Predicate::Gt => n > need,
                    Predicate::Gte => n >= need,
                    Predicate::Lt => n < need,
                    Predicate::Lte => n <= need,
                    _ => unreachable!(),
                };
                if ok {
                    return Ok(());
                }
            }
            Err(e) => last_err = Some(e),
        }
        thread::sleep(POLL_INTERVAL);
    }
    match (last, last_err) {
        (Some(n), _) => {
            bail!("element count {n} failed predicate {predicate:?} {need}")
        }
        (None, Some(e)) => bail!("element count claim timed out; count never readable: {e}"),
        (None, None) => bail!("element count claim timed out"),
    }
}

fn count_elements(session: &str, loc: &Locator, scope: &mut ValueScope) -> Result<u64> {
    match loc {
        Locator::Raw(raw) => {
            let v = substitute_scenario_vars(&raw.raw.value, scope);
            let selector = match &raw.raw.kind {
                RawLocatorKind::Css => v,
                RawLocatorKind::TestId => {
                    format!("[data-testid=\"{}\"]", v.replace('"', "\\\""))
                }
                other => {
                    bail!("element count claims do not support raw locator kind {other:?}")
                }
            };
            let expr = format!(
                "(() => document.querySelectorAll({q}).length)()",
                q = serde_json::to_string(&selector).expect("string serializes")
            );
            let raw = browser::eval_expression(session, &expr)?;
            raw.trim()
                .parse::<u64>()
                .map_err(|e| anyhow!("element count eval returned unexpected value {raw:?}: {e}"))
        }
        _ => bail!("element count claims currently require a raw css or testId locator"),
    }
}

fn read_element_attribute(
    session: &str,
    loc: &Locator,
    attribute: &str,
    scope: &mut ValueScope,
    scenario_dir: &std::path::Path,
) -> Result<String> {
    match loc {
        Locator::Role(role) => {
            read_role_snapshot_attribute(session, role, attribute, scope, scenario_dir)
        }
        Locator::Raw(raw) => {
            let v = substitute_scenario_vars(&raw.raw.value, scope);
            let selector = match &raw.raw.kind {
                RawLocatorKind::Css => v,
                RawLocatorKind::TestId => format!("[data-testid=\"{}\"]", v.replace('"', "\\\"")),
                other => {
                    bail!("element attribute claims do not support raw locator kind {other:?}")
                }
            };
            let expr = element_attribute_read_expr(&selector, attribute);
            let raw = browser::eval_expression(session, &expr)?;
            Ok(decode_json_string(raw.trim()))
        }
    }
}

/// `text` reads textContent; value/checked/disabled/selected/readOnly/
/// required/tabIndex/maxLength/minLength read the live IDL property
/// (getAttribute would return the stale default, and boolean states
/// often have no attribute at all); `focused` is
/// `document.activeElement === el`; `style:<prop>` reads
/// getComputedStyle; any other name is a getAttribute read (missing
/// attributes read as the empty string). Attribute spellings are
/// case-insensitive — `readonly` resolves to the `readOnly` IDL
/// property, `Checked` to `checked`, etc.
fn element_attribute_read_expr(selector: &str, attribute: &str) -> String {
    let idl_prop = match attribute.to_ascii_lowercase().as_str() {
        "value" => Some("value"),
        "checked" => Some("checked"),
        "disabled" => Some("disabled"),
        "selected" => Some("selected"),
        "readonly" => Some("readOnly"),
        "required" => Some("required"),
        "tabindex" => Some("tabIndex"),
        "maxlength" => Some("maxLength"),
        "minlength" => Some("minLength"),
        _ => None,
    };
    if attribute == "text" {
        format!(
            "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); return (el.textContent || '').trim(); }})()",
            q = serde_json::to_string(selector).expect("string serializes")
        )
    } else if attribute == "focused" {
        format!(
            "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); return String(document.activeElement === el); }})()",
            q = serde_json::to_string(selector).expect("string serializes")
        )
    } else if let Some(prop) = attribute.strip_prefix("style:") {
        // style:<css-property> reads getComputedStyle — assertions
        // on rendered styles (color, display, ...) that neither
        // getAttribute nor IDL properties can express.
        format!(
            "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); return getComputedStyle(el).getPropertyValue({a}) || ''; }})()",
            q = serde_json::to_string(selector).expect("string serializes"),
            a = serde_json::to_string(prop).expect("string serializes")
        )
    } else if let Some(prop) = idl_prop {
        format!(
            "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); const v = el[{a}]; return v === undefined || v === null ? '' : String(v); }})()",
            q = serde_json::to_string(selector).expect("string serializes"),
            a = serde_json::to_string(prop).expect("string serializes")
        )
    } else {
        format!(
            "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); return el.getAttribute({a}) || ''; }})()",
            q = serde_json::to_string(selector).expect("string serializes"),
            a = serde_json::to_string(attribute).expect("string serializes")
        )
    }
}

/// Attribute reads on a `role`+`name` locator resolve through the page's
/// ARIA snapshot instead of a CSS selector: the line matching
/// `<role> "<name>"` carries the states the accessibility tree reports
/// (`[checked=true]`, `[disabled]`, `[required]`, `[expanded=false]`,
/// `level`, `pressed`, `selected`, `readonly`, `current`,
/// `orientation`, `valuemin`/`valuemax`/`valuenow`), and a `: <text>`
/// tail with the node's value. `value` reads that tail; `text` reads
/// the accessible name. Flag attributes absent from the line read as
/// `"false"`. Attributes the tree does not carry (`href`, `src`,
/// `data-*`, `focused`, `style:*`, ...) still need a raw css/testId
/// locator — the error says which set is readable here.
fn read_role_snapshot_attribute(
    session: &str,
    role: &crate::scenario::LocatorRole,
    attribute: &str,
    scope: &mut ValueScope,
    scenario_dir: &std::path::Path,
) -> Result<String> {
    if role.scope.is_some() {
        bail!("element attribute claims on role locators do not support scope yet — use a raw css or testId locator");
    }
    let snap = browser::snapshot_full(session)
        .map_err(|e| anyhow!("snapshot for element attribute claim: {e}"))?;
    let name = crate::verbs::resolve_name_match(role.name.as_ref(), scope, scenario_dir)?;
    let want = name.as_deref().unwrap_or("");
    let regex_mode = matches!(
        role.name.as_ref(),
        Some(crate::scenario::NameMatch::Pattern {
            r#match: Some(crate::scenario::NameMatchMode::Regex),
            ..
        })
    );
    let re = if regex_mode {
        Some(regex::Regex::new(want).with_context(|| format!("invalid name regex '{want}'"))?)
    } else {
        None
    };
    // Mirror `find role --name`'s case-insensitive substring default,
    // except regex names which match the pattern against the accname.
    let named_lines = browser::snapshot_named_lines(&snap, &role.role);
    let matched: Vec<&(String, String)> = named_lines
        .iter()
        .filter(|(accname, _)| {
            if let Some(re) = &re {
                re.is_match(accname)
            } else {
                accname.to_lowercase().contains(&want.to_lowercase())
            }
        })
        .collect();
    let (_, line) = match matched.as_slice() {
        [] => bail!("no `{role} \"{want}\"` node in the a11y snapshot", role = role.role),
        [one] => one,
        many => bail!(
            "{n} a11y nodes match role='{role}' name='{want}' — narrow the name or use a raw locator",
            n = many.len(),
            role = role.role
        ),
    };
    snapshot_line_attribute(line, attribute)
}

/// Read `attribute` off a matched snapshot line
/// (`checkbox "Sunday" [checked=true, ref=e195]: tail`). Pure — split
/// out for unit tests.
fn snapshot_line_attribute(line: &str, attribute: &str) -> Result<String> {
    // Tokens inside the `[...]` block following the quoted name:
    // `checked=true`, bare `disabled`, `ref=eN`, `level=1`, ...
    let bracket_start = line.find('[');
    let bracket_end = line.find(']');
    let tokens: Vec<String> = match (bracket_start, bracket_end) {
        (Some(s), Some(e)) if s < e => line[s + 1..e]
            .split(',')
            .map(|t| t.trim().to_string())
            .collect(),
        _ => Vec::new(),
    };
    let lookup = |key: &str| -> Option<String> {
        for t in &tokens {
            if t == key {
                return Some("true".to_string());
            }
            if let Some(v) = t.strip_prefix(&format!("{key}=")) {
                return Some(v.to_string());
            }
        }
        None
    };
    const FLAG_STATES: &[&str] = &[
        "checked", "disabled", "required", "expanded", "pressed", "selected", "readonly", "current",
    ];
    const NUMERIC_STATES: &[&str] = &["level", "orientation", "valuemin", "valuemax", "valuenow"];
    match attribute {
        "value" => {
            // `: <text>` tail after the bracket block.
            let tail = bracket_end
                .and_then(|e| line[e + 1..].strip_prefix(':').map(str::trim))
                .unwrap_or("");
            Ok(tail.to_string())
        }
        "text" => {
            // The accessible name is the quoted span after the role.
            let name = line
                .split_once('"')
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(n, _)| n.to_string())
                .unwrap_or_default();
            Ok(name)
        }
        a if FLAG_STATES.contains(&a) => Ok(lookup(a).unwrap_or_else(|| "false".to_string())),
        a if NUMERIC_STATES.contains(&a) => lookup(a)
            .with_context(|| format!("element has no a11y state '{a}'")),
        other => bail!(
            "attribute '{other}' is not readable via a role locator (a11y snapshot carries: checked, disabled, required, expanded, pressed, selected, readonly, current, level, orientation, valuemin/valuemax/valuenow, value, text) — use a raw css or testId locator"
        ),
    }
}

pub(crate) fn locator_resolves(
    session: &str,
    loc: &Locator,
    scope: &mut ValueScope,
    scenario_dir: &std::path::Path,
) -> Result<()> {
    // Re-use the same locator → CLI mapping as dispatch_do uses for
    // its act calls; here we probe with a read act (`text`) just to
    // confirm the element resolves. `find <sel> focus` is not a valid
    // action on agent-browser — it errored every role/xpath probe.
    // If/when agent-browser grows a dedicated `find ... exists` or
    // `query` subverb, switch to that.
    match loc {
        Locator::Role(role) => {
            let name = crate::verbs::resolve_name_match(role.name.as_ref(), scope, scenario_dir)?;
            let name_str = name.as_deref().unwrap_or("");
            if let Some(locs) = role.scope.as_deref() {
                if !locs.is_empty() {
                    let steps = crate::verbs::scope_steps(locs, scope, scenario_dir)?;
                    return if crate::dom_activate::probe_scoped(
                        session, &role.role, name_str, &steps,
                    )? {
                        Ok(())
                    } else {
                        bail!(
                            "scoped locator: no role='{}' name='{}' match inside the scope chain",
                            role.role,
                            name_str
                        )
                    };
                }
            }
            // Probe role+name quietly so a recovering miss doesn't print a
            // misleading `✗ Element not found` line.
            match browser::find_role_act_quiet(session, &role.role, name_str, RoleAct::Text, None) {
                Ok(()) => Ok(()),
                Err(e) if !name_str.is_empty() => {
                    // Same parity hack as verbs.rs::act_on_locator: agent-browser's
                    // role+name engine misses elements inside Radix portals etc.
                    // that the snapshot reports with role+name. We fall back to
                    // a DOM-eval probe by visible text — throws if no element
                    // contains the recorded name (or any of its space-split
                    // chunks of length ≥ 3, to handle the snapshot-stitches-text
                    // -nodes case). If text also misses, parse the snapshot for
                    // `<role> "<name>" [ref=eN]` as the final rung; presence in
                    // the snapshot proves the element exists even if neither
                    // role-engine nor text-engine can locate it.
                    if text_presence_probe(session, name_str).is_ok() {
                        eprintln!(
                            "[v2-replay] role+name miss for role='{}' name='{}' recovered via text presence probe",
                            role.role, name_str
                        );
                        Ok(())
                    } else if snapshot_presence_probe(session, &role.role, name_str).is_ok() {
                        eprintln!(
                            "[v2-replay] role+name miss for role='{}' name='{}' recovered via snapshot ref",
                            role.role, name_str
                        );
                        Ok(())
                    } else {
                        Err(e.into())
                    }
                }
                Err(e) => Err(e.into()),
            }
        }
        Locator::Raw(raw) => {
            let v = substitute_scenario_vars(&raw.raw.value, scope);
            match &raw.raw.kind {
                RawLocatorKind::Css => {
                    css_presence_probe(session, &v)?;
                }
                RawLocatorKind::Xpath => {
                    xpath_presence_probe(session, &v)?;
                }
                RawLocatorKind::TestId => {
                    let css = format!("[data-testid=\"{}\"]", v.replace('"', "\\\""));
                    css_presence_probe(session, &css)?;
                }
                RawLocatorKind::Text => {
                    // `find text` in agent-browser doesn't support focus.
                    // Synthesise a presence probe via in-page eval: throws
                    // when no element with the text exists, so the
                    // surrounding poll_until reads the same signal as a
                    // failed find.
                    let expr = format!(
                        "(() => {{ const want = {q}; const hit = [...document.querySelectorAll('*')].find(el => (el.innerText || el.textContent || '').includes(want)); if (!hit) throw new Error('text not found: ' + want); }})()",
                        q = serde_json::to_string(&v).expect("string serializes")
                    );
                    browser::eval_expression(session, &expr)?;
                }
            }
            Ok(())
        }
    }
}

fn css_presence_probe(session: &str, selector: &str) -> Result<()> {
    let expr = format!(
        "(() => {{ const selector = {q}; const hit = document.querySelector(selector); if (!hit) throw new Error('selector not found: ' + selector); }})()",
        q = serde_json::to_string(selector).expect("string serializes")
    );
    browser::eval_expression(session, &expr)?;
    Ok(())
}

/// Presence probe for XPath locators. agent-browser has no `find xpath`
/// subcommand, so the probe evaluates `document.evaluate` in-page; an
/// unmatched expression throws, same signal as a failed find.
fn xpath_presence_probe(session: &str, xpath: &str) -> Result<()> {
    let expr = format!(
        "(() => {{ const xp = {q}; const hit = document.evaluate(xp, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue; if (!hit) throw new Error('xpath not found: ' + xp); }})()",
        q = serde_json::to_string(xpath).expect("string serializes")
    );
    browser::eval_expression(session, &expr)?;
    Ok(())
}

/// Presence probe by visible text. Tries the full name first, then each
/// whitespace-separated chunk of length ≥ 3. Throws on no match. Used as
/// a fallback when agent-browser's role+name engine misses an element
/// that snapshot reports with role+name (Radix portals etc.).
fn text_presence_probe(session: &str, name: &str) -> Result<()> {
    if probe_text(session, name).is_ok() {
        return Ok(());
    }
    for chunk in name.split_whitespace() {
        if chunk.len() < 3 {
            continue;
        }
        if probe_text(session, chunk).is_ok() {
            return Ok(());
        }
    }
    bail!("no element matched text '{name}' (or any of its chunks)")
}

fn probe_text(session: &str, want: &str) -> Result<()> {
    let expr = format!(
        "(() => {{ const want = {q}; const hit = [...document.querySelectorAll('*')].find(el => (el.innerText || el.textContent || '').includes(want)); if (!hit) throw new Error('text not found: ' + want); }})()",
        q = serde_json::to_string(want).expect("string serializes")
    );
    browser::eval_expression(session, &expr)?;
    Ok(())
}

/// Snapshot-based presence probe: parse the ARIA snapshot for
/// `<role> "<name>" [ref=...]`. Returns Ok if the line exists — the
/// element is present in the page's a11y tree even if the role and
/// text engines can't locate it. Used as the final fallback rung for
/// presence asserts.
fn snapshot_presence_probe(session: &str, role: &str, name: &str) -> Result<()> {
    let snap = browser::snapshot_full(session)
        .map_err(|e| anyhow!("snapshot for ref-based presence probe: {e}"))?;
    if browser::find_ref_in_snapshot(&snap, role, name).is_some() {
        Ok(())
    } else {
        bail!("no `{role} \"{name}\" [ref=...]` line in snapshot")
    }
}

fn poll_until(
    timeout: Duration,
    mut probe: impl FnMut(&mut ValueScope) -> Result<()>,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut scope = ValueScope::default(); // unused but matches signature
    let mut last: Option<anyhow::Error> = None;
    while Instant::now() < deadline {
        match probe(&mut scope) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                thread::sleep(POLL_INTERVAL);
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("predicate timed out")))
}

// ---------- url ----------

fn check_url(
    predicate: &Predicate,
    expected: Option<&Json>,
    ctx: &CheckContext,
    scope: &mut ValueScope,
    timeout: Duration,
) -> Result<()> {
    if matches!(predicate, Predicate::Exists | Predicate::NotExists) {
        // No expected value required — for url we always 'exist'.
        return Ok(());
    }
    let expected = expected
        .ok_or_else(|| anyhow!("url claim with predicate '{predicate:?}' requires 'value'"))?;
    let need = value_to_string(expected);
    let need = substitute_scenario_vars(&need, scope);

    let deadline = Instant::now() + timeout;
    let mut last_err: Option<anyhow::Error> = None;
    let mut last_href = String::new();
    while Instant::now() < deadline {
        let href =
            decode_json_string(browser::eval_expression(ctx.session, "location.href")?.trim());
        let path =
            decode_json_string(browser::eval_expression(ctx.session, "location.pathname")?.trim());
        last_href = href.clone();
        match compare_string(predicate, &href, &need).or_else(|err_href| {
            compare_string(predicate, &path, &need).map_err(|err_path| {
                last_err = Some(err_path);
                err_href
            })
        }) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err.get_or_insert(e);
                thread::sleep(POLL_INTERVAL);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("url claim timed out; last seen url={last_href:?}")))
}

fn decode_json_string(raw: &str) -> String {
    serde_json::from_str::<Json>(raw)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| raw.to_string())
}

// ---------- data / var ----------

fn read_saved(scope: &mut ValueScope, name: &str, path: Option<&str>) -> Result<Json> {
    let base = scope
        .saved_steps
        .get(name)
        .cloned()
        .or_else(|| scope.inputs.get(name).cloned())
        .ok_or_else(|| anyhow!("no saved binding or input named '{name}'"))?;
    match path {
        Some(p) => select_json_path(&base, p),
        None => Ok(base),
    }
}

fn check_value(
    actual: &Json,
    predicate: &Predicate,
    expected: Option<&Json>,
    scope: &mut ValueScope,
) -> Result<()> {
    match predicate {
        Predicate::Exists => {
            if actual.is_null() {
                bail!("expected value to exist");
            }
            Ok(())
        }
        Predicate::NotExists => {
            if !actual.is_null() {
                bail!("expected value not to exist");
            }
            Ok(())
        }
        _ => {
            let expected =
                expected.ok_or_else(|| anyhow!("predicate '{predicate:?}' requires 'value'"))?;
            let need = substitute_scenario_vars(&value_to_string(expected), scope);
            compare_string(predicate, &value_to_string(actual), &need)
        }
    }
}

// ---------- predicates ----------

/// Pages routinely emit non-ASCII whitespace (nbsp, thin space, …) and
/// repeated space runs where a recorded claim expects plain spacing. Fold
/// Unicode space separators to ' ' and collapse runs, so text asserts don't
/// flap on invisible characters.
fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            ws = true;
        } else {
            if ws && !out.is_empty() {
                out.push(' ');
            }
            ws = false;
            out.push(c);
        }
    }
    if ws && !out.is_empty() {
        out.push(' ');
    }
    out
}

fn compare_string(predicate: &Predicate, actual: &str, need: &str) -> Result<()> {
    let actual_n = normalize_ws(actual);
    let need_n = normalize_ws(need);
    let (actual, need) = (actual_n.as_str(), need_n.as_str());
    match predicate {
        Predicate::Equals => {
            if actual != need {
                bail!("expected equals {need:?}, got {actual:?}");
            }
            Ok(())
        }
        Predicate::Contains => {
            if !actual.contains(need) {
                bail!("expected to contain {need:?}, got {actual:?}");
            }
            Ok(())
        }
        Predicate::Matches => {
            let re = Regex::new(need).map_err(|e| anyhow!("invalid regex {need:?}: {e}"))?;
            if !re.is_match(actual) {
                bail!("expected to match /{need}/, got {actual:?}");
            }
            Ok(())
        }
        Predicate::StartsWith => {
            if !actual.starts_with(need) {
                bail!("expected to start with {need:?}, got {actual:?}");
            }
            Ok(())
        }
        Predicate::EndsWith => {
            if !actual.ends_with(need) {
                bail!("expected to end with {need:?}, got {actual:?}");
            }
            Ok(())
        }
        other => bail!("predicate '{other:?}' not supported for this subject"),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser as ab;
    use crate::test_util::lock_env;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn pred_from_json(j: serde_json::Value) -> Predicate {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn element_attribute_expr_attribute_spellings_resolve_idl() {
        // The HTML spelling `readonly` must hit the `readOnly` IDL
        // property — getAttribute would return "" for a bare attribute.
        let expr = element_attribute_read_expr("#x", "readonly");
        assert!(expr.contains(r#"el["readOnly"]"#), "expr={expr}");
        // IDL spellings and any-case spellings resolve the same way.
        let expr = element_attribute_read_expr("#x", "readOnly");
        assert!(expr.contains(r#"el["readOnly"]"#), "expr={expr}");
        let expr = element_attribute_read_expr("#x", "Checked");
        assert!(expr.contains(r#"el["checked"]"#), "expr={expr}");
        let expr = element_attribute_read_expr("#x", "tabindex");
        assert!(expr.contains(r#"el["tabIndex"]"#), "expr={expr}");
        let expr = element_attribute_read_expr("#x", "MAXLENGTH");
        assert!(expr.contains(r#"el["maxLength"]"#), "expr={expr}");
    }

    #[test]
    fn element_attribute_expr_unknown_names_read_getattribute() {
        let expr = element_attribute_read_expr("#x", "data-foo");
        assert!(expr.contains(r#"getAttribute("data-foo")"#), "expr={expr}");
        let expr = element_attribute_read_expr("#x", "href");
        assert!(expr.contains(r#"getAttribute("href")"#), "expr={expr}");
    }

    #[test]
    fn element_attribute_expr_special_subjects() {
        let expr = element_attribute_read_expr("#x", "text");
        assert!(expr.contains("textContent"), "expr={expr}");
        let expr = element_attribute_read_expr("#x", "focused");
        assert!(expr.contains("activeElement"), "expr={expr}");
        let expr = element_attribute_read_expr("#x", "style:color");
        assert!(expr.contains(r#"getPropertyValue("color")"#), "expr={expr}");
    }

    #[test]
    fn compare_equals() {
        compare_string(&pred_from_json(json!("equals")), "x", "x").unwrap();
        compare_string(&pred_from_json(json!("equals")), "x", "y").unwrap_err();
    }

    #[test]
    fn compare_contains_matches_starts_ends() {
        compare_string(&pred_from_json(json!("contains")), "hello world", "lo wo").unwrap();
        compare_string(&pred_from_json(json!("matches")), "abc123", r"\d{3}$").unwrap();
        compare_string(&pred_from_json(json!("startsWith")), "abc", "ab").unwrap();
        compare_string(&pred_from_json(json!("endsWith")), "abc", "bc").unwrap();
    }

    #[test]
    fn snapshot_line_attribute_reads_state_tokens() {
        let checked = r#"checkbox "Sunday" [checked=true, ref=e195]"#;
        assert_eq!(snapshot_line_attribute(checked, "checked").unwrap(), "true");
        let unchecked = r#"radio "Male" [checked=false, ref=e193]"#;
        assert_eq!(
            snapshot_line_attribute(unchecked, "checked").unwrap(),
            "false"
        );
        // Bare flag token + absent flag → "false".
        let disabled = r#"button "Save" [disabled, ref=e9]"#;
        assert_eq!(
            snapshot_line_attribute(disabled, "disabled").unwrap(),
            "true"
        );
        assert_eq!(
            snapshot_line_attribute(disabled, "checked").unwrap(),
            "false"
        );
        // Numeric/enum state key=value.
        let heading = r#"heading "Automation" [level=1, ref=e3]"#;
        assert_eq!(snapshot_line_attribute(heading, "level").unwrap(), "1");
        // `: tail` reads as value.
        let textbox = r#"textbox "Enter Name" [required, ref=e57]: Devin Dogfood"#;
        assert_eq!(
            snapshot_line_attribute(textbox, "value").unwrap(),
            "Devin Dogfood"
        );
        assert_eq!(
            snapshot_line_attribute(textbox, "required").unwrap(),
            "true"
        );
        assert_eq!(
            snapshot_line_attribute(textbox, "text").unwrap(),
            "Enter Name"
        );
        // Attributes the a11y tree doesn't carry point at raw locators.
        let err = snapshot_line_attribute(textbox, "href")
            .unwrap_err()
            .to_string();
        assert!(err.contains("raw css or testId"), "err={err}");
        let err = snapshot_line_attribute(checked, "level")
            .unwrap_err()
            .to_string();
        assert!(err.contains("no a11y state"), "err={err}");
    }

    #[test]
    fn compare_folds_unicode_whitespace() {
        // NBSP (and friends) compare equal to a plain space.
        compare_string(
            &pred_from_json(json!("contains")),
            "Men - \u{00A0}Tshirts Products",
            "Men - Tshirts",
        )
        .unwrap();
        compare_string(&pred_from_json(json!("equals")), "a\u{2009}b", "a b").unwrap();
        compare_string(
            &pred_from_json(json!("startsWith")),
            "\u{00A0}lead",
            " lead",
        )
        .unwrap();
    }

    #[test]
    fn check_value_exists_and_not_exists() {
        let mut scope = ValueScope::default();
        check_value(
            &json!("x"),
            &pred_from_json(json!("exists")),
            None,
            &mut scope,
        )
        .unwrap();
        check_value(
            &json!(null),
            &pred_from_json(json!("exists")),
            None,
            &mut scope,
        )
        .unwrap_err();
        check_value(
            &json!(null),
            &pred_from_json(json!("notExists")),
            None,
            &mut scope,
        )
        .unwrap();
    }

    #[test]
    fn console_subject_no_errors_passes_when_none_logged() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_console(tmp.path(), &[]);
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "console": { "type": "error" } },
            "predicate": "notExists"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
        clear_console();
    }

    #[test]
    fn console_subject_text_predicate_matches_any_message() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_console(
            tmp.path(),
            &[
                json!({ "type": "log", "text": "boot ok" }),
                json!({ "type": "warn", "text": "deprecation: use v2" }),
            ],
        );
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "console": { "type": "warn" } },
            "predicate": "contains",
            "value": "deprecation"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
        clear_console();
    }

    #[test]
    fn console_subject_error_present_fails_not_exists() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_console(
            tmp.path(),
            &[json!({ "type": "error", "text": "TypeError: x is undefined" })],
        );
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "console": { "type": "error" } },
            "predicate": "notExists"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        // times out fast — pass a 1ms timeout so the poll exits immediately
        let err =
            dispatch_check(&claim, &ctx, &mut scope, Some(Duration::from_millis(1))).unwrap_err();
        clear_console();
        assert!(err.to_string().contains("console claim"), "got: {err}");
    }

    fn install_fake_console(dir: &Path, messages: &[serde_json::Value]) {
        // Respond to `console` with a canned --json payload; other verbs
        // are no-ops.
        let resp_path = dir.join("console.json");
        fs::write(
            &resp_path,
            json!({ "success": true, "data": { "messages": messages } }).to_string(),
        )
        .unwrap();
        let body = format!(
            "#!/bin/sh\ncase \" $* \" in *\\ console\\ *) cat '{}' ;;\nesac\nexit 0\n",
            resp_path.display()
        );
        let bin = dir.join("agent-browser");
        fs::write(&bin, body).unwrap();
        let mut perm = fs::metadata(&bin).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&bin, perm).unwrap();
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn install_fake_errors(dir: &Path, errors: &[serde_json::Value]) {
        // Respond to `errors` with a canned --json payload; other verbs
        // are no-ops.
        let resp_path = dir.join("errors.json");
        fs::write(
            &resp_path,
            json!({ "success": true, "data": { "errors": errors } }).to_string(),
        )
        .unwrap();
        let body = format!(
            "#!/bin/sh\ncase \" $* \" in *\\ errors\\ *) cat '{}' ;;\nesac\nexit 0\n",
            resp_path.display()
        );
        let bin = dir.join("agent-browser");
        fs::write(&bin, body).unwrap();
        let mut perm = fs::metadata(&bin).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&bin, perm).unwrap();
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    #[test]
    fn page_error_subject_no_errors_passes_not_exists() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_errors(tmp.path(), &[]);
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "pageError": true },
            "predicate": "notExists"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
        clear_console();
    }

    #[test]
    fn page_error_subject_text_match_and_url_filter() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_errors(
            tmp.path(),
            &[
                json!({ "text": "TypeError: Cannot read properties of undefined (reading 'xyz')", "url": "https://app.example/a" }),
                json!({ "text": "RangeError: too far", "url": null }),
            ],
        );
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "pageError": { "text": "Cannot read", "url": "app.example" } },
            "predicate": "exists"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

        // text predicate against the error's rendered text
        let claim2: Claim = serde_json::from_value(json!({
            "subject": { "pageError": { "text": "too far" } },
            "predicate": "contains",
            "value": "RangeError"
        }))
        .unwrap();
        dispatch_check(&claim2, &ctx, &mut scope, None).unwrap();

        // count the total
        let claim3: Claim = serde_json::from_value(json!({
            "subject": { "pageError": true },
            "predicate": "countEquals",
            "value": 2
        }))
        .unwrap();
        dispatch_check(&claim3, &ctx, &mut scope, None).unwrap();
        clear_console();
    }

    #[test]
    fn page_error_subject_error_present_fails_not_exists() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_errors(
            tmp.path(),
            &[json!({ "text": "TypeError: x is undefined", "url": null })],
        );
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "pageError": true },
            "predicate": "notExists"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        let err =
            dispatch_check(&claim, &ctx, &mut scope, Some(Duration::from_millis(1))).unwrap_err();
        clear_console();
        assert!(err.to_string().contains("pageError claim"), "got: {err}");
    }

    fn clear_console() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    fn install_fake_a11y(dir: &Path, data: serde_json::Value) {
        // Respond to `a11y` with a canned --json payload; other verbs no-op.
        let resp_path = dir.join("a11y.json");
        fs::write(
            &resp_path,
            json!({ "success": true, "data": data }).to_string(),
        )
        .unwrap();
        let body = format!(
            "#!/bin/sh\ncase \" $* \" in *\\ a11y\\ *) cat '{}' ;;\nesac\nexit 0\n",
            resp_path.display()
        );
        let bin = dir.join("agent-browser");
        fs::write(&bin, body).unwrap();
        let mut perm = fs::metadata(&bin).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&bin, perm).unwrap();
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn a11y_ctx() -> CheckContext<'static> {
        CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        }
    }

    #[test]
    fn a11y_clean_page_passes_notexists() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_a11y(
            tmp.path(),
            json!({ "violations": [], "incomplete": [], "counts": { "violations": 0 } }),
        );
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "a11y": true },
            "predicate": "notExists"
        }))
        .unwrap();
        dispatch_check(&claim, &a11y_ctx(), &mut ValueScope::default(), None).unwrap();
        clear_console();
    }

    #[test]
    fn a11y_impact_floor_counts_at_or_above() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_a11y(
            tmp.path(),
            json!({ "violations": [
                { "id": "minor-one", "impact": "minor" },
                { "id": "serious-one", "impact": "serious" },
                { "id": "critical-one", "impact": "critical" }
            ], "incomplete": [] }),
        );
        let mut scope = ValueScope::default();
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "a11y": { "impact": "serious" } },
            "predicate": "countEquals",
            "value": 2
        }))
        .unwrap();
        dispatch_check(&claim, &a11y_ctx(), &mut scope, None).unwrap();
        clear_console();
    }

    #[test]
    fn a11y_rule_filter_and_incomplete_flag() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_a11y(
            tmp.path(),
            json!({ "violations": [ { "id": "color-contrast", "impact": "serious" } ],
                    "incomplete": [ { "id": "scrollable-region-focusable", "impact": "serious" } ] }),
        );
        let mut scope = ValueScope::default();
        // rule filter keeps only the named rule
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "a11y": { "rule": "color-contrast" } },
            "predicate": "exists"
        }))
        .unwrap();
        dispatch_check(&claim, &a11y_ctx(), &mut scope, None).unwrap();
        // incomplete excluded by default → total is 1
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "a11y": true },
            "predicate": "countEquals",
            "value": 1
        }))
        .unwrap();
        dispatch_check(&claim, &a11y_ctx(), &mut scope, None).unwrap();
        // incomplete:true counts both buckets → 2
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "a11y": { "incomplete": true } },
            "predicate": "countEquals",
            "value": 2
        }))
        .unwrap();
        dispatch_check(&claim, &a11y_ctx(), &mut scope, None).unwrap();
        clear_console();
    }

    #[test]
    fn a11y_failure_lists_rule_ids() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake_a11y(
            tmp.path(),
            json!({ "violations": [ { "id": "document-title", "impact": "serious" } ] }),
        );
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "a11y": true },
            "predicate": "notExists"
        }))
        .unwrap();
        let err =
            dispatch_check(&claim, &a11y_ctx(), &mut ValueScope::default(), None).unwrap_err();
        assert!(err.to_string().contains("document-title"), "{err}");
        clear_console();
    }

    #[test]
    fn read_saved_walks_path() {
        let mut scope = ValueScope::default();
        scope
            .saved_steps
            .insert("user".into(), json!({ "profile": { "id": "u1" } }));
        let v = read_saved(&mut scope, "user", Some("$.profile.id")).unwrap();
        assert_eq!(v, json!("u1"));
    }

    #[test]
    fn decode_json_string_unwraps_quoted_strings() {
        assert_eq!(decode_json_string("\"hello\""), "hello");
        assert_eq!(decode_json_string("hello"), "hello");
        assert_eq!(decode_json_string("123"), "123");
    }

    #[test]
    fn dispatch_data_subject_equals_via_saved_step() {
        let mut scope = ValueScope::default();
        scope.saved_steps.insert("greet".into(), json!("hello"));
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "data": "greet" },
            "predicate": "equals",
            "value": "hello"
        }))
        .unwrap();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn dispatch_var_subject_with_path() {
        let mut scope = ValueScope::default();
        scope
            .saved_steps
            .insert("u".into(), json!({ "n": "alice" }));
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "kind": "var", "name": "u", "path": "$.n" },
            "predicate": "matches",
            "value": "^al"
        }))
        .unwrap();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn element_text_contains_reads_raw_css_text() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("agent-browser");
        fs::write(
            &bin,
            "#!/bin/sh\nif [ \"$3\" = eval ]; then printf '%s\\n' '\"Data Loaded!\"'; exit 0; fi\nexit 1\n",
        )
        .unwrap();
        let mut perm = fs::metadata(&bin).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&bin, perm).unwrap();
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();

        let claim: Claim = serde_json::from_value(json!({
            "subject": { "element": { "raw": { "kind": "css", "value": "main" }, "reason": "test" }, "attribute": "text" },
            "predicate": "contains",
            "value": "Loaded"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    #[test]
    fn dispatch_network_subject_routes_to_check_network() {
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "network": { "urlMatches": "x" } },
            "predicate": "exists"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: Path::new("."),
            run_dir: None,
        };
        let err =
            dispatch_check(&claim, &ctx, &mut scope, Some(Duration::from_millis(300))).unwrap_err();
        assert!(
            err.to_string().contains("network claim timed out"),
            "got: {err}"
        );
    }

    mod flag_subject {
        use super::*;
        use crate::browser as ab;
        use crate::test_util::lock_env;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;
        use tempfile::TempDir;

        fn install_fake_eval(dir: &Path, payload: &str) {
            // Echo the IIFE return verbatim. agent-browser's eval bridge
            // would normally double-encode strings, but our IIFE returns
            // an object directly so the wire is a single layer of JSON.
            let resp_path = dir.join("resp.txt");
            fs::write(&resp_path, payload).unwrap();
            let body = format!(
                "#!/bin/sh\nif [ \"$3\" = eval ]; then cat '{}'; exit 0; fi\nexit 0\n",
                resp_path.display()
            );
            let bin = dir.join("agent-browser");
            fs::write(&bin, body).unwrap();
            let mut perm = fs::metadata(&bin).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&bin, perm).unwrap();
            std::env::set_var(ab::BIN_ENV, &bin);
            ab::_reset_bin_cache_for_tests();
        }

        fn clear() {
            std::env::remove_var(ab::BIN_ENV);
            ab::_reset_bin_cache_for_tests();
        }

        #[test]
        fn equals_true_passes_when_flag_true() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{"my-flag":true}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "my-flag" },
                "predicate": "equals",
                "value": true
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn storage_subject_parses_key_and_matcher_forms() {
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "storage": "token" },
                "predicate": "exists"
            }))
            .unwrap();
            assert!(matches!(
                claim.subject,
                ClaimSubject::Storage {
                    storage: crate::scenario::StorageSubject::Key(_),
                    ..
                }
            ));
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "storage": { "key": "cart", "scope": "session" }, "path": "$.total" },
                "predicate": "gte",
                "value": 0
            }))
            .unwrap();
            match claim.subject {
                ClaimSubject::Storage { storage, path } => {
                    let crate::scenario::StorageSubject::Matcher(m) = storage else {
                        panic!("expected matcher form");
                    };
                    assert_eq!(m.key, "cart");
                    assert_eq!(m.scope.as_deref(), Some("session"));
                    assert_eq!(path.as_deref(), Some("$.total"));
                }
                _ => panic!("expected storage subject"),
            }
        }

        #[test]
        fn cookie_subject_reads_the_cdp_cookie_jar() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            // Fake `agent-browser cookies` — the CDP jar dump carries
            // HttpOnly cookies that document.cookie would hide.
            let resp_path = tmp.path().join("cookies.txt");
            fs::write(&resp_path, "other=x\nsession=abc\n").unwrap();
            let body = format!(
                "#!/bin/sh\nif [ \"$3\" = cookies ]; then cat '{}'; exit 0; fi\nexit 0\n",
                resp_path.display()
            );
            let bin = tmp.path().join("agent-browser");
            fs::write(&bin, body).unwrap();
            let mut perm = fs::metadata(&bin).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&bin, perm).unwrap();
            std::env::set_var(ab::BIN_ENV, &bin);
            ab::_reset_bin_cache_for_tests();
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "cookie": "session" },
                "predicate": "equals",
                "value": "abc"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn cookie_subject_not_exists_when_absent_from_jar() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            let resp_path = tmp.path().join("cookies.txt");
            fs::write(&resp_path, "other=x\n").unwrap();
            let body = format!(
                "#!/bin/sh\nif [ \"$3\" = cookies ]; then cat '{}'; exit 0; fi\nexit 0\n",
                resp_path.display()
            );
            let bin = tmp.path().join("agent-browser");
            fs::write(&bin, body).unwrap();
            let mut perm = fs::metadata(&bin).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&bin, perm).unwrap();
            std::env::set_var(ab::BIN_ENV, &bin);
            ab::_reset_bin_cache_for_tests();
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "cookie": "session" },
                "predicate": "notExists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn clipboard_subject_parses_and_reads_text() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), "\"aq-copied-text\"");
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "clipboard": true },
                "predicate": "contains",
                "value": "aq-copied"
            }))
            .unwrap();
            assert!(matches!(claim.subject, ClaimSubject::Clipboard { .. }));
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn clipboard_empty_text_reads_as_null() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), "\"\"");
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "clipboard": true },
                "predicate": "notExists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn clipboard_read_error_bails_with_reason() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), "\"__aq_clip_err__NotAllowedError\"");
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "clipboard": true },
                "predicate": "exists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            let err = dispatch_check(&claim, &ctx, &mut scope, None)
                .unwrap_err()
                .to_string();
            clear();
            assert!(err.contains("NotAllowedError"), "got: {err}");
        }

        #[test]
        fn indexeddb_subject_parses_matcher_forms() {
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "indexeddb": { "db": "cart", "store": "items" } },
                "predicate": "exists"
            }))
            .unwrap();
            match claim.subject {
                ClaimSubject::IndexedDb { indexeddb, path } => {
                    assert_eq!(indexeddb.db, "cart");
                    assert_eq!(indexeddb.store, "items");
                    assert!(indexeddb.key.is_none());
                    assert!(path.is_none());
                }
                _ => panic!("expected indexeddb subject"),
            }
            let claim: Claim = serde_json::from_value(json!({
                "subject": {
                    "indexeddb": { "db": "cart", "store": "items", "key": "sku-1" },
                    "path": "$.qty"
                },
                "predicate": "gte",
                "value": 1
            }))
            .unwrap();
            match claim.subject {
                ClaimSubject::IndexedDb { indexeddb, path } => {
                    assert_eq!(indexeddb.key.as_deref(), Some("sku-1"));
                    assert_eq!(path.as_deref(), Some("$.qty"));
                }
                _ => panic!("expected indexeddb subject"),
            }
        }

        #[test]
        fn indexeddb_record_walks_json_by_path() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{"qty":2}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": {
                    "indexeddb": { "db": "cart", "store": "items", "key": "sku-1" },
                    "path": "$.qty"
                },
                "predicate": "equals",
                "value": 2
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn indexeddb_absent_record_passes_not_exists() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), "null");
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "indexeddb": { "db": "cart", "store": "items", "key": "gone" } },
                "predicate": "notExists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn indexeddb_store_probe_passes_exists() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), "\"__aq_idb_store__\"");
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "indexeddb": { "db": "cart", "store": "items" } },
                "predicate": "exists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn indexeddb_unsupported_browser_bails() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), "\"__aq_idb_unsupported__\"");
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "indexeddb": { "db": "cart", "store": "items" } },
                "predicate": "exists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            let err = dispatch_check(&claim, &ctx, &mut scope, None)
                .unwrap_err()
                .to_string();
            clear();
            assert!(err.contains("databases()"), "got: {err}");
        }

        #[test]
        fn storage_subject_walks_json_value_by_path() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            // localStorage returns the stored JSON text; the eval layer
            // wraps it once, so the payload is a quoted string.
            install_fake_eval(tmp.path(), "\"{\\\"profile\\\":{\\\"id\\\":\\\"u1\\\"}}\"");
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "storage": { "key": "user" }, "path": "$.profile.id" },
                "predicate": "equals",
                "value": "u1"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn equals_true_fails_when_flag_false() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{"my-flag":false}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "my-flag" },
                "predicate": "equals",
                "value": true
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            let err = dispatch_check(&claim, &ctx, &mut scope, None)
                .unwrap_err()
                .to_string();
            assert!(err.contains("= true, got false"));
            clear();
        }

        #[test]
        fn exists_passes_when_present_either_value() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{"my-flag":false}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "my-flag" },
                "predicate": "exists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }

        #[test]
        fn not_exists_passes_when_absent() {
            let _g = lock_env();
            let tmp = TempDir::new().unwrap();
            install_fake_eval(tmp.path(), r#"{}"#);
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "flag": "unset" },
                "predicate": "notExists"
            }))
            .unwrap();
            let mut scope = ValueScope::default();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Path::new("."),
                run_dir: None,
            };
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
            clear();
        }
    }

    mod timing_claims {
        use super::*;
        use tempfile::TempDir;

        fn ctx_with_events(rows: &[serde_json::Value]) -> (TempDir, CheckContext<'static>) {
            let run = TempDir::new().unwrap();
            let body: String = rows
                .iter()
                .map(|r| serde_json::to_string(r).unwrap() + "\n")
                .collect();
            std::fs::write(run.path().join("events.jsonl"), body).unwrap();
            // Leak the path so the ctx can borrow it past `run`'s move —
            // test-only, the tempdir itself still cleans up on drop.
            let ctx = CheckContext {
                session: "s",
                scenario_dir: Box::leak(Box::new(run.path().to_path_buf())),
                run_dir: Some(Box::leak(Box::new(run.path().to_path_buf()))),
            };
            (run, ctx)
        }

        #[test]
        fn lt_passes_and_gt_fails_on_step_ms() {
            let (_t, ctx) = ctx_with_events(&[
                json!({"idx":1,"id":"s3","status":"running"}),
                json!({"idx":1,"id":"s3","status":"pass","ms":1420}),
            ]);
            let mut scope = ValueScope::default();
            let fast: Claim = serde_json::from_value(json!({
                "subject": {"timing": "s3"}, "predicate": "lt", "value": 2000
            }))
            .unwrap();
            dispatch_check(&fast, &ctx, &mut scope, None).unwrap();
            let slow: Claim = serde_json::from_value(json!({
                "subject": {"timing": "s3"}, "predicate": "gt", "value": 2000
            }))
            .unwrap();
            let e = dispatch_check(&slow, &ctx, &mut scope, None).unwrap_err();
            assert!(e.to_string().contains("1420ms"), "got: {e}");
        }

        #[test]
        fn missing_step_bails_and_not_exists_passes() {
            let (_t, ctx) = ctx_with_events(&[json!({"idx":1,"id":"s0","status":"pass","ms":100})]);
            let mut scope = ValueScope::default();
            let missing: Claim = serde_json::from_value(json!({
                "subject": {"timing": "s9"}, "predicate": "lt", "value": 1000
            }))
            .unwrap();
            assert!(dispatch_check(&missing, &ctx, &mut scope, None)
                .unwrap_err()
                .to_string()
                .contains("no timed step row"));
            let absent: Claim = serde_json::from_value(json!({
                "subject": {"timing": "s9"}, "predicate": "notExists"
            }))
            .unwrap();
            dispatch_check(&absent, &ctx, &mut scope, None).unwrap();
        }

        #[test]
        fn latest_terminal_row_wins_over_running() {
            let (_t, ctx) = ctx_with_events(&[
                json!({"id":"s1","status":"pass","ms":900}),
                json!({"id":"s1","status":"running"}),
            ]);
            let mut scope = ValueScope::default();
            let c: Claim = serde_json::from_value(json!({
                "subject": {"timing": "s1"}, "predicate": "equals", "value": 900
            }))
            .unwrap();
            dispatch_check(&c, &ctx, &mut scope, None).unwrap();
        }
    }

    mod file_claims {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn exists_and_size_on_relative_path() {
            let tmp = TempDir::new().unwrap();
            std::fs::write(tmp.path().join("report.txt"), b"hello world").unwrap();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: tmp.path(),
                run_dir: None,
            };
            let mut scope = ValueScope::default();

            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "report.txt" },
                "predicate": "exists"
            }))
            .unwrap();
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "report.txt" },
                "predicate": "gt",
                "value": 10
            }))
            .unwrap();
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "report.txt" },
                "predicate": "equals",
                "value": "report.txt"
            }))
            .unwrap();
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
        }

        #[test]
        fn not_exists_when_absent_and_name_mismatch_times_out() {
            let tmp = TempDir::new().unwrap();
            std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: tmp.path(),
                run_dir: None,
            };
            let mut scope = ValueScope::default();

            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "missing.bin" },
                "predicate": "notExists"
            }))
            .unwrap();
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "a.txt" },
                "predicate": "contains",
                "value": "zzz"
            }))
            .unwrap();
            let err = dispatch_check(&claim, &ctx, &mut scope, Some(Duration::from_millis(300)))
                .unwrap_err()
                .to_string();
            assert!(err.contains("file claim timed out"), "got: {err}");
        }

        #[test]
        fn content_attribute_compares_file_text() {
            let tmp = TempDir::new().unwrap();
            std::fs::write(tmp.path().join("data.json"), b"{\"fixture\":true}").unwrap();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: tmp.path(),
                run_dir: None,
            };
            let mut scope = ValueScope::default();

            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "data.json", "attribute": "content" },
                "predicate": "contains",
                "value": "\"fixture\":true"
            }))
            .unwrap();
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "data.json", "attribute": "content" },
                "predicate": "equals",
                "value": "{\"fixture\":true}"
            }))
            .unwrap();
            dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
        }

        #[test]
        fn unknown_attribute_errors() {
            let tmp = TempDir::new().unwrap();
            std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
            let ctx = CheckContext {
                session: "s",
                scenario_dir: tmp.path(),
                run_dir: None,
            };
            let mut scope = ValueScope::default();
            let claim: Claim = serde_json::from_value(json!({
                "subject": { "file": "a.txt", "attribute": "sha256" },
                "predicate": "exists"
            }))
            .unwrap();
            let err = dispatch_check(&claim, &ctx, &mut scope, None)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("does not support attribute 'sha256'"),
                "got: {err}"
            );
        }
    }

    #[test]
    fn shot_claim_matches_identical_and_flags_drift() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("screenshots")).unwrap();

        // 4x4 baseline: opaque white; current: same, then with 1 pixel red.
        let a = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
        a.save(sid_dir.join("baselines/s1.png")).unwrap();
        a.save(run_dir.join("screenshots/s1.png")).unwrap();

        let claim: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" },
            "predicate": "matches"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

        // Flip one pixel → 1/16 = 6.25% > default 1% → fail + diff png.
        let mut b = a.clone();
        b.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        b.save(run_dir.join("screenshots/s1.png")).unwrap();
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("pixels changed"), "got: {err}");
        assert!(run_dir.join("shots-diff/s1.diff.png").is_file());

        // tolerance.pixels=0.1 accepts the drift.
        let claim2: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" },
            "predicate": "matches",
            "tolerance": { "pixels": 0.1 }
        }))
        .unwrap();
        dispatch_check(&claim2, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn shot_claim_size_mismatch_still_writes_a_diff() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("screenshots")).unwrap();

        // 4x4 baseline vs taller 4x6 current — bail on size, but the
        // padded diff must still land so reviewers see the change.
        image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]))
            .save(sid_dir.join("baselines/s1.png"))
            .unwrap();
        image::RgbaImage::from_pixel(4, 6, image::Rgba([255, 255, 255, 255]))
            .save(run_dir.join("screenshots/s1.png"))
            .unwrap();

        let claim: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" },
            "predicate": "matches"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("changed size"), "got: {err}");
        let diff = run_dir.join("shots-diff/s1.diff.png");
        assert!(diff.is_file(), "padded diff should be written");
        let img = crate::compare::screenshots::decode_png(&diff).unwrap();
        assert_eq!(img.dimensions(), (4, 6));
    }

    #[test]
    fn shot_claim_bails_without_baseline_or_run_dir() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        fs::create_dir_all(&sid_dir).unwrap();
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" },
            "predicate": "matches"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: None,
        };
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("run-step"), "got: {err}");
        let ctx2 = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(tmp.path()),
        };
        let err2 = dispatch_check(&claim, &ctx2, &mut scope, None).unwrap_err();
        assert!(err2.to_string().contains("shot-accept"), "got: {err2}");
    }

    #[test]
    fn shot_clip_parses_and_crop_clamps_to_bounds() {
        let _g = lock_env();
        // `{"shot":"s1","clip":{...}}` parses as the Shot subject's locator.
        let claim: Claim = serde_json::from_value(json!({
            "subject": {
                "shot": "s1",
                "clip": { "raw": { "kind": "css", "value": "#card" }, "reason": "clip target" }
            },
            "predicate": "matches"
        }))
        .unwrap();
        match claim.subject {
            ClaimSubject::Shot { shot, clip, .. } => {
                assert_eq!(shot, "s1");
                assert!(clip.is_some());
            }
            other => panic!("expected shot subject, got {other:?}"),
        }

        // Crop clamps to bounds and rejects empty rects.
        let img = image::RgbaImage::from_pixel(10, 10, image::Rgba([1, 2, 3, 255]));
        let c = crop_to_rect(&img, (4, 2, 4, 3)).unwrap();
        assert_eq!(c.dimensions(), (4, 3));
        assert_eq!(c.get_pixel(0, 0), &image::Rgba([1, 2, 3, 255]));
        // Overhanging box → clamped to the image edge.
        let c2 = crop_to_rect(&img, (8, 8, 10, 10)).unwrap();
        assert_eq!(c2.dimensions(), (2, 2));
        // Empty rect → hard error.
        assert!(crop_to_rect(&img, (0, 0, 0, 5)).is_err());
    }

    #[test]
    fn shot_tolerance_preset_sets_pixels_and_aa() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("screenshots")).unwrap();

        // 16x16 baseline; current differs in ONE pixel by delta 20
        // (255→235): inside balanced AA (32) → invisible, outside strict
        // AA (16) → a 0.39% diff that strict's pixels=0 rejects.
        let a = image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 255, 255, 255]));
        a.save(sid_dir.join("baselines/s1.png")).unwrap();
        let mut b = a.clone();
        b.put_pixel(0, 0, image::Rgba([235, 235, 235, 255]));
        b.save(run_dir.join("screenshots/s1.png")).unwrap();

        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        let mut scope = ValueScope::default();

        // balanced (default): AA swallows the delta → pass.
        let balanced: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" }, "predicate": "matches"
        }))
        .unwrap();
        dispatch_check(&balanced, &ctx, &mut scope, None).unwrap();
        let explicit: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" }, "predicate": "matches",
            "tolerance": { "preset": "balanced" }
        }))
        .unwrap();
        dispatch_check(&explicit, &ctx, &mut scope, None).unwrap();

        // strict: aa=16 counts the delta and pixels=0 rejects the frac.
        let strict: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" }, "predicate": "matches",
            "tolerance": { "preset": "strict" }
        }))
        .unwrap();
        let err = dispatch_check(&strict, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("pixels changed"), "got: {err}");

        // tolerance.pixels overrides the preset's own value: relaxed
        // preset + pixels:0 still fails on a counted pixel.
        let override_claim: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" }, "predicate": "matches",
            "tolerance": { "preset": "relaxed", "pixels": 0, "aa": 100 }
        }))
        .unwrap();
        // aa:100 swallows delta 20 → zero differing pixels → pass.
        dispatch_check(&override_claim, &ctx, &mut scope, None).unwrap();

        // Unknown preset → hard error naming the valid set.
        let bad: Claim = serde_json::from_value(json!({
            "subject": { "shot": "s1" }, "predicate": "matches",
            "tolerance": { "preset": "snug" }
        }))
        .unwrap();
        let err = dispatch_check(&bad, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("preset"), "got: {err}");
    }

    // ---------- layout claims ----------

    fn layout_doc(els: &[(&str, f64, f64, f64, f64)]) -> String {
        let els: Vec<Json> = els
            .iter()
            .map(|(k, x, y, w, h)| json!({"k": k, "x": x, "y": y, "w": w, "h": h}))
            .collect();
        serde_json::to_string(&json!({"v": 1, "vw": 800, "vh": 600, "els": els})).unwrap()
    }

    #[test]
    fn layout_claim_passes_identical_flags_moved_and_writes_diff() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("layouts")).unwrap();

        let doc = layout_doc(&[
            ("body>div#card", 10.0, 20.0, 300.0, 100.0),
            ("body>div#card>button", 20.0, 60.0, 80.0, 30.0),
        ]);
        fs::write(sid_dir.join("baselines/s1.layout.json"), &doc).unwrap();
        fs::write(run_dir.join("layouts/s1.json"), &doc).unwrap();

        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        let mut scope = ValueScope::default();
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "layout": "s1" }, "predicate": "matches"
        }))
        .unwrap();
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

        // Subject parses as the Layout arm.
        match claim.subject {
            ClaimSubject::Layout { ref layout } => assert_eq!(layout, "s1"),
            _ => panic!("expected layout subject"),
        }

        // 2px move is inside the default px=4 → still passes.
        let drifted = layout_doc(&[
            ("body>div#card", 12.0, 20.0, 300.0, 100.0),
            ("body>div#card>button", 20.0, 60.0, 80.0, 30.0),
        ]);
        fs::write(run_dir.join("layouts/s1.json"), &drifted).unwrap();
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();

        // 20px move → fail + layouts-diff JSON with the moved key.
        let moved = layout_doc(&[
            ("body>div#card", 30.0, 20.0, 300.0, 100.0),
            ("body>div#card>button", 20.0, 60.0, 80.0, 30.0),
        ]);
        fs::write(run_dir.join("layouts/s1.json"), &moved).unwrap();
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("1 moved"), "got: {err}");
        let diff = run_dir.join("layouts-diff/s1.diff.json");
        assert!(diff.is_file());
        let diff_doc: Json = serde_json::from_str(&fs::read_to_string(diff).unwrap()).unwrap();
        assert_eq!(diff_doc["moved"][0]["key"], "body>div#card");
    }

    #[test]
    fn layout_claim_added_removed_and_tolerance_counts() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("layouts")).unwrap();

        fs::write(
            sid_dir.join("baselines/s1.layout.json"),
            layout_doc(&[
                ("body>div#a", 0.0, 0.0, 10.0, 10.0),
                ("body>div#b", 0.0, 20.0, 10.0, 10.0),
            ]),
        )
        .unwrap();
        // div#b removed, div#c added.
        fs::write(
            run_dir.join("layouts/s1.json"),
            layout_doc(&[
                ("body>div#a", 0.0, 0.0, 10.0, 10.0),
                ("body>div#c", 0.0, 20.0, 10.0, 10.0),
            ]),
        )
        .unwrap();

        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        let mut scope = ValueScope::default();
        let strict: Claim = serde_json::from_value(json!({
            "subject": { "layout": "s1" }, "predicate": "matches"
        }))
        .unwrap();
        let err = dispatch_check(&strict, &ctx, &mut scope, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("1 added") && msg.contains("1 removed"),
            "got: {msg}"
        );

        let lenient: Claim = serde_json::from_value(json!({
            "subject": { "layout": "s1" }, "predicate": "matches",
            "tolerance": { "added": 1, "removed": 1 }
        }))
        .unwrap();
        dispatch_check(&lenient, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn layout_claim_bails_without_baseline_capture_or_wrong_predicate() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(&sid_dir).unwrap();
        fs::create_dir_all(&run_dir).unwrap();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        let mut scope = ValueScope::default();
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "layout": "s1" }, "predicate": "matches"
        }))
        .unwrap();
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("layout-accept"), "got: {err}");

        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::write(
            sid_dir.join("baselines/s1.layout.json"),
            layout_doc(&[("body>div#a", 0.0, 0.0, 10.0, 10.0)]),
        )
        .unwrap();
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("no layout capture"), "got: {err}");

        let wrong_pred: Claim = serde_json::from_value(json!({
            "subject": { "layout": "s1" }, "predicate": "equals"
        }))
        .unwrap();
        let err = dispatch_check(&wrong_pred, &ctx, &mut scope, None).unwrap_err();
        assert!(
            err.to_string()
                .contains("only supports predicate 'matches'"),
            "got: {err}"
        );
    }

    // ---------- domshot claims ----------

    #[test]
    fn domshot_passes_when_normalized_snapshots_match() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("snapshots")).unwrap();

        fs::write(
            sid_dir.join("baselines/s1.snap.txt"),
            "- heading \"Login\" [ref=@e1]\n- textbox \"User\" [ref=@e2]\n- button \"Go\" [ref=@e3]\n",
        )
        .unwrap();
        // Same tree, refs renumbered → normalized match.
        fs::write(
            run_dir.join("snapshots/s1.txt"),
            "- heading \"Login\" [ref=@e9]\n- textbox \"User\" [ref=@e7]\n- button \"Go\" [ref=@e4]\n",
        )
        .unwrap();

        let claim: Claim = serde_json::from_value(json!({
            "subject": { "domshot": "s1" },
            "predicate": "matches"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn domshot_normalizes_plain_ref_en_spelling() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("snapshots")).unwrap();

        // The daemon's real spelling is `ref=eN` (no @) — numbering still
        // shifts across session reuse, so it must normalize identically.
        fs::write(
            sid_dir.join("baselines/s1.snap.txt"),
            "- generic [ref=e1] clickable\n  - link \"Chat\" [ref=e27]\n  - link \"Editor\" [ref=e28]\n",
        )
        .unwrap();
        fs::write(
            run_dir.join("snapshots/s1.txt"),
            "- generic [ref=e101] clickable\n  - link \"Chat\" [ref=e145]\n  - link \"Editor\" [ref=e146]\n",
        )
        .unwrap();

        let claim: Claim = serde_json::from_value(json!({
            "subject": { "domshot": "s1" },
            "predicate": "matches"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        dispatch_check(&claim, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn domshot_fails_with_unified_diff_and_skip_drops_volatile_lines() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        let run_dir = tmp.path().join("run");
        fs::create_dir_all(sid_dir.join("baselines")).unwrap();
        fs::create_dir_all(run_dir.join("snapshots")).unwrap();

        fs::write(
            sid_dir.join("baselines/s1.snap.txt"),
            "- heading \"Dash\" [ref=@e1]\n- text \"generated 12:00\" [ref=@e2]\n- button \"Go\" [ref=@e3]\n",
        )
        .unwrap();
        fs::write(
            run_dir.join("snapshots/s1.txt"),
            "- heading \"Dash\" [ref=@e1]\n- text \"generated 13:37\" [ref=@e9]\n- button \"Go\" [ref=@e3]\n",
        )
        .unwrap();

        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(&run_dir),
        };
        // Without skip: the volatile line diffs → fail + diff artifact.
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "domshot": "s1" },
            "predicate": "matches"
        }))
        .unwrap();
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(
            err.to_string().contains("differs from baseline"),
            "got: {err}"
        );
        let diff = fs::read_to_string(run_dir.join("domshots-diff/s1.diff.txt")).unwrap();
        assert!(diff.contains("-- text \"generated 12:00\""), "{diff}");
        assert!(diff.contains("+- text \"generated 13:37\""), "{diff}");

        // With a skip regex on the volatile row: pass.
        let claim2: Claim = serde_json::from_value(json!({
            "subject": { "domshot": "s1", "skip": ["generated \\d+:\\d+"] },
            "predicate": "matches"
        }))
        .unwrap();
        dispatch_check(&claim2, &ctx, &mut scope, None).unwrap();
    }

    #[test]
    fn domshot_bails_without_baseline_or_run_dir() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let sid_dir = tmp.path().join("scenario");
        fs::create_dir_all(&sid_dir).unwrap();
        let claim: Claim = serde_json::from_value(json!({
            "subject": { "domshot": "s1" },
            "predicate": "matches"
        }))
        .unwrap();
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: None,
        };
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("run-step"), "got: {err}");
        let ctx2 = CheckContext {
            session: "s",
            scenario_dir: &sid_dir,
            run_dir: Some(tmp.path()),
        };
        let err2 = dispatch_check(&claim, &ctx2, &mut scope, None).unwrap_err();
        assert!(err2.to_string().contains("domshot-accept"), "got: {err2}");
    }

    // ---------- network claims ----------

    fn cap_req(id: &str, url: &str, method: &str, status: Option<i64>) -> CapturedRequest {
        CapturedRequest {
            request_id: id.to_string(),
            url: url.to_string(),
            method: method.to_string(),
            status,
            resource_type: None,
            mime_type: None,
            post_data: None,
            ws_frames: vec![],
        }
    }

    fn net_claim(j: serde_json::Value) -> Claim {
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn network_post_data_contains_narrows_by_body() {
        let mut reqs = vec![
            cap_req("1", "https://a/api/save", "POST", Some(200)),
            cap_req("2", "https://a/api/save", "POST", Some(200)),
        ];
        reqs[0].post_data = Some(r#"{"op":"deleteAll"}"#.into());
        reqs[1].post_data = Some(r#"{"op":"rename"}"#.into());
        let matcher: NetworkMatcher = serde_json::from_value(json!({
            "urlMatches": "/api/save",
            "postDataContains": "deleteAll"
        }))
        .unwrap();
        let matches = filter_by_post(
            matching_requests(Some(&Regex::new("/api/save").unwrap()), None, None, &reqs),
            matcher.post_data_contains.as_deref(),
            "unused-session",
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].request_id, "1");
    }

    #[test]
    fn network_matching_and_semantics() {
        let reqs = vec![
            cap_req(
                "1",
                "https://a/api/users?operationName=ListUsers",
                "GET",
                Some(200),
            ),
            cap_req("2", "https://a/api/users", "POST", Some(201)),
            cap_req("3", "https://a/static/logo.png", "GET", Some(200)),
        ];
        let m = |matcher: serde_json::Value| -> NetworkMatcher {
            serde_json::from_value(matcher).unwrap()
        };
        let url_re = |p: &str| Regex::new(p).unwrap();

        // url regex + method AND together
        let matches = matching_requests(
            Some(&url_re("/api/")),
            None,
            m(json!({"method":"GET"})).method.as_ref(),
            &reqs,
        );
        assert_eq!(matches.len(), 1);
        // operationName narrows to the GraphQL-flavored call
        let matches = matching_requests(None, Some("ListUsers"), None, &reqs);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].request_id, "1");
        let _ = m;

        // fired
        let mut scope = ValueScope::default();
        let mut fetch = |_: &str| -> Result<Json> { Ok(json!({"data":{"total":3}})) };
        let r: Vec<&CapturedRequest> = reqs.iter().collect();
        let pass = evaluate_network(
            &NetworkClaimKind::Fired,
            &r,
            None,
            &Predicate::Exists,
            None,
            &mut scope,
            &mut fetch,
        )
        .unwrap();
        assert!(matches!(pass, NetEval::Pass));
        let empty: Vec<&CapturedRequest> = vec![];
        let pass = evaluate_network(
            &NetworkClaimKind::Fired,
            &empty,
            None,
            &Predicate::NotExists,
            None,
            &mut scope,
            &mut fetch,
        )
        .unwrap();
        assert!(matches!(pass, NetEval::Pass));
        let pass = evaluate_network(
            &NetworkClaimKind::Fired,
            &r,
            None,
            &Predicate::NotExists,
            None,
            &mut scope,
            &mut fetch,
        )
        .unwrap();
        assert!(matches!(pass, NetEval::Pending(_)));

        // status — latest match wins
        let api: Vec<&CapturedRequest> = reqs.iter().take(2).collect();
        let pass = evaluate_network(
            &NetworkClaimKind::Status,
            &api,
            None,
            &Predicate::Equals,
            Some(&json!("201")),
            &mut scope,
            &mut fetch,
        )
        .unwrap();
        assert!(matches!(pass, NetEval::Pass));
        let pass = evaluate_network(
            &NetworkClaimKind::Status,
            &api,
            None,
            &Predicate::Gte,
            Some(&json!(500)),
            &mut scope,
            &mut fetch,
        )
        .unwrap();
        assert!(matches!(pass, NetEval::Pending(_)));

        // responseJsonPath — body fetched for the latest match only
        let pass = evaluate_network(
            &NetworkClaimKind::ResponseJsonPath,
            &api,
            Some("$.data.total"),
            &Predicate::Equals,
            Some(&json!(3)),
            &mut scope,
            &mut fetch,
        )
        .unwrap();
        assert!(matches!(pass, NetEval::Pass));
        let pass = evaluate_network(
            &NetworkClaimKind::ResponseJsonPath,
            &api,
            Some("$.data.missing"),
            &Predicate::Exists,
            None,
            &mut scope,
            &mut fetch,
        )
        .unwrap();
        assert!(matches!(pass, NetEval::Pending(_)));
    }

    #[test]
    fn network_claim_hard_validation() {
        let sid_dir = Path::new("/tmp");
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: sid_dir,
            run_dir: None,
        };
        // responseJsonPath without path → immediate bail
        let claim = net_claim(json!({
            "subject": {"network": {"urlMatches": "/api/"}, "ofKind": "responseJsonPath"},
            "predicate": "exists"
        }));
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("requires 'path'"), "got: {err}");
        // fired with a text predicate → immediate bail
        let claim = net_claim(json!({
            "subject": {"network": {"urlMatches": "/api/"}, "ofKind": "fired"},
            "predicate": "equals", "value": "x"
        }));
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(
            err.to_string().contains("does not support predicate"),
            "got: {err}"
        );
        // wsPayloadContains narrows to sockets carrying a matching frame
        {
            use super::*;
            let entries = [CapturedRequest {
                request_id: "cdpws-0".into(),
                url: "wss://echo.example/socket".into(),
                method: "WS".into(),
                status: Some(101),
                resource_type: Some("WebSocket".into()),
                mime_type: None,
                post_data: None,
                ws_frames: vec![
                    serde_json::json!({"dir": "received", "opcode": 1, "payload": "pong:hello"}),
                ],
            }];
            let got = filter_by_ws_payload(entries.iter().collect(), Some("pong:hello"));
            assert_eq!(got.len(), 1);
            let got = filter_by_ws_payload(entries.iter().collect(), Some("nope"));
            assert!(got.is_empty());
        }

        // bad urlMatches regex → immediate bail
        let claim = net_claim(json!({
            "subject": {"network": {"urlMatches": "([bad"}},
            "predicate": "exists"
        }));
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("not a valid regex"), "got: {err}");
    }

    #[test]
    fn element_count_claim_hard_validation() {
        let sid_dir = Path::new("/tmp");
        let mut scope = ValueScope::default();
        let ctx = CheckContext {
            session: "s",
            scenario_dir: sid_dir,
            run_dir: None,
        };
        let el_claim = |subject: serde_json::Value, rest: serde_json::Value| -> Claim {
            let mut m = serde_json::Map::new();
            m.insert("subject".into(), subject);
            if let serde_json::Value::Object(rest) = rest {
                m.extend(rest);
            }
            serde_json::from_value(serde_json::Value::Object(m)).unwrap()
        };
        let subject = || json!({"element": {"raw": {"kind": "css", "value": ".item"}, "reason": "test"}, "ofKind": "count"});
        // missing/blank value → bail before any browser call
        let claim = el_claim(subject(), json!({"predicate": "equals"}));
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("numeric 'value'"), "got: {err}");
        // string predicate → unsupported
        let claim = el_claim(subject(), json!({"predicate": "contains", "value": 3}));
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(
            err.to_string().contains("does not support predicate"),
            "got: {err}"
        );
        // ofKind:"attribute" without the attribute field → bail before any
        // browser call
        let claim: Claim = serde_json::from_value(json!({
            "subject": {"element": {"raw": {"kind": "css", "value": ".item"}, "reason": "test"}, "ofKind": "attribute"},
            "predicate": "equals", "value": "x"
        }))
        .unwrap();
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(
            err.to_string().contains("requires the 'attribute' field"),
            "got: {err}"
        );
        // ofKind:"text"/"value" route into the attribute-read path: they
        // bail on a missing `value` for value predicates, not on the ofKind
        let claim: Claim = serde_json::from_value(json!({
            "subject": {"element": {"raw": {"kind": "css", "value": ".item"}, "reason": "test"}, "ofKind": "text"},
            "predicate": "equals"
        }))
        .unwrap();
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("requires 'value'"), "got: {err}");
    }
}
