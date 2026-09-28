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

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde_json::Value as Json;

use crate::browser::{self, CapturedRequest, RoleAct};
use crate::scenario::{
    Claim, ClaimSubject, HttpMethod, Locator, NetworkClaimKind, NetworkMatcher, Predicate,
    RawLocatorKind,
};
use crate::value::{select_json_path, substitute_scenario_vars, value_to_string, ValueScope};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const MAX_TIMEOUT: Duration = Duration::from_secs(10);

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
        } => {
            if of_kind.is_some() {
                bail!("element claim with ofKind is not yet supported");
            }
            check_element(
                element,
                attribute.as_deref(),
                &claim.predicate,
                claim.value.as_ref(),
                ctx,
                scope,
                timeout,
            )
        }
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
        ClaimSubject::Shot { shot, clip } => check_shot(
            shot,
            clip.as_ref(),
            &claim.predicate,
            claim.tolerance.as_ref(),
            ctx,
            scope,
        ),
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
    let tol = tolerance
        .and_then(|t| t.get("pixels"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.01);
    let mut a = crate::compare::screenshots::decode_png(&baseline)?;
    let mut b = crate::compare::screenshots::decode_png(&current)?;
    if let Some(loc) = clip {
        // Crop BOTH images to the element's live box (CSS px → image px via
        // the screenshot's device-pixel scale). The rect is read now, at
        // claim time — keep the viewport pinned so record ≈ replay rects.
        let rect = clip_rect(ctx.session, loc, scope, b.width())?;
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
        bail!(
            "shot '{shot}' changed size — baseline {:?} vs current {:?}; re-mint with shot-accept if intentional",
            a.dimensions(),
            b.dimensions()
        );
    }
    let (frac, diff_img) = crate::compare::screenshots::pixel_diff(&a, &b);
    if frac <= tol {
        return Ok(());
    }
    let diff_dir = run_dir.join("shots-diff");
    std::fs::create_dir_all(&diff_dir).ok();
    let diff_path = diff_dir.join(format!("{shot}.diff.png"));
    let _ = diff_img.save(&diff_path);
    bail!(
        "shot '{shot}' differs from baseline: {:.2}% pixels changed (tolerance {:.2}%) — diff at {}",
        frac * 100.0,
        tol * 100.0,
        diff_path.display()
    )
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

    let deadline = Instant::now() + timeout;
    let mut pending: String;
    loop {
        match browser::network_requests(ctx.session) {
            Ok(reqs) => {
                let matches = matching_requests(
                    url_re.as_ref(),
                    op_name.as_deref(),
                    matcher.method.as_ref(),
                    &reqs,
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
            match read_element_attribute(ctx.session, loc, attribute, scope) {
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

fn read_element_attribute(
    session: &str,
    loc: &Locator,
    attribute: &str,
    scope: &mut ValueScope,
) -> Result<String> {
    match loc {
        Locator::Raw(raw) => {
            let v = substitute_scenario_vars(&raw.raw.value, scope);
            let selector = match &raw.raw.kind {
                RawLocatorKind::Css => v,
                RawLocatorKind::TestId => format!("[data-testid=\"{}\"]", v.replace('"', "\\\"")),
                other => {
                    bail!("element attribute claims do not support raw locator kind {other:?}")
                }
            };
            // `text` reads textContent; value/checked/disabled/selected/readOnly
            // read the live IDL property (getAttribute would return the stale
            // default value, and boolean states often have no attribute at all);
            // `focused` is `document.activeElement === el`; any other name is a
            // getAttribute read (missing attributes read as the empty string).
            let prop_attrs = [
                "value", "checked", "disabled", "selected", "readOnly", "required",
            ];
            let expr = if attribute == "text" {
                format!(
                    "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); return (el.textContent || '').trim(); }})()",
                    q = serde_json::to_string(&selector).expect("string serializes")
                )
            } else if attribute == "focused" {
                format!(
                    "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); return String(document.activeElement === el); }})()",
                    q = serde_json::to_string(&selector).expect("string serializes")
                )
            } else if prop_attrs.contains(&attribute) {
                format!(
                    "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); const v = el[{a}]; return v === undefined || v === null ? '' : String(v); }})()",
                    q = serde_json::to_string(&selector).expect("string serializes"),
                    a = serde_json::to_string(attribute).expect("string serializes")
                )
            } else {
                format!(
                    "(() => {{ const el = document.querySelector({q}); if (!el) throw new Error('selector not found: ' + {q}); return el.getAttribute({a}) || ''; }})()",
                    q = serde_json::to_string(&selector).expect("string serializes"),
                    a = serde_json::to_string(attribute).expect("string serializes")
                )
            };
            let raw = browser::eval_expression(session, &expr)?;
            Ok(decode_json_string(raw.trim()))
        }
        _ => bail!("element attribute claims currently require a raw css or testId locator"),
    }
}

fn locator_resolves(
    session: &str,
    loc: &Locator,
    scope: &mut ValueScope,
    scenario_dir: &std::path::Path,
) -> Result<()> {
    // Re-use the same locator → CLI mapping as dispatch_do uses for
    // its act calls; here we use Focus (the cheapest no-op-ish act
    // agent-browser exposes) just to confirm the element resolves.
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
            match browser::find_role_act_quiet(session, &role.role, name_str, RoleAct::Focus, None)
            {
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
                    browser::find_xpath_act(session, &v, RoleAct::Focus, None)?;
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

    fn clear_console() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
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
            ClaimSubject::Shot { shot, clip } => {
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

    // ---------- network claims ----------

    fn cap_req(id: &str, url: &str, method: &str, status: Option<i64>) -> CapturedRequest {
        CapturedRequest {
            request_id: id.to_string(),
            url: url.to_string(),
            method: method.to_string(),
            status,
            resource_type: None,
            mime_type: None,
        }
    }

    fn net_claim(j: serde_json::Value) -> Claim {
        serde_json::from_value(j).unwrap()
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
        // bad urlMatches regex → immediate bail
        let claim = net_claim(json!({
            "subject": {"network": {"urlMatches": "([bad"}},
            "predicate": "exists"
        }));
        let err = dispatch_check(&claim, &ctx, &mut scope, None).unwrap_err();
        assert!(err.to_string().contains("not a valid regex"), "got: {err}");
    }
}
