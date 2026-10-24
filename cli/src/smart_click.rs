//! `smart-click` verb — click an element by accessible name and auto-record
//! the matching do/click step.
//!
//! The accessibility tree is read first. A role + accessible name that
//! matches more than one node fails without clicking or recording unless
//! the caller passes `--scope`. When the requested role misses and exactly
//! one interactive control has that name under a different role, that role
//! is recorded. Raw text is only used when no interactive role+name matches.
//!
//! CLI shape:
//!
//!   agent-qa smart-click "<accessible-name>"
//!                          [--role <role>]              default: 'button'
//!                          [--scope <role> <name>]      repeatable, outermost first
//!                          [--no-record]
//!                          [--session <name>]

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::browser::{self, RoleAct};
use crate::dom_activate::{self, ScopeStep, ScopedOutcome};
use crate::record_step::StepKind;
use crate::recorder_state::RecorderState;

const DEFAULT_ROLE: &str = "button";

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let mut state = RecorderState::load_active()?;
    let session = opts
        .session
        .clone()
        .unwrap_or_else(|| state.session.clone());

    let snapshot = match browser::snapshot_full(&session) {
        Ok(text) => Some(text),
        Err(e) => {
            eprintln!("[smart-click] accessibility snapshot unavailable: {e}");
            None
        }
    };
    let plan = plan_click(snapshot.as_deref(), &opts);
    match plan {
        Plan::Ambiguous { message } => bail!("{message}"),
        Plan::Unique(target) => {
            if target.discovered {
                eprintln!(
                    "[smart-click] role {} missed; using {} {:?}",
                    opts.role, target.role, target.name
                );
            }
            activate_target(&session, &target)?;
            record_role_click(&mut state, &opts, &session, &target)?;
            return Ok(0);
        }
        Plan::NoInteractive if !opts.scope.is_empty() => {
            bail!(
                "no interactive control named {:?} inside the given scope",
                opts.name
            );
        }
        Plan::Unseen if !opts.scope.is_empty() => {
            // The tree could not be read. Still honor the caller's scope so
            // an unscoped first-match cannot fire, and record that scope.
            let target = ResolvedTarget {
                role: opts.role.to_ascii_lowercase(),
                name: opts.name.clone(),
                node_ref: None,
                scope: normalize_scope(&opts.scope),
                discovered: false,
            };
            activate_target(&session, &target)?;
            record_role_click(&mut state, &opts, &session, &target)?;
            return Ok(0);
        }
        Plan::NoInteractive | Plan::Unseen => {}
    }

    // No interactive role+name match. Text, snapshot-ref, and the resolve
    // plugin stay available here only.
    let click_outcome = if try_named_control_click(&session, &opts.role, &opts.name)? {
        Ok(())
    } else {
        browser::find_role_act_quiet(&session, &opts.role, &opts.name, RoleAct::Click, None)
    };
    enum Recovery {
        /// Role and accessible name worked first.
        None,
        /// Text or chunked-text fallback worked. Replay uses a raw text locator.
        Text(String),
        /// Snapshot-ref fallback worked. The direct draft keeps the role and
        /// accessible name because the snapshot confirmed both.
        Snapshot,
        /// A `resolve` plugin picked the element — the recorded step keeps
        /// the picked candidate's role+name, not the missed description.
        Resolved(crate::resolve::ResolvedPick),
    }
    let recovery: Recovery = match click_outcome {
        Ok(()) => Recovery::None,
        Err(role_err) => {
            // Ladder rung 2: text + chunked text.
            if let Some(matched) = try_text_fallback(&session, &opts.name) {
                eprintln!(
                    "[smart-click] role+name miss recovered via text {:?}",
                    matched
                );
                Recovery::Text(matched)
            } else if try_snapshot_fallback(&session, &opts.role, &opts.name).is_ok() {
                // Ladder rung 3: parse snapshot for `<role> "<name>" ref=eN`
                // and click @eN. Bypasses the role engine entirely.
                eprintln!(
                    "[smart-click] role+name miss recovered via snapshot ref ({} {:?})",
                    opts.role, opts.name
                );
                Recovery::Snapshot
            } else if let Some(pick) = crate::resolve::resolve_and_act(
                &session,
                &opts.name,
                Some(&opts.role),
                RoleAct::Click,
                None,
            ) {
                // Ladder rung 4: configured `resolve` plugin picks among the
                // page's interactive elements — authoring-time only; the
                // recorded step keeps the pick's durable role+name.
                let conf = pick
                    .confidence
                    .map(|c| format!(" (confidence {c:.2})"))
                    .unwrap_or_default();
                eprintln!(
                    "[smart-click] miss {:?} resolved to {} {:?} via plugin{}",
                    opts.name, pick.candidate.role, pick.candidate.name, conf
                );
                Recovery::Resolved(pick)
            } else {
                return Err(anyhow::Error::new(role_err)).with_context(|| {
                    format!(
                        "agent-browser find role {} click --name {} (text + snapshot + resolve fallbacks also missed)",
                        opts.role, opts.name
                    )
                });
            }
        }
    };

    if opts.record {
        let payload = match &recovery {
            Recovery::None | Recovery::Snapshot => json!({
                "intent": format!("smart-click {}", opts.name),
                "verb": "click",
                "on": { "role": opts.role, "name": opts.name },
            }),
            Recovery::Resolved(pick) => json!({
                "intent": format!("smart-click {} (resolved)", opts.name),
                "verb": "click",
                "on": {
                    "role": pick.candidate.role,
                    "name": pick.candidate.name,
                },
            }),
            Recovery::Text(matched) => json!({
                "intent": format!("smart-click {}", opts.name),
                "verb": "click",
                "on": {
                    "raw": { "kind": "text", "value": matched },
                    "reason": "role and accessible-name lookup missed during recording"
                },
            }),
        };
        match crate::record_step::record_draft(&mut state, StepKind::Do, &payload, &session)? {
            Some(row) => {
                let step_id = row.step_id;
                match &recovery {
                    Recovery::None => println!(
                        "clicked {} (step {step_id}) — role={} name={:?}",
                        step_id, opts.role, opts.name
                    ),
                    Recovery::Text(matched) => println!(
                        "clicked {} (step {step_id}) — text={:?} (role+name miss recovered via text)",
                        step_id, matched
                    ),
                    Recovery::Snapshot => println!(
                        "clicked {} (step {step_id}) — role={} name={:?} (recovered via snapshot ref)",
                        step_id, opts.role, opts.name
                    ),
                    Recovery::Resolved(pick) => println!(
                        "clicked {} (step {step_id}) — resolved {:?} to {} {:?}",
                        step_id, opts.name, pick.candidate.role, pick.candidate.name
                    ),
                }
            }
            None => println!("clicked {} — not recorded (recording paused)", opts.name),
        }
    } else {
        match &recovery {
            Recovery::None => println!("clicked role={} name={:?}", opts.role, opts.name),
            Recovery::Text(matched) => println!(
                "clicked text={:?} (role+name miss recovered via text)",
                matched
            ),
            Recovery::Snapshot => println!(
                "clicked role={} name={:?} (recovered via snapshot ref)",
                opts.role, opts.name
            ),
            Recovery::Resolved(pick) => println!(
                "clicked resolved {:?} → {} {:?}",
                opts.name, pick.candidate.role, pick.candidate.name
            ),
        }
    }
    Ok(0)
}

