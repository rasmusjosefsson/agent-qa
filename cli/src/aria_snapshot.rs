//! `aria-snapshot` verb — dump the live page's accessibility tree.
//!
//! The authoring editor's element picker renders this tree so the
//! author can click a node and capture its `role` + accessible `name`
//! as a stable, deterministic target — agent-qa's accessibility-first
//! targeting, with no runtime-AI resolution.
//!
//! It is a thin, read-only adapter over agent-browser's snapshot output
//! (`browser::snapshot_full` / `snapshot_interactive`): it parses the
//! indented `- role "name" [… ref=eN]` text into the structured rows the
//! editor's picker renders. (For the raw text, use `agent-browser snapshot`.)
//!
//! CLI shape:
//!
//!   agent-qa aria-snapshot [--session <name>] [--interactive]
//!
//! `--interactive` limits the tree to actionable elements (smaller, the
//! default pick surface); without it the full tree is returned. Output is
//! always `{ "session": "...", "interactive": bool,
//!   "nodes": [ {depth, role, name, ref, pickable, attrs} ] }`.

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::json;

use crate::browser;
use crate::recorder_state::RecorderState;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let session = resolve_session(opts.session.as_deref())?;
    let snapshot = if opts.interactive {
        browser::snapshot_interactive(&session)?
    } else {
        browser::snapshot_full(&session)?
    };
    let nodes = parse_snapshot(&snapshot);
    println!(
        "{}",
        serde_json::to_string(&json!({
            "session": session,
            "interactive": opts.interactive,
            "nodes": nodes,
        }))?
    );
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa aria-snapshot — dump the live page's accessibility tree

Usage:
  agent-qa aria-snapshot [--session <name>] [--interactive]

Reads the ARIA tree from the live agent-browser session. The authoring
editor's element picker renders this so you can pick a node and capture
its role + accessible name as a deterministic target.

Always emits structured JSON rows (for raw text use `agent-browser snapshot`):
  {{\"session\":\"…\",\"interactive\":false,\"nodes\":[
    {{\"depth\":0,\"role\":\"button\",\"name\":\"Login\",\"ref\":\"e5\",
     \"pickable\":true,\"attrs\":\"…\"}}]}}

Flags:
  --interactive   Limit to actionable elements (smaller pick surface).

Read-only: aria-snapshot never drives the page or records anything."
    );
}

#[derive(Debug, Clone, Default)]
struct Opts {
    session: Option<String>,
    interactive: bool,
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
            "--interactive" => opts.interactive = true,
            other => bail!("unknown flag {other:?}"),
        }
    }
    Ok(opts)
}

fn resolve_session(explicit: Option<&str>) -> Result<String> {
    if let Some(session) = explicit {
        return Ok(session.to_string());
    }
    Ok(RecorderState::try_load_active()?
        .map(|state| state.session)
        .unwrap_or_else(|| "default".to_string()))
}

/// One parsed accessibility-tree node.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct AriaNode {
    depth: usize,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "ref")]
    node_ref: Option<String>,
    /// True when the node carries a non-empty accessible name — the rows
    /// the picker should let you target by role+name.
    pickable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    attrs: Option<String>,
}

