//! Minimal blocking CDP client on the session's browser-level WebSocket.
//!
//! agent-browser owns the page-session commands we drive verbs through, but
//! a few browser-domain capabilities have no CLI surface — notably
//! `Browser.setPermission`, which an emulation that sets a geolocation
//! override needs before `navigator.geolocation` will answer. `cdp-url`
//! returns the browser endpoint, and Chrome multiplexes many ws clients on
//! it, so we attach our own without disturbing the daemon's session.
//!
//! The client is deliberately narrow: synchronous, one outstanding command
//! at a time. Incoming frames that are *events* (carry `method`, no `id`)
//! are buffered, not dropped — `enable_network_capture` turns on
//! `Network.webSocket*` notifications on the page session, and
//! `ws_entries` folds them into request-shaped entries that merge into
//! `network_requests`, so WebSocket/SSE exchanges become claimable and
//! land in `network.json` like HTTP ones (the daemon's own capture only
//! sees fetch/XHR).
//!
//! No TLS — the endpoint is loopback-only.
//!
//! Connections are pooled per session name and kept open for the process
//! lifetime — this is load-bearing, not an optimization: Chrome discards
//! grants and emulation overrides sent over a ws client when that client
//! disconnects, so a fire-and-forget connection leaves the page
//! un-emulated by the time the next step runs.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value as Json};
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

/// State for `Network.*` event capture on the page's flat session:
/// the flat `sessionId` plus each open WebSocket's entry, keyed by CDP
/// requestId. Entries accumulate for the run's lifetime (cleared at run
/// start like the daemon's own request log).
#[derive(Default)]
struct CaptureState {
    _page_sid: String,
    entries: Vec<CapEntry>,
}

/// A captured exchange the daemon's fetch/XHR hook can't see: WebSocket
/// sockets (with their frames) and SSE/EventSource streams.
#[derive(Debug)]
struct CapEntry {
    request_id: String,
    url: String,
    method: String,
    status: Option<i64>,
    kind: &'static str,
    frames: Vec<Json>,
}

pub struct CdpConnection {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: AtomicU64,
    events: Vec<Json>,
    capture: Option<CaptureState>,
}

impl CdpConnection {
    /// Connect to a `ws://…/devtools/browser/…` endpoint (from
    /// `browser::cdp_url`). Returns an error on non-browser targets — only
    /// browser-domain methods are expected through this client.
    pub fn connect(ws_url: &str) -> Result<Self> {
        if !ws_url.starts_with("ws://") && !ws_url.starts_with("wss://") {
            bail!("[unavailable] cdp url must be ws:// or wss://, got {ws_url:?}");
        }
        let (ws, _) = tungstenite::client::connect(ws_url)
            .map_err(|e| anyhow!("[unavailable] cdp connect {ws_url}: {e}"))?;
        Ok(Self {
            ws,
            next_id: AtomicU64::new(1),
            events: Vec::new(),
            capture: None,
        })
    }

    /// Send a command and wait for its response. Event frames and
    /// replies for other ids are buffered/skipped — callers issue
    /// commands serially.
    pub fn call(&mut self, method: &str, params: Json) -> Result<Json> {
        self.send_and_wait(method, params, None)
    }

    /// Like `call` but scoped to a target session (flat-mode `sessionId`)
    /// for commands a page target must handle, e.g. `Emulation.*`.
    pub fn call_on(&mut self, method: &str, params: Json, session_id: &str) -> Result<Json> {
        self.send_and_wait(method, params, Some(session_id))
    }