fn record_role_click(
    state: &mut RecorderState,
    opts: &Opts,
    session: &str,
    target: &ResolvedTarget,
) -> Result<()> {
    let payload = json!({
        "intent": format!("smart-click {}", target.name),
        "verb": "click",
        "on": locator_on(&target.role, &target.name, &target.scope),
    });
    if !opts.record {
        println!("clicked {}", click_summary(target));
        return Ok(());
    }
    match crate::record_step::record_draft(state, StepKind::Do, &payload, session)? {
        Some(row) => {
            let step_id = row.step_id;
            if target.discovered {
                println!(
                    "clicked {step_id} (step {step_id}) — {} (requested role {} missed)",
                    click_summary(target),
                    opts.role
                );
            } else {
                println!(
                    "clicked {step_id} (step {step_id}) — {}",
                    click_summary(target)
                );
            }
        }
        None => println!("clicked {} — not recorded (recording paused)", target.name),
    }
    Ok(())
}

fn click_summary(target: &ResolvedTarget) -> String {
    if target.scope.is_empty() {
        format!("role={} name={:?}", target.role, target.name)
    } else {
        let chain = target
            .scope
            .iter()
            .map(|s| format!("{} {:?}", s.role, s.name))
            .collect::<Vec<_>>()
            .join(" > ");
        format!(
            "role={} name={:?} scope=[{chain}]",
            target.role, target.name
        )
    }
}