/// Parse an agent-browser ARIA snapshot into structured rows.
///
/// Lines look like (2-space indent per tree level):
///   `- button "Resources" [expanded=false, ref=e54]`
///   `  - link "Docs" [ref=e69]`
///   `- generic`
///
/// We compute `depth` from the leading indent, the `role` as the first
/// token, the quoted `name` if present, and pull `ref=eN` + the remaining
/// bracket attributes out. Lines we can't parse are skipped (never fatal).
fn parse_snapshot(snapshot: &str) -> Vec<AriaNode> {
    let mut out = Vec::new();
    for raw in snapshot.lines() {
        // Indentation → depth (2 spaces per level), measured before the
        // list dash.
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        let depth = indent / 2;
        let body = raw.trim_start();
        let body = match body.strip_prefix("- ") {
            Some(rest) => rest,
            None => match body.strip_prefix('-') {
                Some(rest) => rest.trim_start(),
                None => continue, // YAML-ish header lines etc.
            },
        };
        if body.is_empty() {
            continue;
        }
        // Role: up to the first space, quote, or bracket.
        let role_end = body.find([' ', '"', '[']).unwrap_or(body.len());
        let role = body[..role_end].trim().to_string();
        if role.is_empty() {
            continue;
        }
        let rest = &body[role_end..];

        // Name: the first double-quoted run, if any (before the `[`).
        let bracket_at = rest.find('[');
        let name_search = match bracket_at {
            Some(i) => &rest[..i],
            None => rest,
        };
        let name = extract_quoted(name_search);

        // Attributes / ref live inside `[...]`.
        let (node_ref, attrs) = match bracket_at {
            Some(i) => parse_brackets(&rest[i..]),
            None => (None, None),
        };

        let pickable = name.as_deref().map(|n| !n.is_empty()).unwrap_or(false);
        out.push(AriaNode {
            depth,
            role,
            name,
            node_ref,
            pickable,
            attrs,
        });
    }
    out
}

/// Extract the first `"..."`-quoted substring (no escape handling — ARIA
/// names from agent-browser don't embed quotes).
fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('"')?;
    let rest = &s[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Parse a `[attr1, attr2, ref=eN]` trailer into `(ref, other-attrs)`.
fn parse_brackets(s: &str) -> (Option<String>, Option<String>) {
    let inner = s.trim_start_matches('[').trim_end();
    let inner = inner.strip_suffix(']').unwrap_or(inner);
    let mut node_ref = None;
    let mut others: Vec<&str> = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(r) = part.strip_prefix("ref=") {
            node_ref = Some(r.trim().to_string());
        } else {
            others.push(part);
        }
    }
    let attrs = if others.is_empty() {
        None
    } else {
        Some(others.join(", "))
    };
    (node_ref, attrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_flags() {
        let o = parse_args(&["--session".into(), "sx".into(), "--interactive".into()]).unwrap();
        assert_eq!(o.session.as_deref(), Some("sx"));
        assert!(o.interactive);
    }

    #[test]
    fn parse_args_rejects_unknown_flag() {
        parse_args(&["--nope".into()]).unwrap_err();
    }

    #[test]
    fn parses_role_name_ref_and_depth() {
        let snap =
            "- button \"Resources\" [expanded=false, ref=e54]\n  - link \"Docs\" [ref=e69]\n";
        let nodes = parse_snapshot(snap);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].depth, 0);
        assert_eq!(nodes[0].role, "button");
        assert_eq!(nodes[0].name.as_deref(), Some("Resources"));
        assert_eq!(nodes[0].node_ref.as_deref(), Some("e54"));
        assert_eq!(nodes[0].attrs.as_deref(), Some("expanded=false"));
        assert!(nodes[0].pickable);
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[1].role, "link");
        assert_eq!(nodes[1].name.as_deref(), Some("Docs"));
        assert_eq!(nodes[1].node_ref.as_deref(), Some("e69"));
        assert_eq!(nodes[1].attrs, None);
    }

    #[test]
    fn unnamed_node_is_not_pickable() {
        let nodes = parse_snapshot("- generic\n- list [ref=e2]\n");
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].role, "generic");
        assert!(nodes[0].name.is_none());
        assert!(!nodes[0].pickable);
        assert_eq!(nodes[1].role, "list");
        assert_eq!(nodes[1].node_ref.as_deref(), Some("e2"));
        assert!(!nodes[1].pickable);
    }

    #[test]
    fn skips_unparseable_lines() {
        let nodes = parse_snapshot("# yaml header\n\n   \n- textbox \"Email\" [ref=e1]\n");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].role, "textbox");
        assert_eq!(nodes[0].name.as_deref(), Some("Email"));
    }

    #[test]
    fn handles_name_with_brackets_in_attrs() {
        let nodes = parse_snapshot("- checkbox \"Accept\" [checked=true, disabled, ref=e9]\n");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name.as_deref(), Some("Accept"));
        assert_eq!(nodes[0].node_ref.as_deref(), Some("e9"));
        assert_eq!(nodes[0].attrs.as_deref(), Some("checked=true, disabled"));
    }
}
