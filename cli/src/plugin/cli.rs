//! `plugins` verb — surface plugin discovery and ping each.
//!
//! Subverbs:
//!   plugins                         → list (default)
//!   plugins list                    → enumerate discovered plugins + kinds
//!   plugins doctor                  → ping each, report status
//!   plugins path <kind>             → print the binary path for a kind
//!
//! CLI flag `--plugin <path>` may be passed BEFORE the subverb to inject
//! an additional plugin binary (highest priority).

use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};

use super::discovery::{discover, DiscoveryOpts, DiscoverySource};
use super::host;

pub fn run(args: &[String]) -> Result<u8> {
    let (overrides, rest) = parse_plugin_overrides(args);
    let opts = DiscoveryOpts {
        cli_overrides: overrides,
        cwd: None,
    };

    match rest.first().map(String::as_str) {
        None | Some("list") => list(&opts, has_flag(&rest, "--json")),
        Some("doctor") => doctor(&opts, has_flag(&rest, "--json")),
        Some("path") => {
            let kind = rest
                .get(1)
                .ok_or_else(|| anyhow!("usage: plugins path <kind>"))?;
            path(&opts, kind)
        }
        Some("--help" | "-h" | "help") => {
            help();
            Ok(0)
        }
        Some(other) => bail!("unknown subverb {other:?}. Try: list | doctor | path <kind>"),
    }
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn help() {
    println!(
        "agent-qa plugins — manage plugin discovery

Usage:
  agent-qa plugins [list]            Enumerate discovered plugins + kinds
  agent-qa plugins doctor            Ping every plugin and report status
  agent-qa plugins path <kind>       Print the binary path serving <kind>
  agent-qa plugins list --json       Structured rows on stdout (binary,
                                     source, kinds, declared, pingFailed)
  agent-qa plugins doctor --json     Structured ping results (binary, source,
                                     ok, name, protocolVersion, kinds, error)

Flags (before the subverb):
  --plugin <path>      Inject an additional plugin binary (highest priority).
                       May be repeated.

Discovery (priority order):
  1. --plugin <path>
  2. agent-qa.toml [plugins] table (walked up from cwd)
  3. AGENT_QA_PLUGINS env var (colon-separated)
  4. $PATH, any executable named agent-qa-plugin-*"
    );
}

/// Strip `--plugin <path>` (and `--plugin=<path>`) tokens out of args.
/// Returns the overrides and the remaining args (verb + args).
fn parse_plugin_overrides(args: &[String]) -> (Vec<PathBuf>, Vec<String>) {
    let mut overrides = Vec::new();
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == "--plugin" {
            if let Some(p) = iter.next() {
                overrides.push(PathBuf::from(p));
            }
        } else if let Some(p) = a.strip_prefix("--plugin=") {
            overrides.push(PathBuf::from(p));
        } else {
            rest.push(a.clone());
        }
    }
    (overrides, rest)
}

fn list(opts: &DiscoveryOpts, json_out: bool) -> Result<u8> {
    let plugins = discover(opts)?;
    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Row {
            binary: String,
            source: String,
            kinds: Vec<String>,
            declared: bool,
            ping_failed: bool,
        }
        let mut rows: Vec<Row> = Vec::new();
        for p in plugins {
            let (kinds, declared, ping_failed) = if let Some(k) = &p.declared_kind {
                (vec![k.clone()], true, false)
            } else {
                match host::ping(&p.binary) {
                    Ok(pong) => (pong.kinds, false, false),
                    Err(_) => (vec![], false, true),
                }
            };
            rows.push(Row {
                binary: p.binary.display().to_string(),
                source: render_source(&p.source),
                kinds,
                declared,
                ping_failed,
            });
        }
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(0);
    }
    if plugins.is_empty() {
        println!("(no plugins discovered)");
        println!();
        println!("Try one of:");
        println!("  --plugin /path/to/agent-qa-plugin-<name>");
        println!("  echo '[plugins]\\nauth = \"/path/to/plugin\"' > agent-qa.toml");
        println!("  AGENT_QA_PLUGINS=/path/to/plugin agent-qa plugins list");
        println!("  ln -s /path/to/plugin /usr/local/bin/agent-qa-plugin-name");
        return Ok(0);
    }
    for p in plugins {
        let source = render_source(&p.source);
        let kinds = if let Some(k) = &p.declared_kind {
            format!("kinds=[{k}] (declared)")
        } else {
            // Ping to learn kinds; non-fatal on failure.
            match host::ping(&p.binary) {
                Ok(pong) => format!("kinds=[{}]", pong.kinds.join(",")),
                Err(_) => "kinds=(ping failed)".to_string(),
            }
        };
        println!("{}\n  source={source}\n  {kinds}", p.binary.display());
    }
    Ok(0)
}