fn locator_on(role: &str, name: &str, scope: &[ScopeSpec]) -> Value {
    let mut on = json!({ "role": role, "name": name });
    if !scope.is_empty() {
        on["scope"] = Value::Array(
            scope
                .iter()
                .map(|s| json!({ "role": s.role, "name": s.name }))
                .collect(),
        );
    }
    on
}

/// Click a target the accessibility tree already disambiguated.
/// A non-empty scope never falls through to an unscoped first match.
fn activate_target(session: &str, target: &ResolvedTarget) -> Result<()> {
    if !target.scope.is_empty() {
        let steps: Vec<ScopeStep> = target
            .scope
            .iter()
            .map(|s| ScopeStep::Role {
                role: s.role.clone(),
                name: s.name.clone(),
            })
            .collect();
        return match dom_activate::act_scoped(
            session,
            &target.role,
            &target.name,
            &steps,
            RoleAct::Click,
            None,
        )? {
            ScopedOutcome::Done => Ok(()),
            ScopedOutcome::Miss => bail!(
                "scoped click found no {} {:?} inside the scope chain",
                target.role,
                target.name
            ),
            ScopedOutcome::ScopeMiss(i) => {
                let label = target
                    .scope
                    .get(i)
                    .map(|s| format!("{} {:?}", s.role, s.name))
                    .unwrap_or_else(|| format!("#{i}"));
                bail!("scoped click: scope level {i} ({label}) matched no element")
            }
        };
    }
    if try_named_control_click(session, &target.role, &target.name)? {
        return Ok(());
    }
    if browser::find_role_act_quiet(session, &target.role, &target.name, RoleAct::Click, None)
        .is_ok()
    {
        return Ok(());
    }
    if let Some(node_ref) = &target.node_ref {
        if browser::click_ref(session, node_ref).is_ok() {
            eprintln!(
                "[smart-click] role+name click missed; recovered via snapshot ref ({} {:?})",
                target.role, target.name
            );
            return Ok(());
        }
    }
    bail!("could not click {} {:?}", target.role, target.name)
}

/// Try `agent-browser find text <name|chunk> click` when role+name misses.
/// Returns the text that actually matched (so the recorded step can use
/// it verbatim) or None if every attempt missed. Probes are quiet
/// (capture stderr) so a non-recovering miss doesn't pollute the user's
/// console with `✗ Element not found` lines for the speculative tries.
fn try_text_fallback(session: &str, name: &str) -> Option<String> {
    if browser::find_text_act_quiet(session, name, RoleAct::Click, None).is_ok() {
        return Some(name.to_string());
    }
    for chunk in name.split_whitespace() {
        if chunk.len() < 3 {
            continue;
        }
        if browser::find_text_act_quiet(session, chunk, RoleAct::Click, None).is_ok() {
            return Some(chunk.to_string());
        }
    }
    None
}

/// Named interactive controls can be present in the snapshot while
/// agent-browser's coordinate click lands without triggering app handlers
/// (overlay interception, or a mousedown-bound handler). Prefer native DOM
/// activation — role + name resolution, scrollIntoView, and the full
/// pointer/mouse event chain on the node — for a broad set of interactive
/// roles, not just button/link. Non-interactive roles return `Ok(false)` so
/// the caller falls back to agent-browser's role/text/ref ladder. See
/// [`crate::dom_activate`].
fn try_named_control_click(session: &str, role: &str, name: &str) -> Result<bool> {
    if !dom_activate::is_interactive_role(role) {
        return Ok(false);
    }
    dom_activate::activate_role_name(session, role, name)
}

/// Final fallback rung: take a fresh ARIA snapshot, find the line that
/// matches `<role> "<name>" [... ref=eN]`, and click `@eN`. This
/// bypasses agent-browser's role engine and visible-text matcher
/// entirely — if the snapshot promises the element exists with that
/// role+name, the backend-node-id click is guaranteed to land on it.
///
/// Returns Ok(()) on success, Err on snapshot failure / no match / click
/// failure. Used after `find_role_act` and `find_text_act` both miss.
fn try_snapshot_fallback(session: &str, role: &str, name: &str) -> Result<()> {
    let snap =
        browser::snapshot_full(session).with_context(|| "snapshot for ref-based fallback")?;
    let r = browser::find_ref_in_snapshot(&snap, role, name)
        .ok_or_else(|| anyhow!("snapshot has no `{role} \"{name}\" [ref=...]` line"))?;
    browser::click_ref(session, &r).with_context(|| format!("click @{r}"))?;
    Ok(())
}

