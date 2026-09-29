//! Per-session `Network.*` event capture on a dedicated CDP socket.
//!
//! The daemon's netlog (`agent-browser network requests`) only surfaces
//! the final response of a request chain — redirect hops are invisible,
//! so a POST → 302 → GET records the 302's status as missing. Owning the
//! Network domain on a *second* ws connection (the pooled socket in
//! `cdp.rs` stays command-only — a blocking reader can't multiplex
//! events and replies on one socket) recovers those hops:
//! `requestWillBeSent` carries the previous hop's `redirectResponse`.
//!
//! `start` is called once per replay run (runner) and is best-effort —
//! no CDP endpoint (fake browser in tests) means no capture, and
//! `network_requests` falls back to the daemon list alone.

use anyhow::{anyhow, Result};
use serde_json::{json, Value as Json};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::cdp::CdpConnection;

/// One tracked exchange, synthesized from Network domain events.
#[derive(Debug, Clone)]
pub struct NetEvent {
    pub request_id: String,
    pub url: String,
    pub method: String,
    /// Response status once `responseReceived` (or the following
    /// `requestWillBeSent` hop's `redirectResponse`) reports it.
    pub status: Option<i64>,
    /// `loadingFinished`/`loadingFailed` seen — safe to treat as done.
    pub finished: bool,
    /// This entry represents a redirect hop — the response that caused
    /// the follow-up request. The daemon never lists these.
    pub redirect: bool,
}

#[derive(Default)]
struct Store {
    events: Vec<NetEvent>,
    /// requestId → index into `events` for in-flight updates.
    by_req: HashMap<String, usize>,
    /// A reader thread is attached to a page target. False until the
    /// first successful attach — a run that starts before any page exists
    /// retries lazily from the read paths below.
    attached: bool,
    /// Lazy-attach attempts so a dead endpoint doesn't retry per read.
    attach_attempts: u8,
}

static STORES: OnceLock<Mutex<HashMap<String, Store>>> = OnceLock::new();

fn stores() -> &'static Mutex<HashMap<String, Store>> {
    STORES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Start a fresh capture for the session: attach to the active page on a
/// dedicated socket, `Network.enable`, then a reader thread owns the
/// socket and feeds the store until the socket dies. Always leaves a
/// (possibly empty) store — failures degrade silently.
pub fn start(session: &str) {
    {
        let mut map = stores().lock().unwrap_or_else(|e| e.into_inner());
        map.insert(session.to_string(), Store::default());
    }
    if let Err(e) = start_in(session) {
        if !crate::cdp::is_unavailable(&e) {
            eprintln!("[v2-replay] cdp network capture skipped: {e:#}");
        }
    }
}

fn start_in(session: &str) -> Result<()> {
    let url =
        crate::browser::cdp_url(session).map_err(|e| anyhow!("[unavailable] cdp url: {e}"))?;
    let mut conn = CdpConnection::connect(&url)?;
    let Some((target_id, _)) = crate::cdp::active_page(&mut conn)? else {
        // No page yet (fresh session, first goto pending) — `ensure`
        // retries this attach from the read paths once a page exists.
        return Ok(());
    };
    let attached = conn.call(
        "Target.attachToTarget",
        json!({ "targetId": target_id, "flatten": true }),
    )?;
    let session_id = attached
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Target.attachToTarget: no sessionId in {attached}"))?
        .to_string();
    conn.call_on("Network.enable", json!({}), &session_id)?;
    {
        let mut map = stores().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = map.get_mut(session) {
            s.attached = true;
        }
    }
    let name = session.to_string();
    std::thread::spawn(move || reader_loop(conn, name));
    Ok(())
}

/// Attach lazily — `start` runs before the session's first page exists
/// on cold launches, so read paths retry the attach once a page is up.
fn ensure(session: &str) {
    let needs = {
        let mut map = stores().lock().unwrap_or_else(|e| e.into_inner());
        match map.get_mut(session) {
            // ~6s of `wait url` polling at 150ms ticks before giving up —
            // claims still work via resource timing either way.
            Some(s) if !s.attached && s.attach_attempts < 40 => {
                // Optimistically mark so a slow/hung attempt doesn't pile
                // up retries; failure resets `attached` below.
                s.attached = true;
                s.attach_attempts += 1;
                true
            }
            _ => false,
        }
    };
    if needs {
        if let Err(e) = start_in(session) {
            let mut map = stores().lock().unwrap_or_else(|e| e.into_inner());
            if let Some(s) = map.get_mut(session) {
                s.attached = false;
            }
            if !crate::cdp::is_unavailable(&e) {
                eprintln!("[v2-replay] cdp network capture attach failed: {e:#}");
            }
        }
    }
}

fn reader_loop(mut conn: CdpConnection, session: String) {
    while let Ok(v) = conn.next_event() {
        record(&session, &v);
    }
}

