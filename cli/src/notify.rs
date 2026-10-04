//! `notify` — the alert sink for scheduled/CI replays. `[notify]` in
//! `agent-qa.toml` points at a webhook; after each run the verdict is
//! POSTed as JSON — replay-as-monitor without a hosted service.
//!
//!   [notify]
//!   url = "https://hooks.slack.com/services/…"
//!   on  = "failure"        # "failure" (default) or "always"
//!
//! Payload carries a Slack-compatible `text` line plus the structured
//! fields (`sid`, `runId`, `verdict`, `summary`, `failed`, `runDir`),
//! so a plain webhook or a Slack/Discord incoming hook both work.
//! Delivery problems never fail the run — they warn on stderr only.

use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::json;

use crate::paths;
use crate::sidecar::RunAudit;

/// What the caller wants delivered: "failure" posts only when the run
/// did not pass; "always" posts every verdict.
fn should_send(on: &str, ok: bool) -> bool {
    match on {
        "always" => true,
        _ => !ok,
    }
}

/// Post the finished run's verdict to `[notify] url` when configured.
/// `ok` is the raw run outcome (verdict text); `gated` is the gate
/// decision (a quarantined failure passes the gate — under the default
/// on="failure" it stays silent; on="always" it posts as QUAR-FAIL).
/// Best-effort: config lookup, the POST, and the response code are all
/// allowed to fail without affecting the run's exit code.
pub(crate) fn post_verdict(audit: &RunAudit, ok: bool, gated: bool, run_dir: &str) {
    let Some((_cfg, table)) = paths::notify_config() else {
        return;
    };
    let Some(url) = table.url.as_deref().filter(|u| !u.trim().is_empty()) else {
        return;
    };
    let on = table.on.as_deref().unwrap_or("failure");
    if on != "failure" && on != "always" {
        eprintln!(
            "[notify] [notify].on {on:?} unknown — expected failure|always; posting on failure"
        );
    }
    if !should_send(on, gated) {
        return;
    }
    let verdict = if ok {
        "PASS"
    } else if audit.quarantined == Some(true) {
        "QUAR-FAIL"
    } else {
        "FAIL"
    };
    let summary = audit.summary.clone().unwrap_or_default();
    let body = json!({
        "text": format!("agent-qa {} — {} ({})", verdict, audit.scenario_id, summary),
        "sid": audit.scenario_id,
        "runId": audit.run_id,
        "verdict": verdict,
        "summary": summary,
        "runDir": run_dir,
        "tag": audit.tag,
    });
    match ureq::post(url)
        .timeout(Duration::from_secs(10))
        .send_json(&body)
    {
        Ok(_) => eprintln!("[notify] verdict posted ({verdict})"),
        Err(e) => eprintln!("[notify] POST failed (run unaffected): {e:#}"),
    }
}

/// `agent-qa notify test` — fire a sample payload at the configured
/// webhook so the wiring can be checked without running a scenario.
pub fn run(args: &[String]) -> Result<u8> {
    match args.first().map(|s| s.as_str()) {
        None | Some("-h") | Some("--help") | Some("help") => {
            println!(
                "agent-qa notify — alert sink for finished runs\n\nUsage:\n  agent-qa notify test\n\nConfigure in agent-qa.toml:\n  [notify]\n  url = \"https://hooks.example/…\"\n  on  = \"failure\"   # failure (default) | always\n\nEvery replay posts its verdict JSON to url; `test` sends a sample."
            );
            Ok(0)
        }
        Some("test") => {
            let (cfg, table) = paths::notify_config()
                .context("notify: no [notify] table in agent-qa.toml — set url to a webhook")?;
            let url = table
                .url
                .as_deref()
                .filter(|u| !u.trim().is_empty())
                .with_context(|| format!("notify: [notify] in {} needs url", cfg.display()))?;
            let body = json!({
                "text": "agent-qa notify test — sink wired",
                "sid": "(test)",
                "verdict": "TEST",
            });
            ureq::post(url)
                .timeout(Duration::from_secs(10))
                .send_json(&body)
                .with_context(|| format!("notify: POST {url}"))?;
            println!("notify: test payload delivered to {url}");
            Ok(0)
        }
        Some(other) => {
            anyhow::bail!("unknown notify subcommand {other:?} — try `agent-qa notify test`")
        }
    }
}