fn print_help() {
    println!(
        "agent-qa smart-click - click an element by accessible name + auto-record\n\n\
Usage:\n  \
agent-qa smart-click \"<accessible-name>\"\n                       \
[--role <role>]              (default: button)\n                       \
[--scope <role> <name>]      (repeatable; outermost container first)\n                       \
[--no-record]\n                       \
[--session <name>]\n\n\
Reads the accessibility tree first. When role + name matches more than one\n\
node, the click fails and nothing is recorded — pass --scope for the ancestor\n\
that contains the control (recorded as on.scope; there is no nth). If the\n\
requested role misses and exactly one interactive control has that accessible\n\
name under another role (link, checkbox, menuitem, tab, …), that role is\n\
clicked and recorded. Several interactive roles fail as ambiguous. Raw text\n\
is only the fallback when no interactive role+name matches.\n\
It does not verify post-click app state; record a check when the outcome matters."
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScopeSpec {
    role: String,
    name: String,
}

#[derive(Debug, Clone)]
struct Opts {
    name: String,
    role: String,
    scope: Vec<ScopeSpec>,
    record: bool,
    session: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedTarget {
    role: String,
    name: String,
    node_ref: Option<String>,
    scope: Vec<ScopeSpec>,
    discovered: bool,
}

#[derive(Debug)]
enum Plan {
    /// Snapshot missing or unparseable — the historical ladder still applies.
    Unseen,
    /// The tree was readable and no interactive control has this name.
    NoInteractive,
    /// More than one match. Nothing may be clicked or recorded.
    Ambiguous {
        message: String,
    },
    Unique(ResolvedTarget),
}

fn plan_click(snapshot: Option<&str>, opts: &Opts) -> Plan {
    let Some(snapshot) = snapshot else {
        return Plan::Unseen;
    };
    let (node_count, hits) = snapshot_hits(snapshot);
    if node_count == 0 {
        return Plan::Unseen;
    }
    let scope = normalize_scope(&opts.scope);
    let direct = matching(&hits, &opts.role, &opts.name, &scope);
    if direct.len() > 1 {
        return Plan::Ambiguous {
            message: ambiguous_message(&opts.name, &direct, !scope.is_empty()),
        };
    }
    if let Some(hit) = direct.first() {
        return Plan::Unique(ResolvedTarget {
            role: hit.role.to_ascii_lowercase(),
            name: opts.name.clone(),
            node_ref: hit.node_ref.clone(),
            scope,
            discovered: false,
        });
    }
    let others = interactive_matching(&hits, &opts.name, &scope);
    if others.len() > 1 {
        return Plan::Ambiguous {
            message: ambiguous_message(&opts.name, &others, !scope.is_empty()),
        };
    }
    if let Some(hit) = others.first() {
        return Plan::Unique(ResolvedTarget {
            role: hit.role.to_ascii_lowercase(),
            name: opts.name.clone(),
            node_ref: hit.node_ref.clone(),
            scope,
            discovered: true,
        });
    }
    Plan::NoInteractive
}

fn ambiguous_message(name: &str, hits: &[&Hit], scoped: bool) -> String {
    let roles: Vec<String> = hits.iter().map(|h| h.role.to_ascii_lowercase()).collect();
    let same = roles.iter().all(|r| r == &roles[0]);
    let list = if same {
        format!("{} {} matches", hits.len(), roles[0])
    } else {
        hits.iter()
            .map(|h| format!("{} {:?}", h.role.to_ascii_lowercase(), h.name))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let where_ = if scoped {
        " inside the given scope"
    } else {
        ""
    };
    format!(
        "accessible name {name:?} is ambiguous ({list}){where_}; a scope is required (--scope <role> <name>, repeatable, outermost first)"
    )
}

fn normalize_scope(scope: &[ScopeSpec]) -> Vec<ScopeSpec> {
    scope
        .iter()
        .map(|s| ScopeSpec {
            role: s.role.to_ascii_lowercase(),
            name: s.name.clone(),
        })
        .collect()
}

struct SnapNode {
    depth: usize,
    role: String,
    name: Option<String>,
    node_ref: Option<String>,
}

struct Hit {
    role: String,
    name: String,
    node_ref: Option<String>,
    /// Named ancestors, outermost first.
    ancestors: Vec<(String, String)>,
}

/// `(parsed node count, named hits)`. A count of 0 means the snapshot text
/// had no accessibility-tree lines, so callers must not treat it as "the
/// page has no such control".
fn snapshot_hits(snapshot: &str) -> (usize, Vec<Hit>) {
    let nodes = parse_snapshot_nodes(snapshot);
    let count = nodes.len();
    let mut stack: Vec<SnapNode> = Vec::new();
    let mut hits = Vec::new();
    for node in nodes {
        while stack.last().is_some_and(|top| top.depth >= node.depth) {
            stack.pop();
        }
        if let Some(name) = node.name.clone() {
            let ancestors = stack
                .iter()
                .filter_map(|anc| anc.name.clone().map(|n| (anc.role.clone(), n)))
                .collect();
            hits.push(Hit {
                role: node.role.clone(),
                name,
                node_ref: node.node_ref.clone(),
                ancestors,
            });
        }
        stack.push(node);
    }
    (count, hits)
}

fn parse_snapshot_nodes(snapshot: &str) -> Vec<SnapNode> {
    let mut out = Vec::new();
    for raw in snapshot.lines() {
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        let depth = indent / 2;
        let body = raw.trim_start();
        let body = match body.strip_prefix("- ") {
            Some(rest) => rest,
            None => match body.strip_prefix('-') {
                Some(rest) => rest.trim_start(),
                None => continue,
            },
        };
        if body.is_empty() {
            continue;
        }
        let role_end = body.find([' ', '"', '[']).unwrap_or(body.len());
        let role = body[..role_end].trim();
        if role.is_empty() {
            continue;
        }
        let rest = &body[role_end..];
        out.push(SnapNode {
            depth,
            role: role.to_string(),
            name: extract_quoted_name(rest),
            node_ref: extract_ref(rest),
        });
    }
    out
}

fn extract_quoted_name(rest: &str) -> Option<String> {
    let head = match rest.find('[') {
        Some(i) => &rest[..i],
        None => rest,
    };
    let start = head.find('"')?;
    let after = &head[start + 1..];
    let end = after.find('"')?;
    let name = after[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn extract_ref(rest: &str) -> Option<String> {
    let idx = rest.find("ref=")? + 4;
    let tail = &rest[idx..];
    let end = tail
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(tail.len());
    if end == 0 {
        None
    } else {
        Some(tail[..end].to_string())
    }
}

fn matching<'a>(hits: &'a [Hit], role: &str, name: &str, scope: &[ScopeSpec]) -> Vec<&'a Hit> {
    hits.iter()
        .filter(|h| role_eq(&h.role, role) && h.name == name && in_scope(&h.ancestors, scope))
        .collect()
}

fn interactive_matching<'a>(hits: &'a [Hit], name: &str, scope: &[ScopeSpec]) -> Vec<&'a Hit> {
    hits.iter()
        .filter(|h| {
            dom_activate::is_interactive_role(&h.role.to_ascii_lowercase())
                && h.name == name
                && in_scope(&h.ancestors, scope)
        })
        .collect()
}

fn in_scope(ancestors: &[(String, String)], scope: &[ScopeSpec]) -> bool {
    if scope.is_empty() {
        return true;
    }
    let mut idx = 0;
    for (role, name) in ancestors {
        if idx >= scope.len() {
            break;
        }
        if role_eq(role, &scope[idx].role) && name == &scope[idx].name {
            idx += 1;
        }
    }
    idx == scope.len()
}

fn role_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut name: Option<String> = None;
    let mut role: Option<String> = None;
    let mut scope: Vec<ScopeSpec> = Vec::new();
    let mut record = true;
    let mut session: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--role" => role = it.next().cloned(),
            s if s.starts_with("--role=") => role = Some(s["--role=".len()..].to_string()),
            "--scope" => {
                let role = it.next().cloned().ok_or_else(|| {
                    anyhow!("--scope requires <role> <name> (outermost container first)")
                })?;
                let scope_name = it.next().cloned().ok_or_else(|| {
                    anyhow!("--scope requires <role> <name> (outermost container first)")
                })?;
                push_scope(&mut scope, role, scope_name)?;
            }
            "--no-record" => record = false,
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if name.is_some() {
                    bail!("unexpected positional {other:?}; usage: smart-click \"<name>\"");
                }
                name = Some(other.to_string());
            }
        }
    }
    let name = name.ok_or_else(|| anyhow!("usage: smart-click \"<accessible-name>\""))?;
    let role = role.unwrap_or_else(|| DEFAULT_ROLE.to_string());
    Ok(Opts {
        name,
        role,
        scope,
        record,
        session,
    })
}

