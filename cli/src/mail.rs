//! `mail` — read a test inbox from a scenario: poll a mailpit/mailhog-style
//! HTTP API for a matching message, extract a link or code from the body,
//! and bind it with `saveAs` for later steps (`type {{vars.otp}}`,
//! `goto {{vars.verifyLink}}`).
//!
//! The server is configured once in `agent-qa.toml`:
//!
//!   [mail]
//!   url = "http://localhost:8025"
//!
//! Both mailpit (`/api/v1/messages` + `/api/v1/message/<id>`) and mailhog
//! (`/api/v2/messages`, bodies inline) are supported — detected by response
//! shape, no config flag.
//!
//! Scenario shape:
//!
//!   {"id":"s9","kind":"do","verb":"mail",
//!    "params":{"to":"*@example.test","subject":"*verify*",
//!              "extract":"link","timeoutMs":30000},
//!    "saveAs":"verifyUrl"}
//!
//! `extract`: "link" = first https?:// URL in the body, "code" = first
//! 4–8 digit run, anything else = a regex (capture group 1 wins, else the
//! whole match). Without `extract` the bound value is the whole text body.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::paths;
use crate::value::{substitute_scenario_vars, ValueScope};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const POLL_MS: u64 = 1_000;

#[derive(Debug)]
pub(crate) struct Message {
    id: String,
    subject: String,
    to: Vec<String>,
    /// Text body for mailhog (fetched inline); empty for mailpit until
    /// `body()` is called on the winner.
    text: String,
}

/// `mail` do-step: poll the inbox for the newest message matching the
/// params, extract, return the bound value. `to`/`subject` are globs;
/// both absent = newest message wins (useful for a per-test mailbox).
pub(crate) fn wait_message(
    params: Option<&BTreeMap<String, Json>>,
    scope: &mut ValueScope,
    step_id: &str,
) -> Result<Json> {
    let base = mail_base()?;
    let mut get = |k: &str| -> Option<String> {
        params
            .and_then(|p| p.get(k))
            .and_then(|v| v.as_str())
            .map(|s| substitute_scenario_vars(s, scope))
    };
    let to = get("to");
    let subject = get("subject");
    let extract = get("extract");
    let timeout_ms = params
        .and_then(|p| p.get("timeoutMs"))
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_TIMEOUT_MS);

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match latest_match(&base, to.as_deref(), subject.as_deref()) {
            Ok(Some(msg)) => {
                let body = if msg.text.is_empty() {
                    fetch_body(&base, &msg.id).unwrap_or_default()
                } else {
                    msg.text
                };
                return apply_extract(extract.as_deref(), &body, step_id);
            }
            Ok(None) => {}
            Err(e) => {
                // A transient API failure isn't a miss — keep polling until
                // the deadline like an absent message would.
                eprintln!("mail: poll error ({e:#}) — retrying until timeout");
            }
        }
        if Instant::now() >= deadline {
            bail!(
                "mail: no message matching to={:?} subject={:?} within {}ms",
                to.unwrap_or_default(),
                subject.unwrap_or_default(),
                timeout_ms
            );
        }
        std::thread::sleep(Duration::from_millis(POLL_MS));
    }
}

fn mail_base() -> Result<String> {
    let (cfg_path, t) = paths::mail_config().ok_or_else(|| {
        anyhow::anyhow!(
            "mail: no [mail] table in agent-qa.toml — point it at a mailpit/mailhog API, e.g.\n  [mail]\n  url = \"http://localhost:8025\""
        )
    })?;
    t.url
        .filter(|u| !u.trim().is_empty())
        .map(|u| u.trim_end_matches('/').to_string())
        .with_context(|| {
            format!(
                "mail: [mail] in {} needs url = \"http://<host>:<port>\"",
                cfg_path.display()
            )
        })
}

/// Newest message matching the filters, or None. Both backends return a
/// list ordered newest-first; the filters are `*`-globs over the recipient
/// addresses and the decoded subject.
fn latest_match(base: &str, to: Option<&str>, subject: Option<&str>) -> Result<Option<Message>> {
    for msg in list_messages(base)? {
        let to_ok = to
            .map(|g| msg.to.iter().any(|a| crate::runner::glob_match(g, a)))
            .unwrap_or(true);
        let subject_ok = subject
            .map(|g| crate::runner::glob_match(g, &msg.subject))
            .unwrap_or(true);
        if to_ok && subject_ok {
            return Ok(Some(msg));
        }
    }
    Ok(None)
}

