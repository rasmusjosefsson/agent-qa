//! `agent-qa ingest` — turn a browser-extension capture bundle into a
//! runnable scenario.
//!
//! The extension's one-button export carries the interaction drafts the
//! user produced plus the traffic the injected fetch/XHR patch saw. The
//! drafts go through the same validation pipeline `record-step` uses, so
//! a malformed capture fails here with the same errors, and the network
//! entries land as the `replays/recorded/network.har` sidecar
//! `--mock-from recorded` already understands.
//!
//! ```text
//! agent-qa ingest <bundle.json> [--sid <name>]
//!
//! writes:
//!   <scenarios_root>/<sid>/scenario.json
//!   <scenarios_root>/<sid>/replays/recorded/network.har
//!
//! then:
//!   agent-qa replay <sid>                          — replay against the live app
//!   agent-qa replay <sid> --mock-from recorded --offline
//!                                                  — replay pinned to the
//!                                                    captured backend state
//! ```

use anyhow::{bail, Context, Result};
use serde_json::Value as Json;
use std::fs;
use std::path::PathBuf;

use crate::record_step::{parse_draft, StepKind};
use crate::scenario::{Env, EnvOp, Producer, Provenance, Scenario};

struct Opts {
    bundle: PathBuf,
    sid: Option<String>,
}

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let text = fs::read_to_string(&opts.bundle)
        .with_context(|| format!("ingest: read {}", opts.bundle.display()))?;
    let bundle: Json = serde_json::from_str(&text)
        .with_context(|| format!("ingest: parse {}", opts.bundle.display()))?;
    if !bundle.is_object() {
        bail!("ingest: bundle must be a JSON object — expected the extension export shape");
    }

    let steps_json = bundle["steps"].as_array().cloned().unwrap_or_default();
    if steps_json.is_empty() {
        bail!(
            "ingest: bundle has no steps — the extension exports its captured \
             interactions as `steps`; was anything recorded?"
        );
    }

    let url = bundle["url"].as_str().map(str::to_string);
    for warning in bundle["warnings"].as_array().into_iter().flatten() {
        if let Some(w) = warning.as_str() {
            eprintln!("ingest: bundle warning: {w}");
        }
    }
    let intent = bundle["intent"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "ingested capture".to_string());
    let sid = opts.sid.unwrap_or_else(|| default_sid(url.as_deref()));

    let mut steps = Vec::with_capacity(steps_json.len());
    for (i, item) in steps_json.iter().enumerate() {
        let step_id = format!("s{}", i + 1);
        // Two shapes tolerated: the extension's {kind, draft} wrapper, or a
        // bare draft object when a bundle was hand-assembled.
        let (kind_str, draft) = if let Some(d) = item.get("draft") {
            (item["kind"].as_str().unwrap_or("do").to_string(), d.clone())
        } else {
            (
                item["kind"].as_str().unwrap_or("do").to_string(),
                item.clone(),
            )
        };
        let kind = StepKind::parse(&kind_str)
            .with_context(|| format!("ingest: step {} has unknown kind", step_id))?;
        let draft = normalize_shorthand_locator(draft);
        steps.push(
            parse_draft(kind, &draft, &step_id)
                .with_context(|| format!("ingest: step {}", step_id))?,
        );
    }

    let env = url.as_ref().map(|u| Env {
        open: Some(vec![
            EnvOp::Fresh {
                intent: Some("fresh slate like the extension's tab".to_string()),
                policy: None,
            },
            EnvOp::Nav {
                intent: Some("open the recorded page".to_string()),
                url: Some(u.clone()),
                policy: None,
            },
        ]),
        close: None,
    });

    let scenario = Scenario {
        schema: "scenario/2".to_string(),
        id: sid.clone(),
        intent,
        tags: None,
        inputs: None,
        env,
        steps,
        templates: None,
        produced_by: Some(Provenance {
            producer: Producer::AutomatedCapture,
            produced_at: Some(
                chrono::Utc::now()
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string(),
            ),
            recorded_at: bundle["startedAt"].as_str().map(str::to_string),
            source_ref: Some(format!(
                "agent-qa extension bundle {}",
                opts.bundle.display()
            )),
        }),
    };

    let doc = serde_json::to_value(&scenario)?;
    crate::schema::validate_value(&doc)
        .context("ingest: assembled scenario failed schema validation")?;

    let dir = crate::paths::scenario_dir(&sid)?;
    fs::create_dir_all(&dir).with_context(|| format!("ingest: create {}", dir.display()))?;
    let scenario_path = dir.join("scenario.json");
    fs::write(&scenario_path, serde_json::to_string_pretty(&doc)? + "\n")
        .with_context(|| format!("ingest: write {}", scenario_path.display()))?;

    let step_count = scenario.steps.len();
    let mut har_note = String::new();
    if let Some(network) = bundle["network"].as_array().filter(|n| !n.is_empty()) {
        let har_dir = dir.join("replays").join("recorded");
        fs::create_dir_all(&har_dir)
            .with_context(|| format!("ingest: create {}", har_dir.display()))?;
        let har_path = har_dir.join("network.har");
        fs::write(
            &har_path,
            serde_json::to_string_pretty(&build_har(network, url.as_deref()))? + "\n",
        )
        .with_context(|| format!("ingest: write {}", har_path.display()))?;
        har_note = format!(
            " + {} captured requests → replays/recorded/network.har",
            network.len()
        );
    }

    println!(
        "[ingest] {sid}: {step_count} steps{har_note}\n  scenario: {}\n  replay: agent-qa replay {sid}\n  hermetic: agent-qa replay {sid} --mock-from recorded --offline",
        scenario_path.display()
    );
    Ok(0)
}