fn push_scope(scope: &mut Vec<ScopeSpec>, role: String, name: String) -> Result<()> {
    if role.is_empty() || name.is_empty() || role.starts_with('-') || name.starts_with('-') {
        bail!("--scope requires <role> <name> (outermost container first)");
    }
    scope.push(ScopeSpec { role, name });
    Ok(())
}

#[cfg(test)]
mod plan_tests {
    use super::*;

    fn opts(name: &str, role: &str, scope: &[(&str, &str)]) -> Opts {
        Opts {
            name: name.into(),
            role: role.into(),
            scope: scope
                .iter()
                .map(|(r, n)| ScopeSpec {
                    role: (*r).into(),
                    name: (*n).into(),
                })
                .collect(),
            record: true,
            session: None,
        }
    }

    const CARDS: &str = "\
- main
  - heading \"People\" [level=1, ref=e1]
  - region \"Ada\" [ref=e2]
    - heading \"Ada\" [level=2, ref=e3]
    - button \"Edit\" [ref=e4]
  - region \"Bea\" [ref=e5]
    - heading \"Bea\" [level=2, ref=e6]
    - button \"Edit\" [ref=e7]
- link \"Add user\" [ref=e8]
";

    #[test]
    fn two_same_name_buttons_are_ambiguous() {
        let plan = plan_click(Some(CARDS), &opts("Edit", "button", &[]));
        match plan {
            Plan::Ambiguous { message } => {
                assert!(message.contains("ambiguous"), "{message}");
                assert!(message.contains("scope is required"), "{message}");
                assert!(message.contains("2 button matches"), "{message}");
            }
            other => panic!("expected ambiguous, got {other:?}"),
        }
        assert_eq!(
            browser::find_lines_in_snapshot(CARDS, "button", "Edit").len(),
            2
        );
    }

