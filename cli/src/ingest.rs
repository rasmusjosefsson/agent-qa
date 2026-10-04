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
//! agent-qa ingest --listen [--port <n>]
//!
//! --listen runs a localhost endpoint the extension's export button
//! POSTs bundles to directly — the recording lands as a scenario with
//! no file download step at all. The extension falls back to the
//! download flow when nothing listens.
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
use crate::sidecar::atomic_write_file;

/// What an ingested bundle produced — surfaced as the CLI's summary
/// line and as the JSON the `--listen` endpoint answers with.
struct Outcome {
    sid: String,
    scenario_path: PathBuf,
    steps: usize,
    requests: usize,
    files: usize,
    warnings: Vec<String>,
}

/// Validate a capture bundle end-to-end and write the scenario +
/// sidecars. `source_ref` lands in `produced_by.source_ref` — the CLI
/// passes the bundle path, the HTTP endpoint the request origin.
fn ingest_bundle(bundle: &Json, sid: Option<String>, source_ref: &str) -> Result<Outcome> {
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
    let mut warnings: Vec<String> = bundle["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| w.as_str().map(str::to_string))
        .collect();
    let intent = bundle["intent"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "ingested capture".to_string());
    let sid = sid.unwrap_or_else(|| default_sid(url.as_deref()));

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
        only_when: None,
        produced_by: Some(Provenance {
            producer: Producer::AutomatedCapture,
            produced_at: Some(
                chrono::Utc::now()
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string(),
            ),
            recorded_at: bundle["startedAt"].as_str().map(str::to_string),
            source_ref: Some(format!("agent-qa extension bundle {source_ref}")),
        }),
    };

    let doc = serde_json::to_value(&scenario)?;
    crate::schema::validate_value(&doc)
        .context("ingest: assembled scenario failed schema validation")?;

    let dir = crate::paths::scenario_dir(&sid)?;
    let scenario_path = dir.join("scenario.json");
    if scenario_path.exists() {
        bail!(
            "ingest: destination already exists at {} (refusing to overwrite) — pick a different --sid, or `scenario delete {sid}` first",
            scenario_path.display()
        );
    }
    fs::create_dir_all(&dir).with_context(|| format!("ingest: create {}", dir.display()))?;
    atomic_write_file(
        &scenario_path,
        (serde_json::to_string_pretty(&doc)? + "\n").as_bytes(),
    )?;

    let step_count = scenario.steps.len();
    let mut requests = 0usize;
    if let Some(network) = bundle["network"].as_array().filter(|n| !n.is_empty()) {
        let har_dir = dir.join("replays").join("recorded");
        fs::create_dir_all(&har_dir)
            .with_context(|| format!("ingest: create {}", har_dir.display()))?;
        let har_path = har_dir.join("network.har");
        atomic_write_file(
            &har_path,
            (serde_json::to_string_pretty(&build_har(network, url.as_deref()))? + "\n").as_bytes(),
        )?;
        requests = network.len();
    }

    // Upload steps reference files/<name>; the extension inlines small
    // contents on the step envelope (item.uploads[].data as a data URL).
    // Materialize them so the upload replays end-to-end; names that
    // arrived without data keep the manual-drop story with a warning.
    let mut written_files = 0usize;
    let mut missing_uploads: Vec<String> = Vec::new();
    let files_dir = dir.join("files");
    for item in steps_json {
        let Some(uploads) = item["uploads"].as_array() else {
            continue;
        };
        for u in uploads {
            let Some(name) = u["name"].as_str() else {
                continue;
            };
            let base = PathBuf::from(name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .filter(|n| !n.is_empty() && n != "." && n != "..");
            let Some(base) = base else {
                continue;
            };
            match u["data"].as_str().and_then(decode_data_url) {
                Some(bytes) => {
                    if written_files == 0 {
                        fs::create_dir_all(&files_dir)
                            .with_context(|| format!("ingest: create {}", files_dir.display()))?;
                    }
                    let path = files_dir.join(&base);
                    if !path.exists() {
                        atomic_write_file(&path, &bytes)?;
                        written_files += 1;
                    }
                }
                None => {
                    if !missing_uploads.contains(&base) {
                        missing_uploads.push(base);
                    }
                }
            }
        }
    }
    if !missing_uploads.is_empty() {
        warnings.push(format!(
            "upload(s) arrived without contents — drop {} under {} before replaying",
            missing_uploads.join(", "),
            files_dir.display()
        ));
    }

    Ok(Outcome {
        sid,
        scenario_path,
        steps: step_count,
        requests,
        files: written_files,
        warnings,
    })
}

/// The extension's one-button POST target: `agent-qa ingest --listen`
/// runs a tiny localhost endpoint the export hands its bundle to —
/// no file to find, attach, or send. `GET /health` answers liveness
/// so the popup can fall back to a download when nothing listens.
fn listen(port: u16) -> Result<u8> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("ingest --listen: bind 127.0.0.1:{port}"))?;
    println!("agent-qa ingest listening on http://127.0.0.1:{port}");
    println!("  POST /ingest — the extension's export button hands bundles here");
    println!("  GET  /health — liveness probe");
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                std::thread::spawn(move || {
                    let _ = handle_conn(stream);
                });
            }
            Err(e) => eprintln!("ingest --listen: accept: {e}"),
        }
    }
    Ok(0)
}