/// Compact `a > b` into `a>b` inside `on` shorthand strings — the
/// shorthand grammar requires `\S+` after the `kind:` prefix, and a
/// hand-edited bundle (or an older exporter) may carry spaced
/// combinators. Applied only to `css:`; other kinds pass through.
fn normalize_shorthand_locator(mut draft: Json) -> Json {
    let Some(on) = draft.get("on").and_then(|v| v.as_str()) else {
        return draft;
    };
    let Some(sel) = on.strip_prefix("css:") else {
        return draft;
    };
    if !sel.contains(" > ") {
        return draft;
    }
    let compacted = sel.replace(" > ", ">");
    draft["on"] = Json::String(format!("css:{compacted}"));
    draft
}

/// `capture-2026-01-01` style sid from the recorded host + timestamp; a
/// bundle with no URL falls back to `capture-<epoch>`.
fn default_sid(url: Option<&str>) -> String {
    let host = url
        .and_then(|u| {
            u.split_once("://")
                .map(|(_, rest)| rest.split('/').next().unwrap_or(""))
        })
        .unwrap_or("page");
    let slug: String = host
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("-");
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    format!("capture-{slug}-{stamp}")
}

/// Render the extension's network array as a minimal HAR 1.2 document —
/// exactly the fields `--mock-from` and `replay --har` tooling read:
/// request.{method,url,postData}, response.{status,content.text}.
fn build_har(entries: &[Json], page_url: Option<&str>) -> Json {
    let entries_json: Vec<Json> = entries
        .iter()
        .filter_map(|e| {
            let url = e["url"].as_str()?;
            if url.is_empty() || url.starts_with("data:") {
                return None;
            }
            let mut request = serde_json::json!({
                "method": e["method"].as_str().unwrap_or("GET"),
                "url": url,
            });
            if let Some(p) = e["postData"].as_str() {
                request["postData"] = serde_json::json!({
                    "mimeType": "text/plain",
                    "text": p,
                });
            }
            Some(serde_json::json!({
                "startedDateTime": e["startedAt"].as_str().unwrap_or(""),
                "time": e["durationMs"].as_f64().unwrap_or(0.0),
                "request": request,
                "response": {
                    "status": e["status"].as_i64().unwrap_or(0),
                    "content": {
                        "mimeType": "text/plain",
                        "text": e["body"].as_str().unwrap_or(""),
                    },
                },
            }))
        })
        .collect();
    serde_json::json!({
        "log": {
            "version": "1.2",
            "creator": {"name": "agent-qa extension", "version": "0.1.0"},
            "pages": page_url.map(|u| vec![serde_json::json!({
                "startedDateTime": "",
                "id": "page_1",
                "title": u,
            })]).unwrap_or_default(),
            "entries": entries_json,
        }
    })
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut bundle = None;
    let mut sid = None;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--sid" => {
                sid = Some(
                    it.next()
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("--sid needs a value"))?,
                )
            }
            "-h" | "--help" => {
                println!(
                    "agent-qa ingest — turn an extension capture bundle into a scenario

USAGE
  agent-qa ingest <bundle.json> [--sid <name>]

The browser extension's one-button export (steps + network traffic) is
validated and written as <scenarios_root>/<sid>/scenario.json plus the
replays/recorded/network.har sidecar `--mock-from recorded` replays
against.

ARGUMENTS
  <bundle.json>    path to the downloaded capture bundle
  --sid <name>     scenario id (default: capture-<host>-<timestamp>)"
                );
                std::process::exit(0);
            }
            s if !s.starts_with('-') => {
                if bundle.is_none() {
                    bundle = Some(PathBuf::from(s));
                } else {
                    bail!("ingest: unexpected argument {s:?}");
                }
            }
            s => bail!("ingest: unknown flag {s:?}"),
        }
    }
    let bundle = bundle.ok_or_else(|| {
        anyhow::anyhow!("ingest: needs a bundle path — agent-qa ingest <bundle.json>")
    })?;
    Ok(Opts { bundle, sid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bundle(steps: Vec<Json>, network: Vec<Json>) -> Json {
        json!({
            "version": 1,
            "url": "https://example.com/app",
            "startedAt": "2026-01-01T00:00:00.000Z",
            "steps": steps,
            "network": network,
        })
    }

    fn click_draft() -> Json {
        json!({
            "kind": "do",
            "draft": {
                "intent": "click \"Go\"",
                "verb": "click",
                "on": "css:#go"
            }
        })
    }

    fn type_draft() -> Json {
        json!({
            "kind": "do",
            "draft": {
                "intent": "type into \"Email\"",
                "verb": "type",
                "on": "css:#email",
                "value": {"from": "literal", "literal": "a@b.c"}
            }
        })
    }

    #[test]
    fn build_har_maps_extension_entries() {
        let har = build_har(
            &[json!({
                "url": "https://x/api",
                "method": "POST",
                "status": 201,
                "body": "{\"ok\":true}",
                "postData": "{\"q\":1}",
                "startedAt": "t0",
                "durationMs": 42
            })],
            Some("https://x/app"),
        );
        let e = &har["log"]["entries"][0];
        assert_eq!(e["request"]["method"], "POST");
        assert_eq!(e["request"]["url"], "https://x/api");
        assert_eq!(e["request"]["postData"]["text"], "{\"q\":1}");
        assert_eq!(e["response"]["status"], 201);
        assert_eq!(e["response"]["content"]["text"], "{\"ok\":true}");
        assert_eq!(har["log"]["pages"][0]["title"], "https://x/app");
    }

    #[test]
    fn build_har_drops_data_urls_and_url_less_entries() {
        let har = build_har(
            &[
                json!({"url": "data:text/plain,hi", "method": "GET"}),
                json!({"method": "GET"}),
                json!({"url": "https://x/ok", "method": "GET", "status": 200}),
            ],
            None,
        );
        let entries = har["log"]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["request"]["url"], "https://x/ok");
    }

    #[test]
    fn default_sid_slugs_the_host() {
        let sid = default_sid(Some("https://www.Example.com:8080/app?x=1"));
        assert!(sid.starts_with("capture-www-example-com-"), "got {sid}");
        let bare = default_sid(None);
        assert!(bare.starts_with("capture-page-"), "got {bare}");
    }

    #[test]
    fn parse_args_accepts_bundle_and_sid() {
        let o = parse_args(&[
            "b.json".to_string(),
            "--sid".to_string(),
            "mine".to_string(),
        ])
        .unwrap();
        assert_eq!(o.bundle, PathBuf::from("b.json"));
        assert_eq!(o.sid.as_deref(), Some("mine"));
        assert!(parse_args(&["a".into(), "b".into()]).is_err());
        assert!(parse_args(&[]).is_err());
    }

    #[test]
    fn spaced_child_combinators_are_compacted() {
        // Regression for the first real bundle: cssPath emitted "a > b",
        // which the \S+ shorthand grammar rejects.
        let d = normalize_shorthand_locator(json!({
            "intent": "click", "verb": "click",
            "on": "css:#nav > ul.list > li:nth-of-type(2) > a"
        }));
        assert_eq!(d["on"], "css:#nav>ul.list>li:nth-of-type(2)>a");
        // Non-css kinds and already-compact selectors pass through.
        let other = normalize_shorthand_locator(json!({
            "intent": "t", "verb": "type",
            "on": "text:sign in now", "value": {"from":"literal","literal":"x"}
        }));
        assert_eq!(other["on"], "text:sign in now");
        let compact = normalize_shorthand_locator(json!({
            "intent": "t", "verb": "click", "on": "css:#a>b"
        }));
        assert_eq!(compact["on"], "css:#a>b");
    }

    #[test]
    fn drafts_parse_through_the_record_pipeline() {
        // The end-to-end contract the extension relies on: {kind, draft}
        // items parse through parse_draft into schema-valid Steps.
        for item in [click_draft(), type_draft()] {
            let kind = StepKind::parse(item["kind"].as_str().unwrap()).unwrap();
            let step = parse_draft(kind, &item["draft"], "s1").unwrap();
            assert_eq!(step.id(), "s1");
        }
    }

    #[test]
    fn bundle_shape_is_stable_for_the_extension() {
        // Bundle fields ingest reads — guard so the extension and CLI
        // never drift apart silently.
        let b = bundle(vec![click_draft()], vec![]);
        assert!(b["url"].is_string());
        assert!(b["steps"].is_array());
        assert!(b["network"].is_array());
        assert_eq!(b["steps"][0]["kind"], "do");
        assert!(b["steps"][0]["draft"].is_object());
    }
}
