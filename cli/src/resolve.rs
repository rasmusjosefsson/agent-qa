//! The `resolve` plugin kind — optional authoring-time element resolution.
//!
//! smart-click/smart-fill end in a resolver rung: when every deterministic
//! locator strategy misses, the page's interactive elements (lifted from the
//! ARIA snapshot) are handed to the configured `resolve` plugin together
//! with the author's description. The plugin's pick — a snapshot ref — is
//! acted on, and the recorded step keeps the candidate's concrete role+name,
//! so replay never needs the plugin. With no `resolve` plugin configured the
//! rung is skipped and behaviour is exactly as before.
//!
//! Request (stdin):
//!   { "description": "<what the author meant>",
//!     "role": "<preferred role or null>",
//!     "candidates": [{"ref":"e54","role":"button","name":"Save draft",
//!                     "line":"button \"Save draft\" [ref=e54]"}, ...] }
//! Response (stdout):
//!   { "ref": "e54", "confidence": 0.83 }   // confidence optional
//!   { "ref": null }                        // no confident pick → normal miss
//!
//! Batch form (speculative fan-out — one plugin call, many questions):
//!   { "descriptions": [{"id":"q1","description":"...","role":null}, ...],
//!     "candidates": [...] }                // candidates shared across all
//!   → { "answers": {"q1": {"ref": "e54", "confidence": 0.83}, ...} }
//! Plugins that do not understand `descriptions` must return a
//! no-pick per question or error; single-question callers are unaffected.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::browser;
use crate::plugin::{discovery, host};

/// Plugins get a bounded turn — a resolver that stalls must not wedge the
/// authoring session.
const INVOKE_TIMEOUT: Duration = Duration::from_secs(20);

/// How many snapshot candidates are offered per question. Decision models
/// handle hundreds of options in one Choice call, so the bound is about
/// payload size, not model capacity — 100 covers very dense pages.
const MAX_CANDIDATES: usize = 100;

/// Roles a user can plausibly point at. Static text and grouping nodes are
/// noise for a "which element did I mean" question.
const INTERACTIVE_ROLES: &[&str] = &[
    "button",
    "checkbox",
    "combobox",
    "link",
    "listbox",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "option",
    "radio",
    "searchbox",
    "slider",
    "spinbutton",
    "switch",
    "tab",
    "textbox",
    "treeitem",
];

pub(crate) const KIND: &str = "resolve";

/// One interactive element lifted out of the ARIA snapshot.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    /// Snapshot ref (`e54`) — the only handle guaranteed to hit this node.
    pub(crate) ref_id: String,
    pub(crate) role: String,
    pub(crate) name: String,
    /// The trimmed snapshot line; state flags (`[disabled]` etc.) are
    /// resolution context for the plugin.
    pub(crate) line: String,
}

/// The plugin's pick — a candidate plus its reported confidence (None when
/// the plugin doesn't score answers).
#[derive(Debug, Clone)]
pub(crate) struct ResolvedPick {
    pub(crate) candidate: Candidate,
    pub(crate) confidence: Option<f64>,
}

/// Locate the `resolve` plugin, if any is configured. Preference order:
/// a declared `[plugins] resolve = "<binary>"` entry, then any discovered
/// plugin that pings as serving the kind.
pub(crate) fn find_plugin() -> Result<Option<PathBuf>> {
    let plugins = discovery::discover(&discovery::DiscoveryOpts::default())?;
    for p in &plugins {
        if p.declared_kind.as_deref() == Some(KIND) {
            return Ok(Some(p.binary.clone()));
        }
    }
    for p in &plugins {
        if let Ok(pong) = host::ping(&p.binary) {
            if pong.kinds.iter().any(|k| k == KIND) {
                return Ok(Some(p.binary.clone()));
            }
        }
    }
    Ok(None)
}

/// Ask the resolve plugin to pick the element `description` refers to.
///
/// Returns Ok(None) when there's nothing to resolve against: no plugin
/// configured, no candidates in the snapshot, or the plugin answered with
/// no confident pick. Spawn/transport failures propagate — a configured but
/// broken resolver should be loud at authoring time, not silently off.
pub(crate) fn resolve_element(
    session: &str,
    description: &str,
    preferred_role: Option<&str>,
) -> Result<Option<ResolvedPick>> {
    resolve_inner(session, description, preferred_role, false)
}

/// Like [`resolve_element`] but offers ONLY candidates of `role` — for
/// verbs whose target must be a specific control kind (e.g. `select`
/// needs the combobox, not one of its options).
pub(crate) fn resolve_element_in_role(
    session: &str,
    description: &str,
    role: &str,
) -> Result<Option<ResolvedPick>> {
    resolve_inner(session, description, Some(role), true)
}

