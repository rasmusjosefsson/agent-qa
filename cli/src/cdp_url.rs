//! `cdp-url` verb — print the live session's CDP WebSocket endpoint.
//!
//! The authoring editor's live-browser pane connects to this
//! endpoint to screencast the running agent-browser session into the UI
//! and forward clicks/keystrokes back. Read-only: this verb never drives
//! the page or records anything — it just surfaces the debugger URL.
//!
//! CLI shape:
//!
//!   agent-qa cdp-url [--session <name>] [--json]
//!
//! Without `--json` it prints the raw `ws://…` URL on one line. With
//! `--json` it emits `{ "session": "…", "url": "ws://…" }`.

use std::fs;

use anyhow::{bail, Result};
use serde_json::json;

use crate::browser;
use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let session = resolve_session(opts.session.as_deref());
    let url = browser::cdp_url(&session)?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string(&json!({ "session": session, "url": url }))?
        );
    } else {
        println!("{url}");
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa cdp-url — print the live session's CDP WebSocket endpoint

Usage:
  agent-qa cdp-url [--session <name>] [--json]

Surfaces the agent-browser session's Chrome DevTools Protocol URL so the
authoring editor's live-browser pane can screencast the session and
forward input. Read-only.

Flags:
  --session <name>  Session to read (default: recorder env SESSION, else \"default\").
  --json            Emit {{\"session\":\"…\",\"url\":\"ws://…\"}} instead of the raw URL."
    );
}

#[derive(Debug, Clone, Default)]
struct Opts {
    session: Option<String>,
    json: bool,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut opts = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--session" => opts.session = it.next().cloned(),
            s if s.starts_with("--session=") => {
                opts.session = Some(s["--session=".len()..].to_string())
            }
            "--json" => opts.json = true,
            other => bail!("unknown flag {other:?}"),
        }
    }
    Ok(opts)
}

fn resolve_session(explicit: Option<&str>) -> String {
    if let Some(s) = explicit {
        return s.to_string();
    }
    if let Ok(body) = fs::read_to_string(paths::record_env_file()) {
        if let Some(s) = body
            .lines()
            .find_map(|l| l.strip_prefix("SESSION=").map(|v| v.trim().to_string()))
        {
            if !s.is_empty() {
                return s;
            }
        }
    }
    "default".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_and_json_flags() {
        let o = parse_args(&["--session".into(), "rec".into(), "--json".into()]).unwrap();
        assert_eq!(o.session.as_deref(), Some("rec"));
        assert!(o.json);
    }

    #[test]
    fn parses_inline_session() {
        let o = parse_args(&["--session=alpha".into()]).unwrap();
        assert_eq!(o.session.as_deref(), Some("alpha"));
        assert!(!o.json);
    }

    #[test]
    fn rejects_unknown_flag() {
        assert!(parse_args(&["--nope".into()]).is_err());
    }

    #[test]
    fn explicit_session_wins_over_env() {
        assert_eq!(resolve_session(Some("explicit")), "explicit");
    }
}
