//! Network-log diff — request-level comparison between two replay runs.
//!
//! Reads `<run>/network.json` from each run (written since the netlog
//! artifact landed; runs recorded before it simply have no file, and the
//! compare section reports that instead of failing). Requests are keyed by
//! `METHOD <url>`; a URL fetched N times compares as a multiset of statuses.
//!
//! Outcomes per key:
//!   SAME            — identical status multiset on both sides
//!   CHANGED         — present in both, status(es) or count differ
//!   ONLY-A / ONLY-B — request disappeared / appeared between runs

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NetOutcome {
    Same,
    Changed,
    OnlyA,
    OnlyB,
}

impl NetOutcome {
    pub(super) fn label(&self) -> &'static str {
        match self {
            NetOutcome::Same => "SAME",
            NetOutcome::Changed => "CHANGED",
            NetOutcome::OnlyA => "ONLY-A",
            NetOutcome::OnlyB => "ONLY-B",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct NetEntry {
    /// Display key: `"GET https://…/api/users"`.
    pub key: String,
    pub outcome: NetOutcome,
    /// `"200,200"` style status summary per side; `-` when absent.
    pub statuses_a: String,
    pub statuses_b: String,
}

#[derive(Debug, Clone)]
pub(super) struct NetReport {
    /// Whether `network.json` existed in run A / run B.
    pub present_a: bool,
    pub present_b: bool,
    pub entries: Vec<NetEntry>,
}

/// Status multiset per `(method, url)` key for one run's network.json.
type RequestMap = BTreeMap<String, Vec<i64>>;

fn read_requests(run_dir: &Path) -> Result<Option<RequestMap>> {
    let path = run_dir.join("network.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("parse {} — not a network.json", path.display()))?;
    let mut map: RequestMap = BTreeMap::new();
    for req in v["requests"].as_array().cloned().unwrap_or_default() {
        let method = req["method"].as_str().unwrap_or("?");
        let url = req["url"].as_str().unwrap_or("");
        let key = format!("{method} {url}");
        let status = req["status"].as_i64().unwrap_or(-1);
        map.entry(key).or_default().push(status);
    }
    Ok(Some(map))
}

fn fmt_statuses(v: &[i64]) -> String {
    if v.is_empty() {
        return "-".into();
    }
    v.iter()
        .map(|s| {
            if *s < 0 {
                "pending".to_string()
            } else {
                s.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn build(scenario_dir: &Path, run_a: &str, run_b: &str) -> Result<NetReport> {
    let dir_a = scenario_dir.join("replays").join(run_a);
    let dir_b = scenario_dir.join("replays").join(run_b);
    let map_a = read_requests(&dir_a)?;
    let map_b = read_requests(&dir_b)?;
    if map_a.is_none() && map_b.is_none() {
        return Ok(NetReport {
            present_a: false,
            present_b: false,
            entries: Vec::new(),
        });
    }
    let a = map_a.clone().unwrap_or_default();
    let b = map_b.clone().unwrap_or_default();

    let mut keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    keys.extend(a.keys().cloned());
    keys.extend(b.keys().cloned());

    let mut entries = Vec::new();
    for key in keys {
        match (a.get(&key), b.get(&key)) {
            (Some(sa), Some(sb)) => {
                let outcome = if sa == sb {
                    NetOutcome::Same
                } else {
                    NetOutcome::Changed
                };
                entries.push(NetEntry {
                    key,
                    outcome,
                    statuses_a: fmt_statuses(sa),
                    statuses_b: fmt_statuses(sb),
                });
            }
            (Some(sa), None) => entries.push(NetEntry {
                key,
                outcome: NetOutcome::OnlyA,
                statuses_a: fmt_statuses(sa),
                statuses_b: "-".into(),
            }),
            (None, Some(sb)) => entries.push(NetEntry {
                key,
                outcome: NetOutcome::OnlyB,
                statuses_a: "-".into(),
                statuses_b: fmt_statuses(sb),
            }),
            (None, None) => {}
        }
    }

    Ok(NetReport {
        present_a: map_a.is_some(),
        present_b: map_b.is_some(),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_netlog(jdir: &Path, run: &str, body: &str) {
        let dir = jdir.join("replays").join(run);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("network.json"), body).unwrap();
    }

    #[test]
    fn absent_logs_report_not_present() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        fs::create_dir_all(jdir.join("replays").join("rA")).unwrap();
        let r = build(&jdir, "rA", "rB").unwrap();
        assert!(!r.present_a && !r.present_b);
        assert!(r.entries.is_empty());
    }

    #[test]
    fn diff_classifies_same_changed_and_only_sides() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        write_netlog(
            &jdir,
            "rA",
            r#"{"requestCount":3,"requests":[
              {"requestId":"1","url":"https://x/api","method":"GET","status":200},
              {"requestId":"2","url":"https://x/slow","method":"GET","status":200},
              {"requestId":"3","url":"https://x/gone","method":"GET","status":404}
            ]}"#,
        );
        write_netlog(
            &jdir,
            "rB",
            r#"{"requestCount":3,"requests":[
              {"requestId":"9","url":"https://x/api","method":"GET","status":200},
              {"requestId":"8","url":"https://x/slow","method":"GET","status":500},
              {"requestId":"7","url":"https://x/new","method":"POST","status":201}
            ]}"#,
        );
        let r = build(&jdir, "rA", "rB").unwrap();
        assert!(r.present_a && r.present_b);
        let by_key: std::collections::HashMap<&str, &NetOutcome> = r
            .entries
            .iter()
            .map(|e| (e.key.as_str(), &e.outcome))
            .collect();
        assert_eq!(by_key["GET https://x/api"], &NetOutcome::Same);
        assert_eq!(by_key["GET https://x/slow"], &NetOutcome::Changed);
        assert_eq!(by_key["GET https://x/gone"], &NetOutcome::OnlyA);
        assert_eq!(by_key["POST https://x/new"], &NetOutcome::OnlyB);
        let changed = r
            .entries
            .iter()
            .find(|e| e.key == "GET https://x/slow")
            .unwrap();
        assert_eq!(changed.statuses_a, "200");
        assert_eq!(changed.statuses_b, "500");
    }
}