    fn send_and_wait(
        &mut self,
        method: &str,
        params: Json,
        session_id: Option<&str>,
    ) -> Result<Json> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = match session_id {
            Some(sid) => json!({ "id": id, "method": method, "params": params, "sessionId": sid }),
            None => json!({ "id": id, "method": method, "params": params }),
        };
        self.ws
            .send(Message::Text(msg.to_string().into()))
            .map_err(|e| anyhow!("[transport] cdp send: {e}"))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if std::time::Instant::now() >= deadline {
                bail!("cdp {method} timed out");
            }
            let frame = match self.ws.read() {
                Ok(f) => f,
                Err(e) => bail!("[transport] cdp read: {e}"),
            };
            let text = match frame {
                Message::Text(t) => t,
                Message::Close(_) => bail!("[transport] cdp socket closed waiting for {method}"),
                _ => continue,
            };
            let v: Json = serde_json::from_str(&text).context("cdp response json")?;
            if v.get("id").and_then(|i| i.as_u64()) != Some(id) {
                if v.get("method").is_some() {
                    self.events.push(v);
                }
                continue;
            }
            if let Some(err) = v.get("error") {
                let msg = err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown");
                bail!("cdp {method} failed: {msg}");
            }
            return Ok(v.get("result").cloned().unwrap_or(Json::Null));
        }
    }

    fn tcp(&self) -> Option<&TcpStream> {
        #[allow(unreachable_patterns)]
        match self.ws.get_ref() {
            MaybeTlsStream::Plain(s) => Some(s),
            _ => None,
        }
    }

    /// Pull whatever event frames are sitting in the socket buffer into
    /// `self.events`, without blocking: a short read timeout turns
    /// WouldBlock/TimedOut into "nothing buffered".
    fn drain_events(&mut self) -> Result<()> {
        let stream = self
            .tcp()
            .ok_or_else(|| anyhow!("[transport] cdp drain: not a tcp stream"))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_millis(60)))
            .map_err(|e| anyhow!("[transport] cdp drain set timeout: {e}"))?;
        let result = loop {
            match self.ws.read() {
                Ok(Message::Text(t)) => {
                    if let Ok(v) = serde_json::from_str::<Json>(&t) {
                        if v.get("method").is_some() {
                            self.events.push(v);
                        }
                    }
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break Ok(())
                }
                Err(e) => break Err(anyhow!("[transport] cdp drain read: {e}")),
            }
        };
        // Restore blocking reads for send_and_wait.
        if let Some(s) = self.tcp() {
            let _ = s.set_read_timeout(None);
        }
        result
    }
}

static CONNECTIONS: OnceLock<Mutex<HashMap<String, CdpConnection>>> = OnceLock::new();

fn connections() -> &'static Mutex<HashMap<String, CdpConnection>> {
    CONNECTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether `e` means "no CDP endpoint reachable" — the fake browsers used
/// in unit tests and a not-yet-launched session both land here. CDP
/// helpers degrade to their daemon fallbacks on it rather than failing.
fn is_unavailable(e: &anyhow::Error) -> bool {
    let s = format!("{e:#}");
    s.contains("[unavailable]")
}

/// Borrow the session's pooled CDP connection (connecting lazily) and run
/// `f` on it. The ws stays open after the call so grants/overrides survive
/// for the rest of the process. A dead socket (browser restarted) evicts
/// the entry and errors — the next call reconnects.
pub fn with_connection<T>(
    session: &str,
    f: impl FnOnce(&mut CdpConnection) -> Result<T>,
) -> Result<T> {
    let url =
        crate::browser::cdp_url(session).map_err(|e| anyhow!("[unavailable] cdp url: {e}"))?;
    let mut map = connections()
        .lock()
        .map_err(|_| anyhow!("cdp pool lock poisoned"))?;
    if !map.contains_key(session) {
        map.insert(session.to_string(), CdpConnection::connect(&url)?);
    }
    let conn = map.get_mut(session).expect("just inserted");
    let out = f(conn);
    if let Err(e) = &out {
        // Evict on transport failure so the next call reconnects rather
        // than reusing a corpse; protocol errors keep the connection (it
        // may carry live grants/overrides).
        if e.to_string().contains("[transport]") {
            map.remove(session);
        }
    }
    out
}

/// The active page target — last non-chrome page the browser reports
/// (`chrome://newtab`-style leftovers sort first in practice and must not
/// win).
fn active_page(conn: &mut CdpConnection) -> Result<Option<(String, String)>> {
    let targets = conn.call("Target.getTargets", json!({}))?;
    Ok(targets
        .get("targetInfos")
        .and_then(|t| t.as_array())
        .and_then(|infos| {
            infos.iter().rfind(|t| {
                if t.get("type").and_then(|v| v.as_str()) != Some("page") {
                    return false;
                }
                let u = t.get("url").and_then(|v| v.as_str()).unwrap_or("");
                !u.starts_with("chrome://") && !u.starts_with("devtools://") && !u.is_empty()
            })
        })
        .and_then(|t| {
            Some((
                t.get("targetId")?.as_str()?.to_string(),
                t.get("url")?.as_str()?.to_string(),
            ))
        }))
}

/// The http(s) origin of the session's active page, for origin-scoped
/// permission grants. None when there is no page or it isn't http(s)
/// (geolocation/permission APIs don't resolve on `file:`/`chrome:` anyway).
pub fn active_page_origin(session: &str) -> Result<Option<String>> {
    match active_page_origin_in(session) {
        Err(e) if is_unavailable(&e) => Ok(None),
        out => out,
    }
}