/// Minimal HTTP/1.1: a request line, Content-Length body, one JSON
/// answer per connection. CORS headers ride every response — the
/// extension's service worker fetches cross-origin by design.
fn handle_conn(mut stream: std::net::TcpStream) -> Result<()> {
    use std::io::{Read, Write};
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .ok();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(30)))
        .ok();

    let mut head = Vec::with_capacity(4096);
    let mut buf = [0u8; 4096];
    let body_off = loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            bail!("connection closed before headers");
        }
        head.extend_from_slice(&buf[..n]);
        if head.len() > 64 * 1024 {
            bail!("request headers exceed 64KB");
        }
        if let Some(off) = find_subslice(&head, b"\r\n\r\n") {
            break off + 4;
        }
    };
    let head_text = String::from_utf8_lossy(&head[..body_off]);
    let mut lines = head_text.lines();
    let request = lines.next().unwrap_or_default();
    let mut parts = request.split_whitespace();
    let (method, path) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    let mut content_length = 0usize;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }
    }

    let cors = "Access-Control-Allow-Origin: *\r\n\
                Access-Control-Allow-Headers: content-type\r\n\
                Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n";
    let answer = |status: &str, body: &str| -> Vec<u8> {
        format!(
            "HTTP/1.1 {status}\r\n{cors}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    };

    if method == "OPTIONS" {
        stream.write_all(&answer("204 No Content", ""))?;
        return Ok(());
    }
    if method == "GET" && path.starts_with("/health") {
        stream.write_all(&answer(
            "200 OK",
            r#"{"ok":true,"service":"agent-qa-ingest"}"#,
        ))?;
        return Ok(());
    }
    if method != "POST" || !path.starts_with("/ingest") {
        stream.write_all(&answer(
            "404 Not Found",
            r#"{"error":"POST /ingest or GET /health"}"#,
        ))?;
        return Ok(());
    }
    if content_length == 0 {
        stream.write_all(&answer(
            "400 Bad Request",
            r#"{"error":"empty body — POST the extension bundle JSON"}"#,
        ))?;
        return Ok(());
    }
    if content_length > 64 * 1024 * 1024 {
        stream.write_all(&answer(
            "413 Payload Too Large",
            r#"{"error":"bundle exceeds 64MB"}"#,
        ))?;
        return Ok(());
    }
    let mut body = head[body_off..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            bail!("connection closed mid-body");
        }
        body.extend_from_slice(&buf[..n]);
    }
    body.truncate(content_length);

    let bundle: Json = match serde_json::from_slice(&body) {
        Ok(j) => j,
        Err(e) => {
            stream.write_all(&answer(
                "400 Bad Request",
                &serde_json::json!({"error": format!("body is not JSON: {e}")}).to_string(),
            ))?;
            return Ok(());
        }
    };
    // A bundle may name its scenario; the id still has to be filesystem
    // + sid-safe — slug it like default_sid does for hosts.
    let sid = bundle["sid"]
        .as_str()
        .and_then(|s| slug_id(s).filter(|v| !v.is_empty() && v != "." && v != ".."));
    match ingest_bundle(&bundle, sid, "posted to the listen endpoint") {
        Ok(out) => {
            println!(
                "[ingest] {} via endpoint: {} steps, {} requests, {} files",
                out.sid, out.steps, out.requests, out.files
            );
            stream.write_all(&answer(
                "200 OK",
                &serde_json::json!({
                    "sid": out.sid,
                    "scenario": out.scenario_path.display().to_string(),
                    "steps": out.steps,
                    "requests": out.requests,
                    "files": out.files,
                    "warnings": out.warnings,
                })
                .to_string(),
            ))?;
        }
        Err(e) => {
            stream.write_all(&answer(
                "422 Unprocessable Entity",
                &serde_json::json!({"error": format!("{e:#}")}).to_string(),
            ))?;
        }
    }
    Ok(())
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// `[a-z0-9-_]`-safe slug for a bundle-supplied scenario id — the
/// listen endpoint can't trust the id to stay inside the root.
fn slug_id(s: &str) -> Option<String> {
    let slug: String = s
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

struct Opts {
    bundle: Option<PathBuf>,
    sid: Option<String>,
    listen: Option<u16>,
}

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    if let Some(port) = opts.listen {
        return listen(port);
    }
    let bundle_path = opts.bundle.ok_or_else(|| {
        anyhow::anyhow!("ingest: needs a bundle path — agent-qa ingest <bundle.json>")
    })?;
    let text = fs::read_to_string(&bundle_path)
        .with_context(|| format!("ingest: read {}", bundle_path.display()))?;
    let bundle: Json = serde_json::from_str(&text)
        .with_context(|| format!("ingest: parse {}", bundle_path.display()))?;
    let out = ingest_bundle(&bundle, opts.sid, &bundle_path.display().to_string())?;
    for w in &out.warnings {
        eprintln!("ingest: warning: {w}");
    }
    let har_note = if out.requests > 0 {
        format!(
            " + {} captured requests → replays/recorded/network.har",
            out.requests
        )
    } else {
        String::new()
    };
    let files_note = if out.files > 0 {
        format!(" + {} file(s) → files/", out.files)
    } else {
        String::new()
    };
    println!(
        "[ingest] {sid}: {steps} steps{har_note}{files_note}\n  scenario: {}\n  replay: agent-qa replay {sid}\n  hermetic: agent-qa replay {sid} --mock-from recorded --offline",
        out.scenario_path.display(),
        sid = out.sid,
        steps = out.steps,
    );
    Ok(0)
}

/// Decode a `data:[<type>][;base64],<payload>` URL into bytes. Only the
/// base64 form is handled — it's what FileReader.readAsDataURL emits.
fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let (head, payload) = url.split_once(',')?;
    if !head.starts_with("data:") || !head.contains(";base64") {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()
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

const DEFAULT_PORT: u16 = 17321;

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut bundle = None;
    let mut sid = None;
    let mut listen = None;
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
            "--listen" => {
                listen = Some(DEFAULT_PORT);
            }
            "--port" => {
                let v = it
                    .next()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("--port needs a value"))?;
                listen = Some(
                    v.parse::<u16>()
                        .ok()
                        .filter(|p| *p > 0)
                        .ok_or_else(|| anyhow::anyhow!("--port: {v:?} is not a valid port"))?,
                );
            }
            "-h" | "--help" => {
                println!(
                    "agent-qa ingest — turn an extension capture bundle into a scenario

USAGE
  agent-qa ingest <bundle.json> [--sid <name>]
  agent-qa ingest --listen [--port <n>]

The browser extension's one-button export (steps + network traffic) is
validated and written as <scenarios_root>/<sid>/scenario.json plus the
replays/recorded/network.har sidecar `--mock-from recorded` replays
against.

--listen runs a localhost endpoint the extension's export button POSTs
straight to — the recording lands as a scenario without the user ever
touching a file. The extension falls back to a download when nothing
listens, so the daemon is optional sugar.

ARGUMENTS
  <bundle.json>    path to the downloaded capture bundle
  --sid <name>     scenario id (default: capture-<host>-<timestamp>)
  --listen         serve POST /ingest on localhost (default port {DEFAULT_PORT})
  --port <n>       port for --listen"
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
    Ok(Opts {
        bundle,
        sid,
        listen,
    })
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
        assert_eq!(o.bundle.as_deref(), Some(std::path::Path::new("b.json")));
        assert_eq!(o.sid.as_deref(), Some("mine"));
        assert!(o.listen.is_none());
        assert!(parse_args(&["a".into(), "b".into()]).is_err());
        // No bundle and no --listen is legal at parse — run() reports it.
        assert!(parse_args(&[]).unwrap().bundle.is_none());
    }

    #[test]
    fn parse_args_listen_defaults_and_port() {
        assert_eq!(
            parse_args(&["--listen".into()]).unwrap().listen,
            Some(17321)
        );
        assert_eq!(
            parse_args(&["--listen".into(), "--port".into(), "9999".into()])
                .unwrap()
                .listen,
            Some(9999)
        );
        assert!(parse_args(&["--port".into(), "nope".into()]).is_err());
        assert!(parse_args(&["--port".into(), "0".into()]).is_err());
        assert!(parse_args(&["--port".into()]).is_err());
    }

    #[test]
    fn slug_id_makes_a_bundle_sid_filesystem_safe() {
        assert_eq!(slug_id("My Flow!").as_deref(), Some("my-flow"));
        assert_eq!(slug_id("../../../etc").as_deref(), Some("etc"));
        assert_eq!(slug_id("...").as_deref(), None);
        assert_eq!(slug_id("").as_deref(), None);
    }

    #[test]
    fn ingest_bundle_returns_outcome_with_warnings() {
        let _guard = crate::test_util::lock_env();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        let mut b = bundle(
            vec![click_draft()],
            vec![json!({"url":"https://x/a","method":"GET","status":200,"body":"hi"})],
        );
        b["warnings"] = json!(["capture paused for 1 non-http navigation"]);
        let out = ingest_bundle(&b, Some("w1".into()), "test").unwrap();
        assert_eq!(out.sid, "w1");
        assert_eq!(out.steps, 1);
        assert_eq!(out.requests, 1);
        assert_eq!(out.warnings.len(), 1);
        assert!(out.scenario_path.exists());
        assert!(tmp.path().join("w1/replays/recorded/network.har").exists());
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
    }

    #[test]
    fn listen_endpoint_ingests_a_posted_bundle() {
        let _guard = crate::test_util::lock_env();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_conn(stream).unwrap();
        });
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        use std::io::{Read, Write};
        let body = serde_json::to_vec(&json!({
            "version": 1,
            "url": "https://x/app",
            "sid": "posted!",
            "steps": [click_draft()],
        }))
        .unwrap();
        client
            .write_all(
                format!(
                    "POST /ingest HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .unwrap();
        client.write_all(&body).unwrap();
        let mut resp = String::new();
        client.read_to_string(&mut resp).unwrap();
        server.join().unwrap();
        assert!(resp.starts_with("HTTP/1.1 200"), "got: {resp}");
        assert!(resp.contains(r#""sid":"posted""#), "sid slugged: {resp}");
        assert!(tmp.path().join("posted/scenario.json").exists());
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
    }

    #[test]
    fn listen_endpoint_rejects_a_bad_body() {
        let _guard = crate::test_util::lock_env();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let _ = handle_conn(stream);
        });
        let mut client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        use std::io::{Read, Write};
        client
            .write_all(b"POST /ingest HTTP/1.1\r\nContent-Length: 4\r\n\r\nnope")
            .unwrap();
        let mut resp = String::new();
        client.read_to_string(&mut resp).unwrap();
        server.join().unwrap();
        assert!(resp.starts_with("HTTP/1.1 400"), "got: {resp}");
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
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
    fn ingest_refuses_to_overwrite_an_existing_scenario() {
        let _guard = crate::test_util::lock_env();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        let bundle_file = tmp.path().join("bundle.json");
        fs::write(
            &bundle_file,
            serde_json::to_string(&bundle(vec![click_draft()], vec![])).unwrap(),
        )
        .unwrap();
        let args = |sid: &str| {
            vec![
                bundle_file.display().to_string(),
                "--sid".to_string(),
                sid.to_string(),
            ]
        };
        assert_eq!(run(&args("mine")).unwrap(), 0, "first ingest lands");
        let err = run(&args("mine")).unwrap_err().to_string();
        assert!(err.contains("refusing to overwrite"), "got: {err}");
        // And the original scenario survived — not a partial overwrite.
        let written = fs::read_to_string(tmp.path().join("mine").join("scenario.json")).unwrap();
        assert!(written.contains("\"mine\""), "original scenario intact");
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
    }

    #[test]
    fn ingest_materializes_inlined_upload_contents() {
        let _guard = crate::test_util::lock_env();
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        let step = json!({
            "kind": "do",
            "draft": {
                "intent": "upload report.pdf",
                "verb": "upload",
                "on": "css:#file",
                "value": {"from": "literal", "literal": "files/report.txt"}
            },
            "uploads": [
                {
                    "name": "report.txt",
                    "type": "text/plain",
                    "data": "data:text/plain;base64,aGVsbG8gd29ybGQ="
                },
                {"name": "big.bin", "skipped": "too large"},
                {"name": "../escape/evil.txt",
                 "data": "data:;base64,bmljZQ=="}
            ]
        });
        let bundle_file = tmp.path().join("bundle.json");
        fs::write(
            &bundle_file,
            serde_json::to_string(&bundle(vec![step], vec![])).unwrap(),
        )
        .unwrap();
        let rc = run(&[
            bundle_file.display().to_string(),
            "--sid".into(),
            "up".into(),
        ])
        .unwrap();
        assert_eq!(rc, 0);
        // Inlined contents land under files/; traversal attempts are
        // flattened to a basename inside the scenario dir.
        let written = fs::read(tmp.path().join("up/files/report.txt")).unwrap();
        assert_eq!(written, b"hello world");
        let escaped = fs::read(tmp.path().join("up/files/evil.txt")).unwrap();
        assert_eq!(escaped, b"nice");
        assert!(!tmp.path().join("escape").exists());
        // big.bin (skipped) is not materialized.
        assert!(!tmp.path().join("up/files/big.bin").exists());
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
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