fn doctor(opts: &DiscoveryOpts, json_out: bool) -> Result<u8> {
    let plugins = discover(opts)?;
    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Row {
            binary: String,
            source: String,
            ok: bool,
            name: Option<String>,
            protocol_version: Option<u32>,
            kinds: Vec<String>,
            error: Option<String>,
        }
        let mut rows: Vec<Row> = Vec::new();
        let mut failures = 0u8;
        for p in plugins {
            match host::ping(&p.binary) {
                Ok(pong) => rows.push(Row {
                    binary: p.binary.display().to_string(),
                    source: render_source(&p.source),
                    ok: true,
                    name: Some(pong.name),
                    protocol_version: Some(pong.protocol_version),
                    kinds: pong.kinds,
                    error: None,
                }),
                Err(e) => {
                    failures = failures.saturating_add(1);
                    rows.push(Row {
                        binary: p.binary.display().to_string(),
                        source: render_source(&p.source),
                        ok: false,
                        name: None,
                        protocol_version: None,
                        kinds: vec![],
                        error: Some(e.to_string()),
                    });
                }
            }
        }
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(if failures == 0 { 0 } else { 1 });
    }
    if plugins.is_empty() {
        println!("(no plugins discovered)");
        return Ok(0);
    }
    let mut failures = 0u8;
    for p in plugins {
        let source = render_source(&p.source);
        match host::ping(&p.binary) {
            Ok(pong) => {
                println!(
                    "OK  {} ({}) — name={}, protocol={}, kinds=[{}]",
                    p.binary.display(),
                    source,
                    pong.name,
                    pong.protocol_version,
                    pong.kinds.join(",")
                );
            }
            Err(e) => {
                failures = failures.saturating_add(1);
                println!("FAIL {} ({source})\n     {e}", p.binary.display());
            }
        }
    }
    Ok(if failures == 0 { 0 } else { 1 })
}

fn path(opts: &DiscoveryOpts, kind: &str) -> Result<u8> {
    let plugins = discover(opts)?;
    for p in &plugins {
        if let Some(declared) = &p.declared_kind {
            if declared == kind {
                println!("{}", p.binary.display());
                return Ok(0);
            }
        }
    }
    // Fall back to pinging undeclared plugins.
    for p in &plugins {
        if p.declared_kind.is_some() {
            continue;
        }
        if let Ok(pong) = host::ping(&p.binary) {
            if pong.kinds.iter().any(|k| k == kind) {
                println!("{}", p.binary.display());
                return Ok(0);
            }
        }
    }
    bail!("no plugin serves kind {kind:?}");
}

fn render_source(src: &DiscoverySource) -> String {
    match src {
        DiscoverySource::CliFlag => "--plugin".into(),
        DiscoverySource::ConfigFile(p) => format!("config:{}", p.display()),
        DiscoverySource::EnvVar => "AGENT_QA_PLUGINS".into(),
        DiscoverySource::Path => "$PATH".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_parsing_separated() {
        let args = vec!["--plugin".into(), "/a/b".into(), "doctor".into()];
        let (overrides, rest) = parse_plugin_overrides(&args);
        assert_eq!(overrides, vec![PathBuf::from("/a/b")]);
        assert_eq!(rest, vec!["doctor".to_string()]);
    }

    #[test]
    fn override_parsing_eq_form() {
        let args = vec!["--plugin=/a/b".into(), "list".into()];
        let (overrides, rest) = parse_plugin_overrides(&args);
        assert_eq!(overrides, vec![PathBuf::from("/a/b")]);
        assert_eq!(rest, vec!["list".to_string()]);
    }

    #[test]
    fn override_parsing_multiple() {
        let args = vec![
            "--plugin".into(),
            "/a".into(),
            "--plugin=/b".into(),
            "list".into(),
        ];
        let (overrides, _rest) = parse_plugin_overrides(&args);
        assert_eq!(overrides, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn has_flag_detects_token_anywhere() {
        assert!(has_flag(&["list".into(), "--json".into()], "--json"));
        assert!(has_flag(&["--json".into(), "list".into()], "--json"));
        assert!(!has_flag(&["list".into()], "--json"));
        assert!(!has_flag(&["--json=true".into()], "--json"));
    }
}