    #[test]
    fn scope_selects_the_ancestor_region() {
        let plan = plan_click(Some(CARDS), &opts("Edit", "button", &[("region", "Bea")]));
        match plan {
            Plan::Unique(t) => {
                assert_eq!(t.role, "button");
                assert_eq!(t.name, "Edit");
                assert_eq!(t.node_ref.as_deref(), Some("e7"));
                assert!(!t.discovered);
                assert_eq!(
                    t.scope,
                    vec![ScopeSpec {
                        role: "region".into(),
                        name: "Bea".into()
                    }]
                );
            }
            other => panic!("expected unique scoped target, got {other:?}"),
        }
    }

    #[test]
    fn heading_sibling_is_not_a_scope_ancestor() {
        let plan = plan_click(Some(CARDS), &opts("Edit", "button", &[("heading", "Bea")]));
        match plan {
            Plan::NoInteractive => {}
            other => panic!("heading is not an ancestor of Edit, got {other:?}"),
        }
    }

    #[test]
    fn repeated_scope_chain_narrows_inside_the_region() {
        let snap = "\
- region \"Bea\" [ref=e1]
  - group \"Actions\" [ref=e2]
    - button \"Edit\" [ref=e3]
  - group \"Meta\" [ref=e4]
    - button \"Edit\" [ref=e5]
";
        let ambiguous = plan_click(Some(snap), &opts("Edit", "button", &[("region", "Bea")]));
        assert!(matches!(ambiguous, Plan::Ambiguous { .. }), "{ambiguous:?}");
        let plan = plan_click(
            Some(snap),
            &opts("Edit", "button", &[("region", "Bea"), ("group", "Actions")]),
        );
        match plan {
            Plan::Unique(t) => {
                assert_eq!(t.node_ref.as_deref(), Some("e3"));
                assert_eq!(t.scope.len(), 2);
                assert_eq!(t.scope[1].name, "Actions");
            }
            other => panic!("expected the Actions button, got {other:?}"),
        }
    }

    #[test]
    fn default_role_miss_discovers_a_single_link() {
        let plan = plan_click(Some(CARDS), &opts("Add user", "button", &[]));
        match plan {
            Plan::Unique(t) => {
                assert!(t.discovered);
                assert_eq!(t.role, "link");
                assert!(t.scope.is_empty());
                assert_eq!(t.node_ref.as_deref(), Some("e8"));
            }
            other => panic!("expected discovered link, got {other:?}"),
        }
    }

