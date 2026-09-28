//! Persist the captured network log as a run artifact.
//!
//! `{"network"}` claims already query the browser's request capture while a
//! step runs — but that capture lived only in the browser session and
//! vanished when the run ended. Every completed run now writes
//! `<run_root>/network.json`: the full request list (id, url, method,
//! status, resourceType, mimeType) so reviewers and tools can diff traffic
//! between runs without re-replaying.
//!
//! Best-effort: a capture failure (browser already closed, CDP hiccup)
//! warns and moves on — artifacts must never fail a passing run.

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{browser, sidecar::RunPaths};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkLogEntry {
    request_id: String,
    url: String,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
}

impl From<browser::CapturedRequest> for NetworkLogEntry {
    fn from(r: browser::CapturedRequest) -> Self {
        Self {
            request_id: r.request_id,
            url: r.url,
            method: r.method,
            status: r.status,
            resource_type: r.resource_type,
            mime_type: r.mime_type,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkLog {
    request_count: usize,
    requests: Vec<NetworkLogEntry>,
}

/// Fetch the session's captured requests and write `network.json` into the
/// run dir. Errors are surfaced to the caller as `Err` for a warn log; the
/// file is simply absent when capture fails.
pub(crate) fn write_network_log(run: &RunPaths, session: &str) -> Result<()> {
    let requests: Vec<NetworkLogEntry> = browser::network_requests(session)
        .context("capture network log")?
        .into_iter()
        .map(NetworkLogEntry::from)
        .collect();
    let body = serde_json::to_string_pretty(&NetworkLog {
        request_count: requests.len(),
        requests,
    })? + "\n";
    crate::sidecar::atomic_write_file(&run.run_root.join("network.json"), body.as_bytes())
}

/// Best-effort wrapper: warn and continue on failure.
pub(crate) fn write_network_log_warn(run: &RunPaths, session: &str) {
    if let Err(e) = write_network_log(run, session) {
        eprintln!("[v2-replay] network.json skipped: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_serialises_camel_case_and_skips_absent_fields() {
        let requests = vec![
            NetworkLogEntry {
                request_id: "r1".into(),
                url: "https://example.com/api".into(),
                method: "GET".into(),
                status: Some(200),
                resource_type: Some("xhr".into()),
                mime_type: Some("application/json".into()),
            },
            NetworkLogEntry {
                request_id: "r2".into(),
                url: "https://example.com/pending".into(),
                method: "POST".into(),
                status: None,
                resource_type: None,
                mime_type: None,
            },
        ];
        let log = NetworkLog {
            request_count: requests.len(),
            requests,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&log).unwrap()).unwrap();
        assert_eq!(v["requestCount"], 2);
        assert_eq!(v["requests"][0]["requestId"], "r1");
        assert_eq!(v["requests"][0]["resourceType"], "xhr");
        // absent fields are skipped entirely rather than serialising null
        assert!(v["requests"][1].get("status").is_none());
        assert!(v["requests"][1].get("mimeType").is_none());
    }
}