fn record(session: &str, v: &Json) {
    let Some(params) = v.get("params") else {
        return;
    };
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let request_id = params
        .get("requestId")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    if request_id.is_empty() {
        return;
    }
    let mut map = match stores().lock() {
        Ok(m) => m,
        Err(e) => e.into_inner(),
    };
    let Some(store) = map.get_mut(session) else {
        return;
    };
    match method {
        "Network.requestWillBeSent" => {
            // A redirectResponse means the *previous* exchange for this
            // requestId ended in a redirect — stamp it as a finished
            // redirect hop with the status the daemon never saw.
            if let Some(rr) = params.get("redirectResponse") {
                if let Some(&idx) = store.by_req.get(&request_id) {
                    let hop = &mut store.events[idx];
                    hop.status = rr.get("status").and_then(|s| s.as_i64());
                    hop.finished = true;
                    hop.redirect = true;
                    if let Some(u) = rr.get("url").and_then(|u| u.as_str()) {
                        hop.url = u.to_string();
                    }
                }
            }
            if let Some(req) = params.get("request") {
                let idx = store.events.len();
                store.events.push(NetEvent {
                    request_id: request_id.clone(),
                    url: req
                        .get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string(),
                    method: req
                        .get("method")
                        .and_then(|m| m.as_str())
                        .unwrap_or("")
                        .to_string(),
                    status: None,
                    finished: false,
                    redirect: false,
                });
                store.by_req.insert(request_id, idx);
            }
        }
        "Network.responseReceived" => {
            if let Some(&idx) = store.by_req.get(&request_id) {
                if let Some(resp) = params.get("response") {
                    let e = &mut store.events[idx];
                    e.status = resp.get("status").and_then(|s| s.as_i64());
                    if let Some(u) = resp.get("url").and_then(|u| u.as_str()) {
                        e.url = u.to_string();
                    }
                }
            }
        }
        "Network.loadingFinished" | "Network.loadingFailed" => {
            if let Some(&idx) = store.by_req.get(&request_id) {
                store.events[idx].finished = true;
            }
        }
        _ => {}
    }
}

/// Redirect-hop exchanges the daemon's netlog never lists, as
/// `CapturedRequest`s for merging into `network_requests`.
pub fn redirect_entries(session: &str) -> Vec<crate::browser::CapturedRequest> {
    ensure(session);
    let map = stores().lock().unwrap_or_else(|e| e.into_inner());
    map.get(session)
        .map(|s| {
            s.events
                .iter()
                .filter(|e| e.redirect)
                .map(|e| crate::browser::CapturedRequest {
                    request_id: format!("cdp-{}", e.request_id),
                    url: e.url.clone(),
                    method: e.method.clone(),
                    status: e.status,
                    resource_type: None,
                    mime_type: None,
                    post_data: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// First event whose URL matches `re` and has finished — used by
/// `wait url` so a request still in flight (or a redirect hop resource
/// timing never lists) can be waited on through the event stream.
pub fn find_completed(session: &str, re: &regex::Regex) -> bool {
    ensure(session);
    let map = stores().lock().unwrap_or_else(|e| e.into_inner());
    map.get(session)
        .map(|s| s.events.iter().any(|e| e.finished && re.is_match(&e.url)))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn feed(session: &str, events: Vec<Json>) {
        {
            let mut map = stores().lock().unwrap();
            map.insert(session.to_string(), Store::default());
        }
        for v in &events {
            record(session, v);
        }
    }

    #[test]
    fn redirect_hop_records_status_and_new_request() {
        feed(
            "s1",
            vec![
                json!({"method":"Network.requestWillBeSent","params":{
                    "requestId":"R1",
                    "request":{"url":"https://x/login","method":"POST"}
                }}),
                // Second hop: R1's response was a 302, request continues.
                json!({"method":"Network.requestWillBeSent","params":{
                    "requestId":"R1",
                    "redirectResponse":{"url":"https://x/login","status":302},
                    "request":{"url":"https://x/home","method":"GET"}
                }}),
                json!({"method":"Network.responseReceived","params":{
                    "requestId":"R1",
                    "response":{"url":"https://x/home","status":200}
                }}),
                json!({"method":"Network.loadingFinished","params":{
                    "requestId":"R1"
                }}),
            ],
        );
        let redirects = redirect_entries("s1");
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].url, "https://x/login");
        assert_eq!(redirects[0].status, Some(302));
        assert_eq!(redirects[0].method, "POST");
        // The follow-up request finished → wait url resolves via events.
        let re = regex::Regex::new("x/home").unwrap();
        assert!(find_completed("s1", &re));
        let re_redirect = regex::Regex::new("x/login").unwrap();
        assert!(find_completed("s1", &re_redirect));
    }

    #[test]
    fn plain_requests_have_no_redirect_entries() {
        feed(
            "s2",
            vec![
                json!({"method":"Network.requestWillBeSent","params":{
                    "requestId":"R9",
                    "request":{"url":"https://x/api","method":"GET"}
                }}),
                json!({"method":"Network.loadingFinished","params":{
                    "requestId":"R9"
                }}),
            ],
        );
        assert!(redirect_entries("s2").is_empty());
        let re = regex::Regex::new("x/api").unwrap();
        assert!(find_completed("s2", &re));
    }
}