fn active_page_origin_in(session: &str) -> Result<Option<String>> {
    with_connection(session, |conn| {
        let page = match active_page(conn)? {
            Some((_, url)) => url,
            None => return Ok(None),
        };
        let origin = page.split('/').take(3).collect::<Vec<_>>().join("/");
        Ok((origin.starts_with("http://") || origin.starts_with("https://")).then_some(origin))
    })
}

/// Grant permissions via `Browser.setPermission`. The daemon's pages run
/// in a synthetic browser context whose reported id the Browser domain
/// can't resolve, and *unscoped* grants (`Browser.grantPermissions` or a
/// context-less `setPermission`) silently no-op — all verified empirically
/// on headless Chrome 137. The working variant is origin-scoped: pass the
/// page's origin; without one (no page loaded yet) the grant is skipped
/// and `false` returned. Errors surface — a silent skip would leave
/// emulations half-applied.
pub fn grant_permissions(
    session: &str,
    permissions: &[&str],
    origin: Option<&str>,
) -> Result<bool> {
    let Some(origin) = origin else {
        return Ok(false);
    };
    match grant_permissions_in(session, permissions, origin) {
        Err(e) if is_unavailable(&e) => Ok(false),
        out => out,
    }
}

fn grant_permissions_in(session: &str, permissions: &[&str], origin: &str) -> Result<bool> {
    with_connection(session, |conn| {
        for name in permissions {
            conn.call(
                "Browser.setPermission",
                json!({
                    "permission": { "name": name },
                    "setting": "granted",
                    "origin": origin,
                }),
            )?;
        }
        Ok(true)
    })
}

/// Apply `Emulation.setGeolocationOverride` on the session's page target.
/// `agent-browser set geo` scopes the override to whatever target the
/// daemon session points at — with a leftover `chrome://newtab` page that
/// is often the wrong one, leaving the real page at granted-but-no-override
/// (a hanging `getCurrentPosition`). Here we pick the active page target
/// (last non-chrome page) and send the override on our own flat session.
/// Returns false when no usable page target exists (emulate before the
/// first goto) — the caller falls back to the daemon's `set geo`.
pub fn set_geo_override(session: &str, lat: f64, lng: f64, accuracy: f64) -> Result<bool> {
    match set_geo_override_in(session, lat, lng, accuracy) {
        Err(e) if is_unavailable(&e) => Ok(false),
        out => out,
    }
}

fn set_geo_override_in(session: &str, lat: f64, lng: f64, accuracy: f64) -> Result<bool> {
    emulate_override_in(
        session,
        "Emulation.setGeolocationOverride",
        json!({ "latitude": lat, "longitude": lng, "accuracy": accuracy }),
    )
}

/// Apply a page-target `Emulation.*` command (timezone, locale, …) over a
/// flat session attached to the ACTIVE page — same lifetime rules as
/// `set_geo_override`: the override dies with this pooled ws connection
/// and binds per navigation on that target. Returns Ok(false) when no
/// page target exists so the caller decides between a daemon fallback
/// (geo) or an actionable error (keys with no daemon equivalent).
pub fn emulate_override(session: &str, method: &str, params: Json) -> Result<bool> {
    match emulate_override_in(session, method, params) {
        Err(e) if is_unavailable(&e) => Ok(false),
        out => out,
    }
}

fn emulate_override_in(session: &str, method: &str, params: Json) -> Result<bool> {
    with_connection(session, |conn| {
        let page = active_page(conn)?.map(|(id, _)| id);
        let Some(target_id) = page else {
            return Ok(false);
        };
        let sid = attach_flat(conn, &target_id)?;
        conn.call_on(method, params, &sid)?;
        Ok(true)
    })
}

fn attach_flat(conn: &mut CdpConnection, target_id: &str) -> Result<String> {
    let attached = conn.call(
        "Target.attachToTarget",
        json!({ "targetId": target_id, "flatten": true }),
    )?;
    attached
        .get("sessionId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Target.attachToTarget: no sessionId in {attached}"))
}

/// Locale is three surfaces, not one: `Emulation.setLocaleOverride`
/// moves the JS Intl default; `navigator.language(s)` and the
/// `Accept-Language` request header come from the UA override's
/// acceptLanguage. `set locale`-equivalent coverage therefore applies
/// both on the active page's flat session, reusing the live UA.
/// Returns Ok(false) when no page target exists.
pub fn set_locale_override(session: &str, locale: &str) -> Result<bool> {
    match set_locale_override_in(session, locale) {
        Err(e) if is_unavailable(&e) => Ok(false),
        out => out,
    }
}

