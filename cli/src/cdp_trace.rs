//! Chrome DevTools trace capture on a dedicated CDP socket.
//!
//! `{"perf": {"metric": "lcp", "trace": true}}` wraps the perf claim's
//! poll window in `Tracing.start`/`Tracing.end` and lands the collected
//! events at `<run>/perf/<metric>.trace.json` — the artifact a budget
//! miss needs for RCA (DevTools Perfetto-compatible; chrome-devtools-mcp
//! produces the same shape).
//!
//! Runs on its own ws connection like cdp_net — the pooled `cdp.rs`
//! socket stays command-only because a blocking event reader can't
//! multiplex replies and `Tracing.dataCollected` floods on one socket.
//! Best-effort throughout: no CDP endpoint (fake browser, old session)
//! means no trace and the claim is unaffected.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value as Json};

use crate::cdp::CdpConnection;

/// An in-progress trace — created by `start`, consumed by `stop`.
pub struct TraceGuard {
    conn: CdpConnection,
    page_sid: String,
}

/// Start tracing the session's active page. `Ok(None)` when CDP is
/// unavailable — tracing is additive, never a gate.
pub fn start(session: &str) -> Result<Option<TraceGuard>> {
    match start_in(session) {
        Err(e) if crate::cdp::is_unavailable(&e) => Ok(None),
        out => out,
    }
}

fn start_in(session: &str) -> Result<Option<TraceGuard>> {
    let url =
        crate::browser::cdp_url(session).map_err(|e| anyhow!("[unavailable] cdp url: {e}"))?;
    let mut conn = CdpConnection::connect(&url)?;
    let Some((target_id, _)) = crate::cdp::active_page(&mut conn)? else {
        return Ok(None);
    };
    let attached = conn.call(
        "Target.attachToTarget",
        json!({ "targetId": target_id, "flatten": true }),
    )?;
    let page_sid = attached
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Target.attachToTarget: no sessionId in {attached}"))?
        .to_string();
    conn.call_on(
        "Tracing.start",
        json!({
            "transferMode": "ReportEvents",
            // Default-ish category set for web perf RCA: renderer timing,
            // user timing marks/measures, and V8 execution cost.
            "categories": "devtools.timeline,blink.user_timing,v8.execute,disabled-by-default-v8.cpu_profiler",
            "options": "sampling-frequency=10000"
        }),
        &page_sid,
    )?;
    Ok(Some(TraceGuard { conn, page_sid }))
}

impl TraceGuard {
    /// Stop the trace and return every collected event (`traceEvents`-
    /// shaped values). Reads until `Tracing.tracingComplete` or a 15s
    /// ceiling — a wedged trace shouldn't hang a replay.
    pub fn stop(mut self) -> Result<Vec<Json>> {
        let _ = self.conn.send_on("Tracing.end", json!({}), &self.page_sid);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut events: Vec<Json> = Vec::new();
        loop {
            if std::time::Instant::now() >= deadline {
                bail!("cdp Tracing.tracingComplete timed out");
            }
            let ev = self.conn.next_event()?;
            let method = ev.get("method").and_then(|m| m.as_str()).unwrap_or("");
            match method {
                "Tracing.dataCollected" => {
                    if let Some(vals) = ev
                        .get("params")
                        .and_then(|p| p.get("value"))
                        .and_then(|v| v.as_array())
                    {
                        events.extend(vals.iter().cloned());
                    }
                }
                "Tracing.tracingComplete" => return Ok(events),
                _ => {}
            }
        }
    }
}