    #[test]
    fn several_interactive_roles_are_ambiguous() {
        let snap = "\
- link \"Save\" [ref=e1]
- menuitem \"Save\" [ref=e2]
";
        let plan = plan_click(Some(snap), &opts("Save", "button", &[]));
        match plan {
            Plan::Ambiguous { message } => {
                assert!(message.contains("ambiguous"), "{message}");
                assert!(message.contains("scope is required"), "{message}");
                assert!(message.contains("link"), "{message}");
                assert!(message.contains("menuitem"), "{message}");
            }
            other => panic!("expected ambiguous roles, got {other:?}"),
        }
    }

    #[test]
    fn unique_button_has_no_scope() {
        let snap = "- button \"Show result\" [ref=e9]\n";
        let plan = plan_click(Some(snap), &opts("Show result", "button", &[]));
        match plan {
            Plan::Unique(t) => {
                assert_eq!(t.role, "button");
                assert!(t.scope.is_empty());
                assert!(!t.discovered);
            }
            other => panic!("expected unique button, got {other:?}"),
        }
    }

    #[test]
    fn checkbox_is_discovered_when_button_misses() {
        let snap = "- checkbox \"Send invite\" [ref=e3]\n";
        let plan = plan_click(Some(snap), &opts("Send invite", "button", &[]));
        match plan {
            Plan::Unique(t) => {
                assert!(t.discovered);
                assert_eq!(t.role, "checkbox");
            }
            other => panic!("expected checkbox, got {other:?}"),
        }
    }

    #[test]
    fn empty_snapshot_stays_unseen() {
        let plan = plan_click(Some(""), &opts("Save", "button", &[]));
        assert!(matches!(plan, Plan::Unseen));
        let plan = plan_click(None, &opts("Save", "button", &[]));
        assert!(matches!(plan, Plan::Unseen));
    }

    #[test]
    fn static_text_under_a_link_does_not_make_the_name_ambiguous() {
        let snap = "\
- link \"Add user\" [ref=e1]
  - StaticText \"Add user\"
";
        let plan = plan_click(Some(snap), &opts("Add user", "button", &[]));
        match plan {
            Plan::Unique(t) => assert_eq!(t.role, "link"),
            other => panic!("expected the link only, got {other:?}"),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser::{self, BrowserConnection};
    use crate::paths;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn click_records_a_direct_step() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        let binary = tmp.path().join("agent-browser");
        fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, &binary);
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        browser::_reset_bin_cache_for_tests();
        RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        )
        .save()
        .unwrap();
        run(&["Save".into()]).unwrap();
        let state = RecorderState::load_active().unwrap();
        let step = serde_json::to_value(&state.steps[0]).unwrap();
        assert_eq!(step["kind"], "do");
        assert_eq!(step["verb"], "click");
        assert_eq!(step["on"]["role"], "button");
        assert!(step["on"].get("scope").is_none());
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    struct BrowserEnv {
        _tmp: TempDir,
        log: PathBuf,
    }

    impl BrowserEnv {
        fn new(snapshot: &str) -> Self {
            let tmp = TempDir::new().unwrap();
            let log = tmp.path().join("browser.log");
            let snap = tmp.path().join("snap.txt");
            fs::write(&snap, snapshot).unwrap();
            let binary = tmp.path().join("agent-browser");
            let script = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$3\" = snapshot ]; then\n  cat '{}'\n  exit 0\nfi\nif [ \"$3\" = eval ]; then\n  printf '\"true\"'\n  exit 0\nfi\nexit 0\n",
                log.display(),
                snap.display()
            );
            write_exec(&binary, &script);
            std::env::set_var(browser::BIN_ENV, &binary);
            std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
            std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
            std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
            browser::_reset_bin_cache_for_tests();
            RecorderState::new(
                "s1".into(),
                "record".into(),
                "sess".into(),
                RecorderBaseline::Fresh,
                None,
                BrowserConnection::default(),
            )
            .save()
            .unwrap();
            Self { _tmp: tmp, log }
        }

