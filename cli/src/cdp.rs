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
//! at a time, events ignored. No TLS — the endpoint is loopback-only.
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

pub struct CdpConnection {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: AtomicU64,
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
        })
    }

    /// Send a command and wait for its response. Events and replies for
    /// other ids are skipped — callers issue commands serially.
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
    with_connection(session, |conn| {
        let page = active_page(conn)?.map(|(id, _)| id);
        let Some(target_id) = page else {
            return Ok(false);
        };
        let attached = conn.call(
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
        )?;
        let session_id = attached
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Target.attachToTarget: no sessionId in {attached}"))?;
        conn.call_on(
            "Emulation.setGeolocationOverride",
            json!({ "latitude": lat, "longitude": lng, "accuracy": accuracy }),
            session_id,
        )?;
        Ok(true)
    })
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
