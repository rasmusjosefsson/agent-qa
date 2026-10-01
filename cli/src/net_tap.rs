//! In-flight network instrumentation — a tiny always-on init script that
//! counts pending fetch/XHR calls per document (`window.__aqNet`), plus
//! the replay-side settle that consults it.
//!
//! Why: agent-browser's `wait --load networkidle` has a ~700ms floor (its
//! idle window), so every post-click settle paid ~700ms even when the
//! click loaded nothing. With the tap, `settle_after_click` probes
//! `__aqNet.pending` twice over a short grace window: a quiet page skips
//! the wait entirely; a busy or navigating page falls back to the same
//! networkidle wait as before.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use crate::browser;

/// Page-init script: wraps `fetch` and `XMLHttpRequest.send` with an
/// in-flight counter. Idempotent per document; each navigation re-installs
/// it fresh (that's what makes `performance.timeOrigin` a navigation
/// marker — see `settle_after_click`).
pub(crate) const TAP_JS: &str = r#"(() => {
  if (window.__aqNet) return;
  window.__aqNet = { pending: 0 };
  const up = () => { window.__aqNet.pending += 1; };
  const down = () => { window.__aqNet.pending = Math.max(0, window.__aqNet.pending - 1); };
  const of = window.fetch;
  if (typeof of === 'function') {
    window.fetch = function (...a) {
      up();
      return of.apply(this, a).finally(down);
    };
  }
  const osend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function (...a) {
    up();
    this.addEventListener('loadend', down, { once: true });
    return osend.apply(this, a);
  };
})();"#;

/// Probe evaluated after a click. Returns `<timeOrigin>|<pending>` or an
/// empty string when the tap isn't installed (warm session launched before
/// the tap existed, cross-origin frame ctx the init script never reached).
const SETTLE_PROBE: &str = concat!(
    "(window.__aqNet && typeof __aqNet.pending === 'number')",
    " ? performance.timeOrigin + '|' + __aqNet.pending : ''",
);

/// Grace window between the two probes. A click's handler may defer its
/// request by a tick (`onclick → setTimeout(()=>fetch)`), so a single
/// immediate probe would call an about-to-fire page idle.
const GRACE_MS: u64 = 150;

/// Write the tap init script at `<dir>/net-tap-init.js`. Registered via
/// `AGENT_BROWSER_INIT_SCRIPTS` (comma-separated — merge with the mock
/// init and any pre-set paths), it installs before every navigation.
pub(crate) fn write_init_script(dir: &Path) -> Result<PathBuf> {
    let path = dir.join("net-tap-init.js");
    fs::write(&path, TAP_JS).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Compose the `AGENT_BROWSER_INIT_SCRIPTS` value from existing paths
/// (anything the caller or environment already set — e.g. the mock init
/// script) plus `extra`. Order preserved; empties dropped.
pub(crate) fn merge_init_scripts(existing: Option<&str>, extra: &[&Path]) -> String {
    let mut parts: Vec<String> = existing
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for p in extra {
        parts.push(p.display().to_string());
    }
    parts.join(",")
}

/// Post-click settle: skip the ~700ms `networkidle` floor when the page
/// is provably quiet — tap present, zero pending requests, and no
/// navigation (same `performance.timeOrigin`) across the grace window.
/// Anything else — no tap, in-flight requests, a crossed navigation, a
/// failed probe — waits `networkidle`, bounded like every other
/// post-navigation settle so a beacon-heavy page can't stall the run.
pub(crate) fn settle_after_click(session: &str) -> Result<()> {
    let first = net_probe(session);
    sleep(Duration::from_millis(GRACE_MS));
    let second = net_probe(session);

    match (first, second) {
        (Some((to1, p1)), Some((to2, p2))) if to1 == to2 && p1 == 0 && p2 == 0 => Ok(()),
        _ => Ok(browser::wait_for_load_capped(
            session,
            "networkidle",
            browser::LOAD_CAP_MS,
        )?),
    }
}

/// One tap probe: `Some((performance.timeOrigin, pending))`, `None` when
/// the tap isn't installed or the probe eval failed.
fn net_probe(session: &str) -> Option<(String, u64)> {
    let out = browser::eval_expression(session, SETTLE_PROBE).ok()?;
    let raw = unquote(out.trim());
    if raw.is_empty() {
        return None;
    }
    let (to, p) = raw.split_once('|')?;
    Some((to.to_string(), p.parse().ok()?))
}

/// Eval stdout is a JSON-encoded scalar — unwrap `"..."` when quoted,
/// pass raw through otherwise (same rule as claims' decode_json_string).
fn unquote(raw: &str) -> String {
    match serde_json::from_str::<String>(raw) {
        Ok(s) => s,
        Err(_) => raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_js_wraps_fetch_and_xhr() {
        assert!(TAP_JS.contains("window.fetch = function"));
        assert!(TAP_JS.contains("XMLHttpRequest.prototype.send = function"));
        assert!(TAP_JS.contains("__aqNet = { pending: 0 }"));
        // Idempotent — re-installed per document without stacking.
        assert!(TAP_JS.contains("if (window.__aqNet) return"));
    }

    #[test]
    fn merge_init_scripts_appends_after_existing() {
        let tap = Path::new("/run/net-tap-init.js");
        let mock = Path::new("/run/mock-init.js");
        assert_eq!(merge_init_scripts(None, &[tap]), "/run/net-tap-init.js",);
        assert_eq!(
            merge_init_scripts(Some("/run/mock-init.js"), &[tap]),
            "/run/mock-init.js,/run/net-tap-init.js",
        );
        assert_eq!(
            merge_init_scripts(Some(" /a.js , , /b.js "), &[tap, mock]),
            "/a.js,/b.js,/run/net-tap-init.js,/run/mock-init.js",
        );
    }

    #[test]
    fn unquote_decodes_quoted_and_raw() {
        assert_eq!(unquote("\"123|0\""), "123|0");
        assert_eq!(unquote(""), "");
        assert_eq!(unquote("raw"), "raw");
    }
}