fn resolve_inner(
    session: &str,
    description: &str,
    preferred_role: Option<&str>,
    restrict_to_role: bool,
) -> Result<Option<ResolvedPick>> {
    let Some(binary) = find_plugin()? else {
        return Ok(None);
    };
    let snapshot = browser::snapshot_full(session)
        .map_err(|e| anyhow::anyhow!("resolve: snapshot for candidates: {e}"))?;
    let mut candidates = candidates_from_snapshot(&snapshot, preferred_role, MAX_CANDIDATES);
    if restrict_to_role {
        if let Some(role) = preferred_role {
            candidates.retain(|c| c.role == role);
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    let request = json!({
        "description": description,
        "role": preferred_role,
        "candidates": candidates
            .iter()
            .map(candidate_json)
            .collect::<Vec<_>>(),
    });
    let outcome = host::invoke(&binary, KIND, None, request, INVOKE_TIMEOUT)
        .map_err(|e| anyhow::anyhow!("resolve plugin {}: {e}", binary.display()))?;
    Ok(parse_pick(&outcome.response, &candidates))
}

fn candidate_json(c: &Candidate) -> Value {
    json!({
        "ref": c.ref_id,
        "role": c.role,
        "name": c.name,
        "line": c.line,
    })
}

/// Speculative fan-out: resolve MANY descriptions against the same live
/// snapshot in ONE plugin call. The shared candidate list is enumerated
/// once; the plugin answers every question in a single invocation
/// (Jev's batch-questions pattern — ~10x cheaper than one call each).
///
/// Returns one Option<ResolvedPick> per input description, in order.
/// Plugins that don't support the batch shape get the request anyway —
/// the contract tolerates `answers` missing question ids → None slots.
pub(crate) fn resolve_elements_batch(
    session: &str,
    descriptions: &[(String, Option<String>)],
) -> Result<Vec<Option<ResolvedPick>>> {
    let empty: Vec<Option<ResolvedPick>> = vec![None; descriptions.len()];
    let Some(binary) = find_plugin()? else {
        return Ok(empty);
    };
    let snapshot = browser::snapshot_full(session)
        .map_err(|e| anyhow::anyhow!("resolve: snapshot for candidates: {e}"))?;
    let candidates = candidates_from_snapshot(&snapshot, None, MAX_CANDIDATES);
    if candidates.is_empty() {
        return Ok(empty);
    }
    let request = json!({
        "descriptions": descriptions
            .iter()
            .enumerate()
            .map(|(i, (d, r))| json!({"id": format!("q{i}"), "description": d, "role": r}))
            .collect::<Vec<_>>(),
        "candidates": candidates.iter().map(candidate_json).collect::<Vec<_>>(),
    });
    let outcome = host::invoke(&binary, KIND, None, request, INVOKE_TIMEOUT)
        .map_err(|e| anyhow::anyhow!("resolve plugin {}: {e}", binary.display()))?;
    let answers = outcome
        .response
        .get("answers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    Ok((0..descriptions.len())
        .map(|i| {
            answers
                .get(&format!("q{i}"))
                .and_then(|a| parse_pick(a, &candidates))
        })
        .collect())
}

/// Map the plugin's response to a candidate: `{ref: "eN"}` picks that
/// candidate; `null`, a missing field, or an unknown ref all mean "no
/// confident pick" → None.
fn parse_pick(response: &Value, candidates: &[Candidate]) -> Option<ResolvedPick> {
    let ref_id = response.get("ref")?.as_str()?;
    let candidate = candidates.iter().find(|c| c.ref_id == ref_id)?.clone();
    let confidence = response.get("confidence").and_then(|v| v.as_f64());
    Some(ResolvedPick {
        candidate,
        confidence,
    })
}

/// Parse interactive candidates out of an ARIA snapshot. Lines look like
/// `  - button "Add item" [enabled, ref=e12]`. When `preferred_role` is
/// given, matching candidates sort first (snapshot order preserved inside
/// each group), then the list truncates at `limit`.
pub(crate) fn candidates_from_snapshot(
    snapshot: &str,
    preferred_role: Option<&str>,
    limit: usize,
) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = snapshot
        .lines()
        .filter_map(|line| parse_candidate_line(line.trim_start_matches([' ', '-', '\t'])))
        .collect();
    if let Some(role) = preferred_role {
        // Stable sort: preferred role first, everything else keeps order.
        out.sort_by_key(|c| usize::from(c.role != role));
    }
    out.truncate(limit);
    out
}

fn parse_candidate_line(line: &str) -> Option<Candidate> {
    let q1 = line.find('"')?;
    let role = line[..q1].trim();
    if !INTERACTIVE_ROLES.contains(&role) {
        return None;
    }
    let q2 = line[q1 + 1..].find('"')? + q1 + 1;
    let name = &line[q1 + 1..q2];
    if name.is_empty() {
        return None;
    }
    let ref_id = extract_ref(&line[q2..])?;
    Some(Candidate {
        ref_id,
        role: role.to_string(),
        name: name.to_string(),
        line: line.to_string(),
    })
}

/// Pull the `ref=eN` token out of a snapshot line's bracket tail.
fn extract_ref(tail: &str) -> Option<String> {
    let idx = tail.find("ref=")? + 4;
    let rest = &tail[idx..];
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    let r = &rest[..end];
    if r.is_empty() {
        None
    } else {
        Some(r.to_string())
    }
}

/// Resolve + act: run the plugin pick, then perform `act` on the chosen
/// element via its snapshot ref (`@eN` bypasses the role engine). Returns
/// the pick on success, None on no-plugin/no-pick; plugin transport errors
/// are warned to stderr and degrade to a miss so a flaky resolver never
/// kills an authoring session.
pub(crate) fn resolve_and_act(
    session: &str,
    description: &str,
    preferred_role: Option<&str>,
    act: browser::RoleAct,
    value: Option<&str>,
) -> Option<ResolvedPick> {
    let pick = match resolve_element(session, description, preferred_role) {
        Ok(pick) => pick?,
        Err(e) => {
            eprintln!("resolve plugin: {e:#} — continuing without it");
            return None;
        }
    };
    let sel = format!("@{}", pick.candidate.ref_id);
    match browser::selector_act(session, &sel, act, value) {
        Ok(()) => Some(pick),
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------
// `agent-qa resolve` — authoring probe: resolve a description to an element.

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let session = match opts.session {
        Some(s) => s,
        None => crate::recorder_state::RecorderState::load_active()?.session,
    };

    if find_plugin()?.is_none() {
        anyhow::bail!(
            "no `resolve` plugin configured — add one to agent-qa.toml:\n  [plugins]\n  resolve = \"<plugin-binary>\""
        );
    }
    if let Some(descriptions) = &opts.batch {
        return run_batch(&session, descriptions, opts.json);
    }
    match resolve_element(&session, &opts.description, opts.role.as_deref())? {
        Some(pick) => {
            if opts.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ref": pick.candidate.ref_id,
                        "role": pick.candidate.role,
                        "name": pick.candidate.name,
                        "confidence": pick.confidence,
                    }))?
                );
            } else {
                let conf = pick
                    .confidence
                    .map(|c| format!(" (confidence {:.2})", c))
                    .unwrap_or_default();
                println!(
                    "resolved {:?} → {} {:?} [{}]{}",
                    opts.description,
                    pick.candidate.role,
                    pick.candidate.name,
                    pick.candidate.ref_id,
                    conf
                );
            }
            Ok(0)
        }
        None => {
            println!("resolved {:?} → no confident pick", opts.description);
            Ok(1)
        }
    }
}

