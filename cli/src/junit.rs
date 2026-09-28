//! `--junit` — write the run's terminal step outcomes as JUnit XML.
//!
//! Reads `<run>/events.jsonl`, keeps each step's last terminal row
//! (`pass`/`fail`), and emits one `<testcase>` per step so CI's standard
//! test-result ingestion (Jenkins/GitLab/Azure reporters, GitHub test
//! reporters, dashboards) renders a replay like a unit-test run.
//!
//! ```xml
//! <testsuite name="<sid>" tests="3" failures="1" time="4.2">
//!   <testcase classname="<sid>" name="s3 — submit the form" time="0.8">
//!     <failure message="locator timed out">…</failure>
//!   </testcase>
//! </testsuite>
//! ```

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

struct Case {
    id: String,
    intent: String,
    ms: Option<u64>,
    error: Option<String>,
}

/// Write `<dest>` (absolute) as JUnit XML for the run at `run_dir`.
/// Returns (tests, failures).
pub fn write(run_dir: &Path, sid: &str, dest: &Path) -> Result<(usize, usize)> {
    let body = fs::read_to_string(run_dir.join("events.jsonl")).unwrap_or_default();
    // Last terminal row per step id, in first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut cases: std::collections::BTreeMap<String, Case> = std::collections::BTreeMap::new();
    for line in body.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let status = ev.get("status").and_then(|s| s.as_str()).unwrap_or("");
        if status != "pass" && status != "fail" {
            continue;
        }
        let id = ev
            .get("id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let c = Case {
            id: id.clone(),
            intent: ev
                .get("intent")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            ms: ev.get("ms").and_then(|v| v.as_u64()),
            error: if status == "fail" {
                Some(
                    ev.get("error")
                        .and_then(|s| s.as_str())
                        .unwrap_or("failed")
                        .to_string(),
                )
            } else {
                None
            },
        };
        if !cases.contains_key(&id) {
            order.push(id.clone());
        }
        cases.insert(id, c);
    }
    let tests = cases.len();
    let failures = cases.values().filter(|c| c.error.is_some()).count();
    let total_s: f64 = cases
        .values()
        .map(|c| c.ms.unwrap_or(0) as f64 / 1000.0)
        .sum();

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" time=\"{:.3}\">\n",
        esc(sid),
        tests,
        failures,
        total_s
    ));
    for id in &order {
        let c = &cases[id];
        let name = if c.intent.is_empty() {
            c.id.clone()
        } else {
            format!("{} — {}", c.id, c.intent)
        };
        xml.push_str(&format!(
            "  <testcase classname=\"{}\" name=\"{}\" time=\"{:.3}\">",
            esc(sid),
            esc(&name),
            c.ms.unwrap_or(0) as f64 / 1000.0
        ));
        match &c.error {
            Some(e) => xml.push_str(&format!(
                "<failure message=\"{}\">{}</failure></testcase>\n",
                esc(e.lines().next().unwrap_or("failed")),
                esc(e)
            )),
            None => xml.push_str("</testcase>\n"),
        }
    }
    xml.push_str("</testsuite>\n");

    if let Some(p) = dest.parent() {
        fs::create_dir_all(p).ok();
    }
    crate::sidecar::atomic_write_file(dest, xml.as_bytes())
        .with_context(|| format!("write junit {}", dest.display()))?;
    Ok((tests, failures))
}

/// Resolve the `--junit` flag value into the file to write.
/// Bare `--junit` (empty path sentinel) → `<run>/junit.xml`.
pub fn resolve_dest(run_dir: &Path, flag: &Path) -> PathBuf {
    if flag.as_os_str().is_empty() {
        run_dir.join("junit.xml")
    } else {
        flag.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn junit_writes_one_case_per_terminal_row() {
        let t = TempDir::new().unwrap();
        let run = t.path().join("r1");
        fs::create_dir_all(&run).unwrap();
        fs::write(
            run.join("events.jsonl"),
            concat!(
                "{\"idx\":1,\"total\":2,\"id\":\"s0\",\"intent\":\"open\",\"kind\":\"do:goto\",\"status\":\"running\"}\n",
                "{\"idx\":1,\"total\":2,\"id\":\"s0\",\"intent\":\"open\",\"kind\":\"do:goto\",\"status\":\"pass\",\"ms\":120}\n",
                "{\"idx\":2,\"total\":2,\"id\":\"s1\",\"intent\":\"see <x>\",\"kind\":\"check\",\"status\":\"fail\",\"ms\":30,\"error\":\"nope & <bad>\"}\n"
            ),
        )
        .unwrap();
        let dest = resolve_dest(&run, Path::new(""));
        let (tests, fails) = write(&run, "demo", &dest).unwrap();
        assert_eq!((tests, fails), (2, 1));
        let xml = fs::read_to_string(&dest).unwrap();
        assert!(xml.contains("testsuite name=\"demo\" tests=\"2\" failures=\"1\""));
        assert!(xml.contains("name=\"s1 — see &lt;x&gt;\""));
        assert!(xml.contains("<failure message=\"nope &amp; &lt;bad&gt;\">"));
        // running row produces no testcase
        assert_eq!(xml.matches("<testcase").count(), 2);
    }

    #[test]
    fn junit_explicit_path_wins() {
        assert_eq!(
            resolve_dest(Path::new("/run"), Path::new("/tmp/out.xml")),
            PathBuf::from("/tmp/out.xml")
        );
    }
}
