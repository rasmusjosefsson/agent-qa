//! `doctor` verb — diagnose the local installation.
//!
//! Probes (run in order, each prints `OK <topic>` or `FAIL <topic> — <detail>`):
//!
//!   1. agent-browser binary resolution + `--version` preflight
//!   2. plugin discovery via the host's full lookup chain
//!   3. `--deep <url>`: full end-to-end smoke — launches a real browser
//!      session, navigates, reads the DOM, then closes. Answers "can
//!      agent-qa actually drive this app from here?" in one command.
//!
//! Exit code: 0 when every probe passes, 1 otherwise. `--json` emits a
//! structured report on stdout (one object); useful for automation.

use std::io::Write;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value as Json;

use crate::browser;
use crate::paths;
use crate::plugin::{discovery, host};

pub fn run(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    let mut deep_url: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--json" => json_out = true,
            "--deep" => {
                i += 1;
                match args.get(i) {
                    Some(u) => deep_url = Some(u.clone()),
                    None => {
                        eprintln!("agent-qa doctor: --deep needs a <url>");
                        return Ok(2);
                    }
                }
            }
            s if s.starts_with("--deep=") => {
                deep_url = Some(s["--deep=".len()..].to_string());
            }
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            other => {
                eprintln!("agent-qa doctor: unknown arg {other:?}");
                return Ok(2);
            }
        }
        i += 1;
    }

    let report = collect(deep_url.as_deref());
    if json_out {
        let body = serde_json::to_string_pretty(&report)?;
        let mut out = std::io::stdout();
        out.write_all(body.as_bytes())?;
        out.write_all(b"\n")?;
        return Ok(if report.ok { 0 } else { 1 });
    }
    render_text(&report);
    Ok(if report.ok { 0 } else { 1 })
}

fn print_help() {
    println!(
        "agent-qa doctor \u{2014} diagnose the local installation\n\nUsage:\n  agent-qa doctor            Human-readable summary (default)\n  agent-qa doctor --json     Structured report on stdout\n  agent-qa doctor --deep <url>\n                             End-to-end smoke: launch a browser session,\n                             navigate to <url>, read the DOM, close."
    );
}

#[derive(Debug, Serialize)]
struct Report {
    ok: bool,
    probes: Vec<Probe>,
}

#[derive(Debug, Serialize)]
struct Probe {
    topic: String,
    ok: bool,
    detail: Json,
}

fn collect(deep_url: Option<&str>) -> Report {
    let mut probes: Vec<Probe> = Vec::new();
    let mut overall = true;

    // 1. agent-browser
    let ab = match browser::resolve_bin() {
        Ok(p) => {
            let preflight = browser::ensure_installed();
            match preflight {
                Ok(()) => Probe {
                    topic: "agent-browser".into(),
                    ok: true,
                    detail: serde_json::json!({ "binary": p.display().to_string() }),
                },
                Err(e) => Probe {
                    topic: "agent-browser".into(),
                    ok: false,
                    detail: serde_json::json!({ "binary": p.display().to_string(), "error": e.to_string() }),
                },
            }
        }
        Err(e) => Probe {
            topic: "agent-browser".into(),
            ok: false,
            detail: serde_json::json!({ "error": e.to_string() }),
        },
    };
    if !ab.ok {
        overall = false;
    }
    probes.push(ab);

    // 2. plugins
    match discovery::discover(&discovery::DiscoveryOpts::default()) {
        Ok(found) => {
            let items: Vec<Json> = found
                .iter()
                .map(|p| {
                    let kinds = match host::ping(&p.binary) {
                        Ok(pong) => Json::from(pong.kinds),
                        Err(_) => Json::from(vec!["(ping failed)".to_string()]),
                    };
                    serde_json::json!({
                        "binary": p.binary.display().to_string(),
                        "source": format!("{:?}", p.source),
                        "kinds": kinds,
                    })
                })
                .collect();
            probes.push(Probe {
                topic: "plugins".into(),
                ok: true,
                detail: Json::Array(items),
            });
        }
        Err(e) => {
            overall = false;
            probes.push(Probe {
                topic: "plugins".into(),
                ok: false,
                detail: serde_json::json!({ "error": e.to_string() }),
            });
        }
    }

    // 3. paths
    probes.push(Probe {
        topic: "paths".into(),
        ok: true,
        detail: serde_json::json!({
            "scenariosRoot": paths::scenarios_root().display().to_string(),
            "recordRoot": paths::record_root().display().to_string(),
        }),
    });

    // 4. --deep <url>: real browser smoke
    if let Some(url) = deep_url {
        let probe = deep_probe(url);
        if !probe.ok {
            overall = false;
        }
        probes.push(probe);
    }

    Report {
        ok: overall,
        probes,
    }
}

fn deep_probe(url: &str) -> Probe {
    let session = format!("doctor-deep-{}", std::process::id());
    let outcome = (|| -> Result<Json, crate::browser::AgentBrowserError> {
        browser::open(&session, url)?;
        // bounded settle — a chatty SPA never reaches networkidle
        let _ = browser::wait_for_load_capped(&session, "networkidle", 5000);
        let snap = browser::snapshot_full(&session).unwrap_or_default();
        let elements = snap.lines().filter(|l| !l.trim().is_empty()).count();
        let console = browser::console_messages(&session).unwrap_or_default();
        let errors = console.iter().filter(|m| m.level == "error").count();
        Ok(serde_json::json!({
            "url": url,
            "finalUrl": browser::current_url(&session),
            "elements": elements,
            "consoleErrors": errors,
        }))
    })();
    browser::close_session(&session);
    match outcome {
        Ok(detail) => Probe {
            topic: "deep".into(),
            ok: true,
            detail,
        },
        Err(e) => Probe {
            topic: "deep".into(),
            ok: false,
            detail: serde_json::json!({ "url": url, "error": e.to_string() }),
        },
    }
}

fn render_text(r: &Report) {
    for p in &r.probes {
        let label = if p.ok { "OK  " } else { "FAIL" };
        println!("{label} {topic}", topic = p.topic);
        if !p.ok {
            if let Some(err) = p.detail.get("error").and_then(|v| v.as_str()) {
                println!("     {err}");
            }
        } else {
            // Compact one-liner of the detail.
            match &p.detail {
                Json::Object(map) => {
                    for (k, v) in map {
                        println!("     {k}: {}", short_value(v));
                    }
                }
                Json::Array(items) => {
                    if items.is_empty() {
                        println!("     (none discovered)");
                    } else {
                        for it in items {
                            println!("     - {}", short_value(it));
                        }
                    }
                }
                other => println!("     {}", short_value(other)),
            }
        }
    }
    println!();
    println!("overall: {}", if r.ok { "OK" } else { "FAIL" });
}

fn short_value(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_camel_keys() {
        let r = Report {
            ok: true,
            probes: vec![Probe {
                topic: "agent-browser".into(),
                ok: true,
                detail: serde_json::json!({ "binary": "/bin/x" }),
            }],
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"topic\":\"agent-browser\""));
    }
}