fn run_batch(session: &str, descriptions: &[String], json_out: bool) -> Result<u8> {
    let qs: Vec<(String, Option<String>)> =
        descriptions.iter().map(|d| (d.clone(), None)).collect();
    let picks = resolve_elements_batch(session, &qs)?;
    let mut misses = 0u8;
    if json_out {
        let rows: Vec<Value> = descriptions
            .iter()
            .zip(&picks)
            .map(|(d, p)| match p {
                Some(p) => json!({
                    "description": d,
                    "ref": p.candidate.ref_id,
                    "role": p.candidate.role,
                    "name": p.candidate.name,
                    "confidence": p.confidence,
                }),
                None => json!({"description": d, "ref": null}),
            })
            .collect();
        misses = picks.iter().filter(|p| p.is_none()).count() as u8;
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for (d, p) in descriptions.iter().zip(&picks) {
            match p {
                Some(p) => {
                    let conf = p
                        .confidence
                        .map(|c| format!(" (confidence {c:.2})"))
                        .unwrap_or_default();
                    println!(
                        "resolved {d:?} → {} {:?} [{}]{conf}",
                        p.candidate.role, p.candidate.name, p.candidate.ref_id
                    );
                }
                None => {
                    misses += 1;
                    println!("resolved {d:?} → no confident pick");
                }
            }
        }
    }
    Ok(if misses == 0 { 0 } else { 1 })
}