fn get_json(url: &str) -> Result<Json> {
    let res = ureq::get(url)
        .timeout(Duration::from_secs(10))
        .call()
        .with_context(|| format!("mail: GET {url}"))?;
    res.into_json::<Json>()
        .with_context(|| format!("mail: GET {url} did not return JSON"))
}

/// Mailpit `/api/v1/messages` answers `{messages:[…], total}`; mailhog
/// `/api/v2/messages` answers `{items:[…]}` (and 404s on /v1 — detect
/// whichever shape responds).
fn list_messages(base: &str) -> Result<Vec<Message>> {
    if let Ok(v) = get_json(&format!("{base}/api/v1/messages?limit=50")) {
        if let Some(list) = v.get("messages").and_then(|m| m.as_array()) {
            return Ok(list
                .iter()
                .filter_map(|m| {
                    let to = m
                        .get("To")
                        .and_then(|t| t.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|e| e.get("Address").and_then(|s| s.as_str()))
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(Message {
                        id: m.get("ID")?.as_str()?.to_string(),
                        subject: decode_header(
                            m.get("Subject").and_then(|s| s.as_str()).unwrap_or(""),
                        ),
                        to,
                        text: String::new(),
                    })
                })
                .collect());
        }
    }
    let v = get_json(&format!("{base}/api/v2/messages?limit=50"))?;
    let items = v
        .get("items")
        .and_then(|i| i.as_array())
        .context("mail: unrecognized API shape — expected mailpit /api/v1 or mailhog /api/v2")?;
    Ok(items
        .iter()
        .filter_map(|m| {
            let content = m.get("Content")?;
            let headers = content.get("Headers");
            let subject = headers
                .and_then(|h| h.get("Subject"))
                .and_then(|s| s.as_array())
                .and_then(|a| a.first())
                .and_then(|s| s.as_str())
                .map(decode_header)
                .unwrap_or_default();
            let to = headers
                .and_then(|h| h.get("To"))
                .and_then(|s| s.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str())
                        .map(|addr| {
                            // "Name <addr@x>" → the bare address for globbing.
                            addr.rsplit('<')
                                .next()
                                .unwrap_or(addr)
                                .trim_end_matches('>')
                                .trim()
                                .to_string()
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Message {
                id: m.get("ID")?.as_str()?.to_string(),
                subject,
                to,
                text: qp_decode(content.get("Body").and_then(|b| b.as_str()).unwrap_or("")),
            })
        })
        .collect())
}

/// Mailpit full message: `Text` is the plain body, `HTML` the rich part.
/// Fall back to tag-stripped HTML when the sender only produced HTML.
fn fetch_body(base: &str, id: &str) -> Result<String> {
    let v = get_json(&format!("{base}/api/v1/message/{id}"))?;
    if let Some(t) = v.get("Text").and_then(|s| s.as_str()) {
        if !t.trim().is_empty() {
            return Ok(t.to_string());
        }
    }
    Ok(v.get("HTML")
        .and_then(|s| s.as_str())
        .map(strip_html)
        .unwrap_or_default())
}

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// RFC 2047 encoded-word decode (`=?UTF-8?B?…?=` / `?Q?`) — test senders
/// encode subjects when they carry non-ASCII; mailhog keeps the raw header.
fn decode_header(h: &str) -> String {
    let mut out = h.to_string();
    let re = regex::Regex::new(r"=\?([-\w]+)\?([BbQq])\?([^?]*)\?=").unwrap();
    while let Some(m) = re.captures(&out) {
        let (enc, body) = (m[2].to_string(), m[3].to_string());
        let decoded = if enc.eq_ignore_ascii_case("b") {
            decode_b64(&body)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_else(|| body.clone())
        } else {
            qp_decode(&body.replace('_', " "))
        };
        let span = m.get(0).unwrap();
        out.replace_range(span.start()..span.end(), &decoded);
    }
    out
}

fn decode_b64(s: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.decode(s.trim())?)
}