        fn log(&self) -> String {
            fs::read_to_string(&self.log).unwrap_or_default()
        }
    }

    impl Drop for BrowserEnv {
        fn drop(&mut self) {
            std::env::remove_var(paths::SCENARIOS_DIR_ENV);
            std::env::remove_var(paths::RECORD_DIR_ENV);
            std::env::remove_var(browser::BIN_ENV);
            std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
            browser::_reset_bin_cache_for_tests();
        }
    }

    fn write_exec(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn recorded_step() -> Value {
        let state = RecorderState::load_active().unwrap();
        assert_eq!(state.steps.len(), 1, "expected one recorded step");
        serde_json::to_value(&state.steps[0]).unwrap()
    }

    const CARDS: &str = "\
- main
  - region \"Ada\" [ref=e2]
    - button \"Edit\" [ref=e4]
  - region \"Bea\" [ref=e5]
    - button \"Edit\" [ref=e7]
- link \"Add user\" [ref=e8]
";

    #[test]
    fn ambiguous_edit_does_not_click_or_record() {
        let _guard = lock_env();
        let env = BrowserEnv::new(CARDS);
        let err = run(&["Edit".into()]).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("ambiguous"), "{message}");
        assert!(message.contains("scope is required"), "{message}");
        let state = RecorderState::load_active().unwrap();
        assert!(state.steps.is_empty(), "ambiguous click must not record");
        let log = env.log();
        assert!(log.contains("snapshot"), "{log}");
        assert!(!log.contains(" eval "), "{log}");
        assert!(!log.contains(" find "), "{log}");
        assert!(!log.contains(" click "), "{log}");
    }

    #[test]
    fn scoped_edit_records_the_region_and_activates_inside_it() {
        let _guard = lock_env();
        let env = BrowserEnv::new(CARDS);
        run(&[
            "Edit".into(),
            "--scope".into(),
            "region".into(),
            "Bea".into(),
        ])
        .unwrap();
        let step = recorded_step();
        assert_eq!(step["on"]["role"], "button");
        assert_eq!(step["on"]["name"], "Edit");
        assert!(step["on"].get("nth").is_none());
        assert_eq!(step["on"]["scope"][0]["role"], "region");
        assert_eq!(step["on"]["scope"][0]["name"], "Bea");
        let log = env.log();
        assert!(log.contains("__aqScopedFind"), "{log}");
        assert!(log.contains("Bea"), "{log}");
        assert!(
            !log.contains(" find role "),
            "scoped click must not use an unscoped role finder: {log}"
        );
    }

    #[test]
    fn add_user_records_the_link_role_not_raw_text() {
        let _guard = lock_env();
        let env = BrowserEnv::new(CARDS);
        run(&["Add user".into()]).unwrap();
        let step = recorded_step();
        assert_eq!(step["on"]["role"], "link");
        assert_eq!(step["on"]["name"], "Add user");
        assert!(step["on"].get("raw").is_none(), "{step}");
        assert!(step["on"].get("scope").is_none(), "{step}");
        let log = env.log();
        assert!(
            !log.contains(" find text "),
            "a unique link must not degrade to text: {log}"
        );
    }

    #[test]
    fn several_roles_do_not_degrade_to_text() {
        let _guard = lock_env();
        let env = BrowserEnv::new("- link \"Save\" [ref=e1]\n- menuitem \"Save\" [ref=e2]\n");
        let err = run(&["Save".into()]).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("ambiguous"), "{message}");
        assert!(message.contains("scope is required"), "{message}");
        assert!(RecorderState::load_active().unwrap().steps.is_empty());
        let log = env.log();
        assert!(!log.contains(" find text "), "{log}");
        assert!(!log.contains(" eval "), "{log}");
    }

    #[test]
    fn unique_button_records_role_and_name_without_scope() {
        let _guard = lock_env();
        let _env = BrowserEnv::new("- button \"Show result\" [ref=e9]\n");
        run(&["Show result".into()]).unwrap();
        let step = recorded_step();
        assert_eq!(step["on"]["role"], "button");
        assert_eq!(step["on"]["name"], "Show result");
        assert!(step["on"].get("scope").is_none(), "{step}");
        assert!(step["on"].get("raw").is_none(), "{step}");
    }

    #[test]
    fn scope_flag_is_repeatable_and_outermost_first() {
        let _guard = lock_env();
        let _env = BrowserEnv::new(
            "\
- region \"Bea\" [ref=e1]
  - group \"Actions\" [ref=e2]
    - button \"Edit\" [ref=e3]
",
        );
        run(&[
            "--scope".into(),
            "region".into(),
            "Bea".into(),
            "Edit".into(),
            "--scope".into(),
            "group".into(),
            "Actions".into(),
        ])
        .unwrap();
        let step = recorded_step();
        assert_eq!(step["on"]["scope"][0]["role"], "region");
        assert_eq!(step["on"]["scope"][0]["name"], "Bea");
        assert_eq!(step["on"]["scope"][1]["role"], "group");
        assert_eq!(step["on"]["scope"][1]["name"], "Actions");
    }

    #[test]
    fn scope_requires_role_and_name() {
        let err = parse_args(&["Edit".into(), "--scope".into(), "region".into()]).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("--scope"), "{message}");
    }
}