fn print_help() {
    println!(
        "agent-qa resolve - ask the resolve plugin which element a description means\n\nUsage:\n  agent-qa resolve \"<description>\" [--role <role>] [--session <name>] [--json]\n  agent-qa resolve --batch <descriptions...> [--session <name>] [--json]\n\nEnumerates the live page's interactive elements (ARIA snapshot), hands\nthem + the description to the configured `resolve` plugin, and prints the\npick (role, name, ref, confidence). Exits 1 when the plugin has no\nconfident pick. smart-click and smart-fill run the same resolution as\ntheir last fallback rung automatically.\n\n--batch resolves several descriptions in ONE plugin call (the plugin's\nspeculative fan-out: one snapshot + one API round-trip for all questions).\n\nPlugins are configured in agent-qa.toml: [plugins] resolve = \"<binary>\"."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    description: String,
    /// --batch mode: multiple descriptions resolved in one plugin call.
    batch: Option<Vec<String>>,
    role: Option<String>,
    session: Option<String>,
    json: bool,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut description: Option<String> = None;
    let mut batch: Option<Vec<String>> = None;
    let mut role = None;
    let mut session = None;
    let mut json = false;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--batch" => {
                // Everything after --batch is a description (stop at a flag).
                let mut ds = Vec::new();
                while let Some(n) = it.peek() {
                    if n.starts_with('-') {
                        break;
                    }
                    ds.push(it.next().unwrap().clone());
                }
                if ds.is_empty() {
                    anyhow::bail!("resolve: --batch requires at least one description");
                }
                batch = Some(ds);
            }
            "--role" => {
                role = Some(
                    it.next()
                        .context("resolve: --role requires a value")?
                        .clone(),
                )
            }
            "--session" => {
                session = Some(
                    it.next()
                        .context("resolve: --session requires a value")?
                        .clone(),
                )
            }
            "--json" => json = true,
            s if s.starts_with('-') => anyhow::bail!("resolve: unknown flag {s}"),
            s => {
                if description.is_some() {
                    anyhow::bail!("resolve: unexpected argument {s}");
                }
                description = Some(s.to_string());
            }
        }
    }
    if batch.is_none() && description.is_none() {
        anyhow::bail!("resolve: <description> is required");
    }
    Ok(Opts {
        description: description.unwrap_or_default(),
        batch,
        role,
        session,
        json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAP: &str = r#"RootWebArea "Settings"
  - heading "Account"
  - button "Delete account" [ref=e10]
  - textbox "Display name" [ref=e11]
  - text "some static copy"
  - link "Billing" [ref=e12]
  - button "Save draft" [disabled, ref=e13]
  - group "danger zone"
    - checkbox "I understand" [checked=true, ref=e14]
"#;

    #[test]
    fn candidates_parse_interactive_roles_with_refs() {
        let c = candidates_from_snapshot(SNAP, None, 40);
        let names: Vec<&str> = c.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Delete account",
                "Display name",
                "Billing",
                "Save draft",
                "I understand"
            ]
        );
        assert_eq!(c[0].ref_id, "e10");
        assert_eq!(c[3].line, "button \"Save draft\" [disabled, ref=e13]");
    }

    #[test]
    fn candidates_prefer_role_then_stable_order() {
        let c = candidates_from_snapshot(SNAP, Some("textbox"), 40);
        assert_eq!(c[0].role, "textbox");
        assert_eq!(c[1].role, "button");
        assert_eq!(c[1].name, "Delete account"); // snapshot order kept
    }

    #[test]
    fn candidates_truncate_at_limit() {
        let c = candidates_from_snapshot(SNAP, None, 2);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn candidates_skip_nameless_and_refless() {
        let snap = "- button [ref=e1]\n- button \"x\" [ref=]\n- textbox \"y\" [ref=e9]";
        let c = candidates_from_snapshot(snap, None, 40);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].name, "y");
    }

    #[test]
    fn parse_pick_maps_ref_to_candidate() {
        let c = candidates_from_snapshot(SNAP, None, 40);
        let pick = parse_pick(&json!({"ref": "e13", "confidence": 0.77}), &c).unwrap();
        assert_eq!(pick.candidate.name, "Save draft");
        assert_eq!(pick.confidence, Some(0.77));
    }

    #[test]
    fn parse_pick_treats_null_unknown_ref_and_missing_as_no_pick() {
        let c = candidates_from_snapshot(SNAP, None, 40);
        assert!(parse_pick(&json!({"ref": null}), &c).is_none());
        assert!(parse_pick(&json!({"ref": "e999"}), &c).is_none());
        assert!(parse_pick(&json!({}), &c).is_none());
    }

    #[test]
    fn parse_args_batch_collects_descriptions_until_flag() {
        let args = vec![
            "--batch".into(),
            "the save button".into(),
            "the export thing".into(),
            "--session".into(),
            "s".into(),
        ];
        let o = parse_args(&args).unwrap();
        assert_eq!(
            o.batch.unwrap(),
            vec![
                "the save button".to_string(),
                "the export thing".to_string()
            ]
        );
        assert_eq!(o.session.as_deref(), Some("s"));

        assert!(parse_args(&["--batch".into()]).is_err()); // needs ≥1
        let o = parse_args(&["single".into()]).unwrap();
        assert!(o.batch.is_none());
        assert_eq!(o.description, "single");
    }
}