/// Quoted-printable: `=` + CRLF soft breaks, `=XX` bytes. UTF-8 sequences
/// are decoded at the byte level then re-read as UTF-8 (lossy).
fn qp_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'\n' || bytes[i + 1] == b'\r' {
                i += if bytes.get(i + 1) == Some(&b'\r') && bytes.get(i + 2) == Some(&b'\n') {
                    3
                } else {
                    2
                };
                continue;
            }
            if i + 2 < bytes.len() {
                if let Ok(v) =
                    u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
                {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// "link" / "code" shorthands, otherwise a regex (group 1 preferred).
fn apply_extract(extract: Option<&str>, body: &str, step_id: &str) -> Result<Json> {
    match extract {
        None => Ok(Json::String(body.trim().to_string())),
        Some("link") => extract_first(body, r#"https?://[^\s"'<>\)\]]+"#, step_id, "link"),
        Some("code") => extract_first(body, r"\b(\d{4,8})\b", step_id, "code"),
        Some(re) => extract_first(body, re, step_id, re),
    }
}

fn extract_first(body: &str, pattern: &str, step_id: &str, label: &str) -> Result<Json> {
    let re = regex::Regex::new(pattern)
        .with_context(|| format!("step '{step_id}' mail: bad extract regex {label:?}"))?;
    let caps = re.captures(body).with_context(|| {
        format!("step '{step_id}' mail: extract {label:?} found no match in the message body")
    })?;
    let hit = caps.get(1).or_else(|| caps.get(0)).unwrap();
    Ok(Json::String(hit.as_str().to_string()))
}

/// `agent-qa mail list|read|wait|delete` — manual probing of the configured
/// inbox, same config and matching rules as the verb.
pub fn run(args: &[String]) -> Result<u8> {
    let mut it = args.iter().peekable();
    match it.next().map(|s| s.as_str()) {
        None | Some("-h") | Some("--help") | Some("help") => {
            println!(
                "agent-qa mail — probe the configured test inbox\n\nUsage:\n  agent-qa mail list [--limit N]\n  agent-qa mail read <id>\n  agent-qa mail wait [--to <glob>] [--subject <glob>]\n                     [--extract link|code|<regex>] [--timeout <ms>]\n  agent-qa mail delete <id>|--all\n\nInbox is [mail] url in agent-qa.toml (mailpit or mailhog API base).\nwait prints the extracted value; without --extract the whole text body."
            );
            Ok(0)
        }
        Some("list") => {
            let limit = parse_opt::<usize>(&mut it, "--limit")?.unwrap_or(20);
            let base = mail_base()?;
            for (i, m) in list_messages(&base)?.iter().take(limit).enumerate() {
                println!("{}\t{}\t{}\t{}", i + 1, m.id, m.to.join(","), m.subject);
            }
            Ok(0)
        }
        Some("read") => {
            let id = it.next().context("mail read requires an id")?;
            let base = mail_base()?;
            // Read by list-id: mailpit ids are opaque, so resolve through
            // the list to reuse both backends' shape.
            let msg = list_messages(&base)?
                .into_iter()
                .find(|m| m.id == *id)
                .with_context(|| format!("mail: no message with id {id}"))?;
            let body = if msg.text.is_empty() {
                fetch_body(&base, &msg.id)?
            } else {
                msg.text
            };
            println!("to:      {}", msg.to.join(", "));
            println!("subject: {}", msg.subject);
            println!();
            println!("{body}");
            Ok(0)
        }
        Some("wait") => {
            let mut to = None;
            let mut subject = None;
            let mut extract = None;
            let mut timeout_ms = DEFAULT_TIMEOUT_MS;
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--to" => to = Some(it.next().context("--to needs a value")?.clone()),
                    "--subject" => {
                        subject = Some(it.next().context("--subject needs a value")?.clone())
                    }
                    "--extract" => {
                        extract = Some(it.next().context("--extract needs a value")?.clone())
                    }
                    "--timeout" => {
                        timeout_ms = it
                            .next()
                            .and_then(|v| v.parse::<u64>().ok())
                            .context("--timeout expects milliseconds")?
                    }
                    other => bail!("mail wait: unknown flag {other:?}"),
                }
            }
            let base = mail_base()?;
            let deadline = Instant::now() + Duration::from_millis(timeout_ms);
            loop {
                if let Some(msg) = latest_match(&base, to.as_deref(), subject.as_deref())? {
                    let body = if msg.text.is_empty() {
                        fetch_body(&base, &msg.id).unwrap_or_default()
                    } else {
                        msg.text
                    };
                    let v = apply_extract(extract.as_deref(), &body, "mail wait")?;
                    println!("{}", v.as_str().unwrap_or_default());
                    return Ok(0);
                }
                if Instant::now() >= deadline {
                    bail!("mail wait: no matching message within {timeout_ms}ms");
                }
                std::thread::sleep(Duration::from_millis(POLL_MS));
            }
        }
        Some("delete") => {
            let base = mail_base()?;
            match it.next().map(|s| s.as_str()) {
                Some("--all") => {
                    // Mailpit: DELETE /api/v1/messages (empty body = all).
                    ureq::delete(&format!("{base}/api/v1/messages"))
                        .timeout(Duration::from_secs(10))
                        .send_json(json!({}))
                        .context("mail delete --all")?;
                    Ok(0)
                }
                Some(id) => {
                    ureq::delete(&format!("{base}/api/v1/message/{id}"))
                        .timeout(Duration::from_secs(10))
                        .call()
                        .with_context(|| format!("mail delete {id}"))?;
                    Ok(0)
                }
                None => bail!("usage: agent-qa mail delete <id>|--all"),
            }
        }
        Some(other) => bail!("unknown mail subcommand {other:?} — expected list|read|wait|delete"),
    }
}

fn parse_opt<T: std::str::FromStr>(
    it: &mut std::iter::Peekable<std::slice::Iter<String>>,
    flag: &str,
) -> Result<Option<T>> {
    match it.next() {
        None => Ok(None),
        Some(a) if a == flag => Ok(Some(
            it.next()
                .and_then(|v| v.parse::<T>().ok())
                .with_context(|| format!("{flag} needs a value"))?,
        )),
        Some(a) => bail!("mail list: unknown flag {a:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qp_decode_soft_breaks_and_hex() {
        assert_eq!(qp_decode("hello=20world=\r\nrest"), "hello worldrest");
        assert_eq!(qp_decode("a=C3=A9b"), "a\u{e9}b");
        assert_eq!(qp_decode("plain"), "plain");
    }

    #[test]
    fn decode_header_base64_encoded_word() {
        // "Verify =?UTF-8?B?Y2llbHM=?=" … plus a plain pass-through.
        assert_eq!(decode_header("=?UTF-8?B?VmVyaWZ5?="), "Verify");
        assert_eq!(decode_header("plain subject"), "plain subject");
    }

    #[test]
    fn extract_link_and_code() {
        let body = "Hi, confirm at https://app.test/verify?tok=abc123 now.";
        let v = apply_extract(Some("link"), body, "s1").unwrap();
        assert_eq!(v, "https://app.test/verify?tok=abc123");
        let v = apply_extract(Some("code"), "your code is 482913", "s1").unwrap();
        assert_eq!(v, "482913");
    }

    #[test]
    fn extract_regex_group_one_wins() {
        let body = "OTP: 991122 expires soon";
        let v = apply_extract(Some(r"OTP:\s*(\d+)"), body, "s1").unwrap();
        assert_eq!(v, "991122");
        // No group → whole match.
        let v = apply_extract(Some(r"\d+"), body, "s1").unwrap();
        assert_eq!(v, "991122");
        assert!(apply_extract(Some("zzz("), body, "s1").is_err());
        assert!(apply_extract(Some("nope-\\d"), body, "s1").is_err());
    }

    #[test]
    fn mail_base_requires_config_and_url() {
        // In a temp dir with no agent-qa.toml the error names the fix.
        let tmp = tempfile::TempDir::new().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let e = mail_base().unwrap_err().to_string();
        std::env::set_current_dir(prev).unwrap();
        assert!(e.contains("[mail]"), "{e}");
    }
}