fn set_locale_override_in(session: &str, locale: &str) -> Result<bool> {
    with_connection(session, |conn| {
        let page = active_page(conn)?.map(|(id, _)| id);
        let Some(target_id) = page else {
            return Ok(false);
        };
        let sid = attach_flat(conn, &target_id)?;
        conn.call_on(
            "Emulation.setLocaleOverride",
            json!({ "locale": locale }),
            &sid,
        )?;
        let version = conn.call("Browser.getVersion", json!({}))?;
        let ua = version
            .get("userAgent")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Browser.getVersion: no userAgent in {version}"))?;
        conn.call_on(
            "Network.setUserAgentOverride",
            json!({ "userAgent": ua, "acceptLanguage": locale }),
            &sid,
        )?;
        Ok(true)
    })
}

/// Prepare the page for async-clipboard access: grant `clipboard-read`
/// (plus `clipboard-write`/`clipboard-sanitized-write` when `write`) for
/// the active page's origin and enable focus emulation — without a
/// focused document `readText`/`writeText` reject `NotAllowedError`
/// even with the permission granted. Grants/overrides live on this
/// pooled connection; returns Ok(()) when no CDP endpoint exists so the
/// caller's eval surfaces the real clipboard error instead.
pub fn ensure_clipboard_access(session: &str, write: bool) -> Result<()> {
    let perms: &[&str] = if write {
        // `clipboard-sanitized-write` exists in blink but is rejected by
        // `Browser.setPermission` — `clipboard-write` alone carries it.
        &["clipboard-read", "clipboard-write"]
    } else {
        &["clipboard-read"]
    };
    let origin = active_page_origin(session)?;
    let _ = grant_permissions(session, perms, origin.as_deref())?;
    let _ = emulate_override(
        session,
        "Emulation.setFocusEmulationEnabled",
        json!({ "enabled": true }),
    )?;
    Ok(())
}

/// Attach to the active page via flat session and switch on
/// `Network.enable` there, so `webSocketCreated`/`webSocketFrame*`
/// events start arriving on this connection. Idempotent per connection;
/// returns false when no usable page target exists yet or the endpoint
/// is unavailable (fake browser / pre-open session) — capture simply
/// stays off rather than failing the run.
pub fn enable_network_capture(session: &str) -> Result<bool> {
    match enable_network_capture_in(session) {
        Err(e) if is_unavailable(&e) => Ok(false),
        out => out,
    }
}

fn enable_network_capture_in(session: &str) -> Result<bool> {
    with_connection(session, |conn| {
        if conn.capture.is_some() {
            return Ok(true);
        }
        // Any page target will do — the flat session survives navigations,
        // so arming on a leftover newtab still captures the page it becomes.
        let page = match active_page(conn)?.map(|(id, _)| id) {
            Some(id) => Some(id),
            None => conn
                .call("Target.getTargets", json!({}))?
                .get("targetInfos")
                .and_then(|t| t.as_array())
                .and_then(|infos| {
                    infos
                        .iter()
                        .rfind(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
                })
                .and_then(|t| t.get("targetId")?.as_str().map(str::to_string)),
        };
        let Some(target_id) = page else {
            return Ok(false);
        };
        let attached = conn.call(
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
        )?;
        let sid = attached
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Target.attachToTarget: no sessionId in {attached}"))?
            .to_string();
        conn.call_on("Network.enable", json!({}), &sid)?;
        conn.capture = Some(CaptureState {
            _page_sid: sid,
            entries: Vec::new(),
        });
        Ok(true)
    })
}

/// Drop the capture state — called when the daemon's own request log is
/// cleared at run start, so ws entries cover this run only.
pub fn clear_capture(session: &str) {
    let Ok(mut map) = connections().lock() else {
        return;
    };
    if let Some(conn) = map.get_mut(session) {
        if let Some(cap) = conn.capture.as_mut() {
            cap.entries.clear();
        }
        conn.events.clear();
    }
}

/// Fold buffered `Network.webSocket*` events into socket entries, then
/// return every socket the session has seen this run as a
/// `CapturedRequest`-shaped Json: `requestId` = `cdpws-<n>`,
/// `method` = "WS", `status` = 101, plus `wsFrames` payloads so claims
/// can match on frame content. First call self-enables capture so
/// opening a socket before any network step is still caught.
pub fn ws_entries(session: &str) -> Result<Vec<Json>> {
    match ws_entries_in(session) {
        Err(e) if is_unavailable(&e) => Ok(vec![]),
        out => out,
    }
}

fn ws_entries_in(session: &str) -> Result<Vec<Json>> {
    if let Err(e) = enable_network_capture_in(session) {
        if is_unavailable(&e) {
            return Ok(vec![]);
        }
        return Err(e);
    }
    with_connection(session, |conn| {
        conn.drain_events()?;
        let Some(cap) = conn.capture.as_mut() else {
            return Ok(vec![]);
        };
        for ev in conn.events.drain(..) {
            fold_event(cap, &ev);
        }
        Ok(cap
            .entries
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mut e = json!({
                    "requestId": format!("cdpws-{i}"),
                    "url": s.url,
                    "method": s.method,
                    "status": s.status.unwrap_or(101),
                    "resourceType": s.kind,
                });
                if !s.frames.is_empty() {
                    e.as_object_mut()
                        .map(|o| o.insert("wsFrames".to_string(), Json::Array(s.frames.clone())));
                }
                e
            })
            .collect())
    })
}

