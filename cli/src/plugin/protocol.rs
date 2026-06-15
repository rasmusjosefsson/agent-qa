//! Wire protocol for agent-qa plugins.
//!
//! All values cross the boundary as JSON, one request line in, one response
//! line out. Plugins are invoked as:
//!
//! ```text
//! <plugin-binary> <kind> [<op>]
//! ```
//!
//! - `<kind>` is the extension point (`auth`, `session-policy`,
//!   `setup-hook`, `heal-strategy`, `discovery-defaults`) or the
//!   universal `ping` kind.
//! - `<op>` is an optional sub-operation for kinds that have multiple
//!   verbs (e.g. `auth probe` vs `auth login`).
//!
//! Future protocol versions are negotiated via `protocolVersion` in the
//! request. Plugins that speak a higher version than the host MUST refuse
//! with `error.code = "protocol-version"`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The protocol version this build of agent-qa speaks. Bump only with an
/// upgrade note in `docs/plugins.md`.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    /// Kind-specific payload. The shape is defined per kind in
    /// `docs/plugins.md`. For `ping` the request payload is `{}`.
    pub request: Value,
}

/// Top-level response envelope. Exactly one of `response` or `error` is set
/// (enforced by `ok`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<PluginError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginError {
    pub code: String,
    pub message: String,
}

/// The universal `ping` kind. Every plugin must implement it. Used by
/// `plugins doctor` and during discovery to learn which kinds a plugin
/// serves before invoking it for real.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: u32,
    pub name: String,
    pub kinds: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ping_response_roundtrips() {
        let pr = PingResponse {
            protocol_version: PROTOCOL_VERSION,
            name: "noop-auth".into(),
            kinds: vec!["auth".into()],
        };
        let s = serde_json::to_string(&pr).unwrap();
        let back: PingResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(back.name, "noop-auth");
        assert_eq!(back.kinds, vec!["auth".to_string()]);
    }

    #[test]
    fn response_envelope_ok_shape() {
        // Matches the noop-auth reference plugin's output.
        let s =
            r#"{"ok":true,"response":{"protocolVersion":1,"name":"noop-auth","kinds":["auth"]}}"#;
        let parsed: PluginResponse = serde_json::from_str(s).unwrap();
        assert!(parsed.ok);
        assert!(parsed.error.is_none());
        assert_eq!(parsed.response.unwrap()["name"], json!("noop-auth"));
    }

    #[test]
    fn response_envelope_error_shape() {
        let s = r#"{"ok":false,"error":{"code":"unsupported-kind","message":"x"}}"#;
        let parsed: PluginResponse = serde_json::from_str(s).unwrap();
        assert!(!parsed.ok);
        assert!(parsed.response.is_none());
        let e = parsed.error.unwrap();
        assert_eq!(e.code, "unsupported-kind");
    }
}
