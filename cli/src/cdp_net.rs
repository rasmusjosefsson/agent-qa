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
use std::collections::{HashMap, HashSet};
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
    /// The request came from a worker-class target (service/dedicated/
    /// shared worker) — invisible to the daemon's page-side capture.
    pub worker: bool,
}

#[derive(Default)]
struct Store {
    events: Vec<NetEvent>,
    /// `{sessionId}:{requestId}` → index into `events` for in-flight
    /// updates — requestIds are only unique per target session, so the
    /// session prefix keeps a page fetch and a worker fetch distinct.
    by_req: HashMap<String, usize>,
    /// sessionIds already given `Network.enable` (page + auto-attached
    /// worker targets) — dedupes the enable per attach event.
    enabled: HashSet<String>,
    /// sessionId → targetInfo.type for every attached session.
    session_types: HashMap<String, String>,
    /// sessionIds queued for `Network.enable` — filled by `record` on
    /// attach events (pure state, testable), drained by `reader_loop`,
    /// which owns the socket.
    pending_enables: Vec<String>,
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
    // Auto-attach every worker-class target as it appears: service-worker
    // fetches live on the worker's own session, invisible to the page
    // session we just enabled. Attach events land on THIS socket; the
    // reader loop issues `Network.enable` per attached session. Filtered
    // first; if a Chrome rejects the filter shape, retry unfiltered and
    // let `handle_attach` ignore non-network-worthy types.
    let attach = conn.call(
        "Target.setAutoAttach",
        json!({
            "autoAttach": true,
            "waitForDebuggerOnStart": false,
            "flatten": true,
            "filter": [
                { "type": "worker" }, { "type": "service_worker" },
                { "type": "shared_worker" }, { "type": "page" }
            ]
        }),
    );
    if attach.is_err() {
        let _ = conn.call(
            "Target.setAutoAttach",
            json!({ "autoAttach": true, "waitForDebuggerOnStart": false, "flatten": true }),
        );
    }
    {
        let mut map = stores().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = map.get_mut(session) {
            s.attached = true;
            s.enabled.insert(session_id.clone());
            s.session_types
                .insert(session_id.clone(), "page".to_string());
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
        drain_pending_enables(&mut conn, &session);
    }
}

/// Send `Network.enable` for sessions `record` queued on attach events.
/// Fire-and-forget (`send_on`): the response frames interleave with
/// events harmlessly while `call_on`'s wait would strand them.
fn drain_pending_enables(conn: &mut CdpConnection, session: &str) {
    let pending = {
        let mut map = stores().lock().unwrap_or_else(|e| e.into_inner());
        map.get_mut(session)
            .map(|s| std::mem::take(&mut s.pending_enables))
            .unwrap_or_default()
    };
    for sid in pending {
        if let Err(e) = conn.send_on("Network.enable", json!({}), &sid) {
            eprintln!("[v2-replay] cdp network capture: Network.enable on {sid}: {e:#}");
        }
    }
}

/// Target classes whose Network domain reports fetches we care about.
fn network_worthy(target_type: &str) -> bool {
    matches!(
        target_type,
        "page" | "worker" | "service_worker" | "shared_worker"
    )
}

fn record(session: &str, v: &Json) {
    let Some(params) = v.get("params") else {
        return;
    };
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    // Target lifecycle frames carry no requestId — handle them before
    // the request path. `record` only updates store state (queued
    // enables); `reader_loop` performs the socket send afterwards.
    if method == "Target.attachedToTarget" {
        let Some(sid) = params.get("sessionId").and_then(|s| s.as_str()) else {
            return;
        };
        let ttype = params
            .pointer("/targetInfo/type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let mut map = match stores().lock() {
            Ok(m) => m,
            Err(e) => e.into_inner(),
        };
        let Some(store) = map.get_mut(session) else {
            return;
        };
        store
            .session_types
            .insert(sid.to_string(), ttype.to_string());
        if network_worthy(ttype) && !store.enabled.contains(sid) {
            store.enabled.insert(sid.to_string());
            store.pending_enables.push(sid.to_string());
        }
        return;
    }
    if method == "Target.detachedFromTarget" {
        if let Some(sid) = params.get("sessionId").and_then(|s| s.as_str()) {
            let mut map = stores().lock().unwrap_or_else(|e| e.into_inner());
            if let Some(store) = map.get_mut(session) {
                store.enabled.remove(sid);
                store.session_types.remove(sid);
                store.pending_enables.retain(|p| p != sid);
            }
        }
        return;
    }
    let request_id = params
        .get("requestId")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    if request_id.is_empty() {
        return;
    }
    // Flat-mode frames carry their session at top level — the composite
    // key keeps a worker's requestId from colliding with the page's.
    let frame_sid = v.get("sessionId").and_then(|s| s.as_str()).unwrap_or("");
    let req_key = format!("{frame_sid}:{request_id}");
    let mut map = match stores().lock() {
        Ok(m) => m,
        Err(e) => e.into_inner(),
    };
    let Some(store) = map.get_mut(session) else {
        return;
    };
    let worker = !frame_sid.is_empty()
        && store
            .session_types
            .get(frame_sid)
            .is_some_and(|t| t != "page");
    match method {
        "Network.requestWillBeSent" => {
            // A redirectResponse means the *previous* exchange for this
            // requestId ended in a redirect — stamp it as a finished
            // redirect hop with the status the daemon never saw.
            if let Some(rr) = params.get("redirectResponse") {
                if let Some(&idx) = store.by_req.get(&req_key) {
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
                    worker,
                });
                store.by_req.insert(req_key, idx);
            }
        }
        "Network.responseReceived" => {
            if let Some(&idx) = store.by_req.get(&req_key) {
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
            if let Some(&idx) = store.by_req.get(&req_key) {
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
                    ws_frames: vec![],
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Requests issued by worker-class targets — service workers, dedicated
/// and shared workers — which the daemon's page-side capture can never
/// see. Merged into `network_requests` so `{"network"}` claims and
/// `wait url` cover SW fetches the app performs (offline caches, sync
/// workers, fetch proxies).
pub fn worker_entries(session: &str) -> Vec<crate::browser::CapturedRequest> {
    ensure(session);
    let map = stores().lock().unwrap_or_else(|e| e.into_inner());
    map.get(session)
        .map(|s| {
            s.events
                .iter()
                .filter(|e| e.worker)
                .map(|e| crate::browser::CapturedRequest {
                    request_id: format!("cdp-sw-{}", e.request_id),
                    url: e.url.clone(),
                    method: e.method.clone(),
                    status: e.status,
                    resource_type: Some("worker".into()),
                    mime_type: None,
                    post_data: None,
                    ws_frames: vec![],
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

    #[test]
    fn worker_session_requests_are_tagged_and_listed() {
        feed(
            "s3",
            vec![
                // Page session attach — no worker tag for its events.
                json!({"method":"Target.attachedToTarget","params":{
                    "sessionId":"P1",
                    "targetInfo":{"type":"page"}
                }}),
                json!({"sessionId":"P1","method":"Network.requestWillBeSent","params":{
                    "requestId":"R1",
                    "request":{"url":"https://x/app","method":"GET"}
                }}),
                // A service worker attaches and fetches on its own session.
                json!({"method":"Target.attachedToTarget","params":{
                    "sessionId":"W1",
                    "targetInfo":{"type":"service_worker"}
                }}),
                json!({"sessionId":"W1","method":"Network.requestWillBeSent","params":{
                    "requestId":"R1",
                    "request":{"url":"https://x/api/cached","method":"GET"}
                }}),
                json!({"sessionId":"W1","method":"Network.responseReceived","params":{
                    "requestId":"R1",
                    "response":{"url":"https://x/api/cached","status":200}
                }}),
                json!({"sessionId":"W1","method":"Network.loadingFinished","params":{
                    "requestId":"R1"
                }}),
            ],
        );
        // Same requestId "R1" on both sessions — the composite key keeps
        // the worker's fetch separate instead of clobbering the page's.
        let workers = worker_entries("s3");
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].url, "https://x/api/cached");
        assert_eq!(workers[0].status, Some(200));
        assert_eq!(workers[0].resource_type.as_deref(), Some("worker"));
        // wait url resolves on the worker's request too.
        let re = regex::Regex::new("api/cached").unwrap();
        assert!(find_completed("s3", &re));
        // Both attaches queued a Network.enable — reader_loop drains
        // them; the queue proves the worker's session is covered.
        let map = stores().lock().unwrap();
        let s = map.get("s3").unwrap();
        assert_eq!(s.pending_enables, vec!["P1".to_string(), "W1".to_string()]);
    }

    #[test]
    fn detached_worker_stops_being_marked() {
        feed(
            "s4",
            vec![
                json!({"method":"Target.attachedToTarget","params":{
                    "sessionId":"W9",
                    "targetInfo":{"type":"worker"}
                }}),
                json!({"method":"Target.detachedFromTarget","params":{
                    "sessionId":"W9"
                }}),
                // A stray post-detach event must not be tagged worker.
                json!({"sessionId":"W9","method":"Network.requestWillBeSent","params":{
                    "requestId":"R2",
                    "request":{"url":"https://x/late","method":"GET"}
                }}),
            ],
        );
        assert!(worker_entries("s4").is_empty());
        let map = stores().lock().unwrap();
        let s = map.get("s4").unwrap();
        assert!(s.pending_enables.is_empty());
    }
}