fn fold_event(cap: &mut CaptureState, ev: &Json) {
    let Some(method) = ev.get("method").and_then(|m| m.as_str()) else {
        return;
    };
    let params = ev.get("params").cloned().unwrap_or(Json::Null);
    let rid = || {
        params
            .get("requestId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    match method {
        "Network.webSocketCreated" => {
            let request_id = rid();
            if request_id.is_empty() || cap.entries.iter().any(|s| s.request_id == request_id) {
                return;
            }
            let url = params
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            cap.entries.push(CapEntry {
                request_id,
                url,
                method: "WS".to_string(),
                status: Some(101),
                kind: "WebSocket",
                frames: Vec::new(),
            });
        }
        "Network.webSocketFrameSent" | "Network.webSocketFrameReceived" => {
            let request_id = rid();
            let Some(sock) = cap.entries.iter_mut().find(|s| s.request_id == request_id) else {
                return;
            };
            let resp = params.get("response").cloned().unwrap_or(Json::Null);
            sock.frames.push(json!({
                "dir": if method.ends_with("Sent") { "sent" } else { "received" },
                "opcode": resp.get("opcode").cloned().unwrap_or(Json::Null),
                "payload": resp.get("payloadData").cloned().unwrap_or(Json::Null),
            }));
        }
        // SSE: EventSource is a plain GET at the HTTP layer but invisible
        // to the daemon's fetch/XHR hook — surface it as its own entry.
        "Network.requestWillBeSent" => {
            if params.get("type").and_then(|v| v.as_str()) != Some("EventSource") {
                return;
            }
            let request_id = rid();
            if request_id.is_empty() || cap.entries.iter().any(|s| s.request_id == request_id) {
                return;
            }
            let url = params
                .pointer("/request/url")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let method = params
                .pointer("/request/method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET")
                .to_string();
            cap.entries.push(CapEntry {
                request_id,
                url,
                method,
                status: None,
                kind: "EventSource",
                frames: Vec::new(),
            });
        }
        "Network.responseReceived" => {
            if params.get("type").and_then(|v| v.as_str()) != Some("EventSource") {
                return;
            }
            let request_id = rid();
            let Some(entry) = cap.entries.iter_mut().find(|s| s.request_id == request_id) else {
                return;
            };
            entry.status = params.pointer("/response/status").and_then(|v| v.as_i64());
        }
        _ => {}
    }
}

/// Detail for a `cdpws-<n>` entry — what `browser::network_request`
/// serves instead of asking the daemon (which never saw the socket).
/// `responseBody` is the joined received-frame payloads so
/// responseJsonPath-style claims can walk it.
pub fn ws_detail(session: &str, request_id: &str) -> Result<Option<Json>> {
    match ws_detail_in(session, request_id) {
        Err(e) if is_unavailable(&e) => Ok(None),
        out => out,
    }
}

fn ws_detail_in(session: &str, request_id: &str) -> Result<Option<Json>> {
    let entries = ws_entries_in(session)?;
    Ok(entries
        .into_iter()
        .find(|e| e.get("requestId").and_then(|v| v.as_str()) == Some(request_id))
        .map(|mut e| {
            let bodies: Vec<String> = e
                .get("wsFrames")
                .and_then(|f| f.as_array())
                .map(|frames| {
                    frames
                        .iter()
                        .filter(|f| f.get("dir").and_then(|d| d.as_str()) == Some("received"))
                        .filter_map(|f| {
                            f.get("payload")
                                .and_then(|p| p.as_str())
                                .map(str::to_string)
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(o) = e.as_object_mut() {
                o.insert("responseBody".to_string(), Json::String(bodies.join("\n")));
            }
            e
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_rejects_non_ws_urls() {
        assert!(CdpConnection::connect("http://x").is_err());
        assert!(CdpConnection::connect("devtools://x").is_err());
    }
}
