//! Inline replay auto-heal.
//!
//! A do-step whose role+name locator misses at replay time usually means
//! accessible-name drift ("Save" → "Save changes", a volatile count, a
//! generated suffix). Instead of failing the step, the runner collects the
//! page's live role candidates and walks an ordered **strategy ladder** —
//! strict to permissive. A strategy only fires when EXACTLY ONE live
//! candidate matches: it refuses to guess when ambiguous.
//!
//! On a match the step retries ONCE with the corrected locator. A
//! matched-but-still-failing retry is a hard failure — the run aborts as
//! usual. Each successful heal is persisted two ways:
//!
//!   * a `heal-row/v1` line on `replays/<runId>/heal.jsonl` with mode
//!     `locator-correction` — the run-scoped heal audit trail
//!     (`scenario-sidecar-tree.md` reserves it for exactly this);
//!   * `replays/<runId>/diffs/<stepId>.patch.json` — the `heal-patch/v1`
//!     producer `heal-promote` was built to consume.
//!
//! A failure no strategy can heal is probed for a value rejection (visible
//! alert/toast/banner) and classified in the audit trail — never retried.
//!
//! Two env gates:
//!   AGENT_QA_NO_HEAL      — disable auto-heal entirely (CI runs that must
//!                           fail hard on any drift).
//!   AGENT_QA_HEAL_STRICT  — a run that needed any heal exits non-zero even
//!                           though every step passed, so drift surfaces for
//!                           review instead of silently self-correcting.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde_json::json;

use crate::browser;
use crate::scenario::{Locator, LocatorRole, NameMatch, Step};
use crate::sidecar::{atomic_write_file, RunPaths};

/// `AGENT_QA_NO_HEAL` set → auto-heal disabled entirely.
pub fn enabled() -> bool {
    std::env::var_os("AGENT_QA_NO_HEAL").is_none()
}

/// `AGENT_QA_HEAL_STRICT` set → a healed run still exits non-zero.
pub fn strict() -> bool {
    std::env::var_os("AGENT_QA_HEAL_STRICT").is_some()
}

/// One accepted correction: which strategy matched, the recorded name, the
/// live name, and the full corrected locator (role + scope preserved).
#[derive(Debug)]
pub struct Heal {
    pub strategy: &'static str,
    pub from: String,
    pub to: String,
    pub locator: Locator,
}

/// Extract (role locator, recorded name) from a do-step when it carries a
/// healable locator — a `Locator::Role` with a literal/pattern name. Raw
/// locators, nameless role locators, and i18nKey refs don't participate.
fn role_name_of(step: &Step) -> Option<(LocatorRole, String)> {
    let Step::Do {
        on: Some(Locator::Role(loc)),
        ..
    } = step
    else {
        return None;
    };
    let want = match &loc.name {
        Some(NameMatch::Plain(s)) => s.clone(),
        Some(NameMatch::Pattern { pattern, .. }) => pattern.clone(),
        Some(NameMatch::I18n { .. }) | None => return None,
    };
    if want.trim().is_empty() {
        return None;
    }
    Some((loc.clone(), want))
}

/// Collect live role+name candidates and run the ladder. `Ok(None)` when the
/// step has no healable locator or no strategy produced a unique match.
pub fn attempt(step: &Step, session: &str) -> Result<Option<Heal>> {
    let Some((loc, want)) = role_name_of(step) else {
        return Ok(None);
    };
    let candidates =
        collect_role_names(session, &loc.role).context("collect live role+name candidates")?;
    let Some((strategy, to)) = ladder(&want, &candidates) else {
        return Ok(None);
    };
    let mut corrected = loc.clone();
    corrected.name = Some(NameMatch::Plain(to.clone()));
    Ok(Some(Heal {
        strategy,
        from: want,
        to,
        locator: Locator::Role(corrected),
    }))
}

/// The step the runner re-dispatches after a heal — the recorded step with
/// `on` swapped for the corrected locator.
pub fn apply_correction(step: &Step, heal: &Heal) -> Step {
    let mut s = step.clone();
    if let Step::Do { on, .. } = &mut s {
        *on = Some(heal.locator.clone());
    }
    s
}

/// Live alert/banner/toast texts on the page — the evidence a failed step was
/// a value rejection (the app refused the submitted input), surfaced in the
/// audit trail. Empty on probe failure (a dead browser never fabricates one).
pub fn rejection_evidence(session: &str) -> Vec<String> {
    let out = match browser::eval_expression(session, &crate::dom_activate::build_rejection_probe())
    {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let t = out.trim().trim_matches('"');
    serde_json::from_str::<Vec<String>>(t).unwrap_or_default()
}

/// Persist a successful heal: the patch file `heal-promote` applies, plus a
/// `locator-correction` row on the shared heal audit trail.
pub fn persist_correction(
    run: &RunPaths,
    scenario_hash: &str,
    step_id: &str,
    heal: &Heal,
) -> Result<()> {
    let diffs = run.run_root.join("diffs");
    fs::create_dir_all(&diffs).with_context(|| format!("mkdir {}", diffs.display()))?;
    let file = diffs.join(format!("{step_id}.patch.json"));
    let body = json!({
        "schema": "heal-patch/v1",
        "stepId": step_id,
        "scenarioContentHash": scenario_hash,
        "newLocator": serde_json::to_value(&heal.locator)?,
        "rationale": format!(
            "auto-heal via {}: {:?} → {:?}",
            heal.strategy, heal.from, heal.to
        ),
    });
    let mut bytes = serde_json::to_string_pretty(&body)?.into_bytes();
    bytes.push(b'\n');
    atomic_write_file(&file, &bytes)?;
    append_row(
        &run.run_root,
        &json!({
            "schema": "heal-row/v1",
            "ts": now_ts(),
            "runId": run.run_id,
            "stepId": step_id,
            "mode": "locator-correction",
            "strategy": heal.strategy,
            "from": heal.from,
            "to": heal.to,
        }),
    )
}

/// Record a classified value rejection on the audit trail — surfaced, never
/// retried (per the replay-robustness spec: no auto-retry with a different
/// value).
pub fn persist_rejection(run: &RunPaths, step_id: &str, evidence: &[String]) -> Result<()> {
    append_row(
        &run.run_root,
        &json!({
            "schema": "heal-row/v1",
            "ts": now_ts(),
            "runId": run.run_id,
            "stepId": step_id,
            "mode": "value-rejection",
            "rationale": evidence.join(" | "),
        }),
    )
}

// ---------- live candidate collection ----------

fn collect_role_names(session: &str, role: &str) -> Result<Vec<String>> {
    let out = browser::eval_expression(
        session,
        &crate::dom_activate::build_collect_role_names(role),
    )
    .map_err(|e| anyhow!("eval collect: {e}"))?;
    Ok(parse_names(&out))
}

fn parse_names(out: &str) -> Vec<String> {
    let t = out.trim().trim_matches('"');
    serde_json::from_str::<serde_json::Value>(t)
        .ok()
        .and_then(|v| v.get("names").and_then(|n| n.as_array()).cloned())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ---------- strategy ladder (pure) ----------

/// Collapse whitespace runs and lowercase — the same normalisation the
/// in-page matcher applies (`__aqText` + whitespace collapse).
fn norm(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Collapse each run of digits to '#', mirroring the in-page `__aqND`
/// digit-tolerant matcher.
fn digit_norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_digits = false;
    for c in norm(s).chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('#');
                in_digits = true;
            }
        } else {
            out.push(c);
            in_digits = false;
        }
    }
    out
}

/// Remove all digits entirely — tolerates a digit token added or dropped
/// between record and replay ("Save 12" ↔ "Save"), not just renumbered.
/// Whitespace is re-collapsed after removal so "save 12" → "save".
fn strip_digits(s: &str) -> String {
    let no_digits: String = s.chars().filter(|c| !c.is_ascii_digit()).collect();
    norm(&no_digits)
}

/// Number of digit runs in the string ("Item 5 of 6" → 2). Digit-anywhere
/// only fires when the two sides differ in run count — when counts match,
/// the digits were renumbered, which is digits-tolerant's job (and if that
/// stricter strategy already rejected, this drift is real, not cosmetic).
fn digit_run_count(s: &str) -> usize {
    let mut n = 0;
    let mut in_digits = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                n += 1;
                in_digits = true;
            }
        } else {
            in_digits = false;
        }
    }
    n
}

/// Drop a trailing whitespace-separated token that looks machine-generated
/// — a fragment with a digit in it ("Session 3xKq2", "Order 98213"). Applied
/// to both sides: the suffix may exist on the recording or only appear live.
fn strip_generated_suffix(s: &str) -> String {
    let n = norm(s);
    match n.rsplit_once(' ') {
        Some((head, tail)) if looks_generated(tail) && !head.is_empty() => head.to_string(),
        _ => n,
    }
}

fn looks_generated(tok: &str) -> bool {
    tok.len() >= 4 && tok.chars().any(|c| c.is_ascii_digit())
}

fn prefix_match(want_n: &str, cand_n: &str) -> bool {
    let (a, b) = if want_n.len() <= cand_n.len() {
        (want_n, cand_n)
    } else {
        (cand_n, want_n)
    };
    a.len() >= 4 && b.starts_with(a)
}

/// Exactly-one-match helper: a strategy returns its hit only when a single
/// DISTINCT live candidate matches — ambiguity refuses rather than guesses.
fn unique_match<F: Fn(&str) -> bool>(candidates: &[String], pred: F) -> Option<String> {
    let hits: BTreeSet<&String> = candidates.iter().filter(|c| pred(c)).collect();
    if hits.len() == 1 {
        Some(hits.into_iter().next().expect("one hit").clone())
    } else {
        None
    }
}

type Strategy = (&'static str, fn(&str, &str) -> bool);

const LADDER: &[Strategy] = &[
    // Same text modulo whitespace/case — but never an identical name (the
    // verbatim lookup already failed).
    ("whitespace", |w, c| w != c && norm(w) == norm(c)),
    // Volatile counts/ids renumbered between record and replay
    // ("Optimize 9986 licenses" → "…9985…"). Requires a real non-digit core.
    ("digits-tolerant", |w, c| {
        let wn = digit_norm(w);
        w != c && wn.replace('#', "").trim().len() >= 3 && wn == digit_norm(c)
    }),
    // Digit tokens added/dropped, not just renumbered — equal run counts
    // mean renumbering, which the stricter strategy above owns.
    ("digits-anywhere", |w, c| {
        let wn = strip_digits(w);
        wn.len() >= 3 && digit_run_count(w) != digit_run_count(c) && wn == strip_digits(c)
    }),
    // App-generated trailing token ("Save draft 3xKq2").
    ("generated-suffix", |w, c| {
        !strip_generated_suffix(w).is_empty()
            && strip_generated_suffix(w) == strip_generated_suffix(c)
    }),
    // "Save" ↔ "Save changes" — shortest side must still carry signal.
    ("name-prefix", |w, c| prefix_match(&norm(w), &norm(c))),
];

/// Walk the ladder strict → permissive; first unique match wins.
/// Returns (strategy name, corrected live name).
///
/// If the recorded name still resolves verbatim among live candidates the
/// miss isn't name drift (overlay, detach, timing) — no correction applies
/// and no looser strategy may pick a lookalike in its place.
pub fn ladder(want: &str, candidates: &[String]) -> Option<(&'static str, String)> {
    if candidates.iter().any(|c| c == want) {
        return None;
    }
    for (name, pred) in LADDER {
        if let Some(hit) = unique_match(candidates, |c| pred(want, c)) {
            return Some((*name, hit));
        }
    }
    None
}

// ---------- persistence helpers ----------

/// Append a heal-row/v1 line to `<run>/heal.jsonl` — the run-scoped heal
/// audit trail the sidecar spec reserves for replay-time heal events.
fn append_row(run_dir: &Path, row: &serde_json::Value) -> Result<()> {
    fs::create_dir_all(run_dir).ok();
    let path = run_dir.join("heal.jsonl");
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    f.write_all(serde_json::to_string(row)?.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

fn now_ts() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::LocatorRawSpec;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn whitespace_matches_collapsed() {
        let got = ladder("Line   break", &names(&["line break"]));
        assert_eq!(got, Some(("whitespace", "line break".into())));
    }

    #[test]
    fn verbatim_name_does_not_heal() {
        // The recorded name resolving verbatim means dispatch succeeded —
        // nothing to heal.
        assert_eq!(ladder("Save", &names(&["Save"])), None);
    }

    #[test]
    fn digits_tolerant_renumbers() {
        let got = ladder(
            "Optimize 9986 licenses available",
            &names(&["Optimize 9985 licenses available"]),
        );
        assert_eq!(
            got,
            Some(("digits-tolerant", "Optimize 9985 licenses available".into()))
        );
    }

    #[test]
    fn digits_anywhere_tolerates_added_token() {
        let got = ladder("Save", &names(&["Save 12"]));
        assert_eq!(got, Some(("digits-anywhere", "Save 12".into())));
    }

    #[test]
    fn generated_suffix_strips_hashy_tail() {
        let got = ladder("Open session", &names(&["Open session 3xKq2"]));
        assert_eq!(got, Some(("generated-suffix", "Open session 3xKq2".into())));
    }

    #[test]
    fn word_tail_is_not_generated() {
        // "items" carries no digit — not a generated suffix; prefix still
        // rescues it further down the ladder.
        let got = ladder("Save", &names(&["Save items"]));
        assert_eq!(got, Some(("name-prefix", "Save items".into())));
    }

    #[test]
    fn prefix_requires_signal() {
        assert_eq!(ladder("Sav", &names(&["Save everything"])), None);
    }

    #[test]
    fn ambiguity_refuses_to_guess() {
        // Two live candidates both satisfy digits-tolerant — refuse rather
        // than pick one arbitrarily.
        let cands = names(&["Optimize 1 items", "Optimize 2 items"]);
        assert_eq!(ladder("Optimize 3 items", &cands), None);
    }

    #[test]
    fn strict_strategy_wins_over_permissive() {
        // Whitespace-equal beats a would-be prefix match.
        let got = ladder("Line\nbreak", &names(&["line break", "line break plus"]));
        assert_eq!(got, Some(("whitespace", "line break".into())));
    }

    #[test]
    fn ladder_returns_none_with_no_candidates() {
        assert_eq!(ladder("Save", &[]), None);
    }

    fn do_step(on: Option<Locator>) -> Step {
        Step::Do {
            id: "s1".into(),
            intent: "click".into(),
            verb: crate::scenario::Verb::Click,
            on,
            value: None,
            save_as: None,
            params: None,
            context: None,
        }
    }

    #[test]
    fn role_name_of_extracts_plain_and_pattern() {
        let step = do_step(Some(Locator::Role(LocatorRole {
            role: "button".into(),
            name: Some(NameMatch::Plain("Save".into())),
            scope: None,
            tolerate: None,
        })));
        assert!(role_name_of(&step).is_some());

        // Raw locators never heal.
        let raw = do_step(Some(Locator::Raw(crate::scenario::LocatorRaw {
            raw: LocatorRawSpec {
                kind: crate::scenario::RawLocatorKind::Css,
                value: "#save".into(),
            },
            reason: "test".into(),
        })));
        assert!(role_name_of(&raw).is_none());

        // No locator at all.
        assert!(role_name_of(&do_step(None)).is_none());
    }

    #[test]
    fn apply_correction_swaps_locator_name() {
        let step = do_step(Some(Locator::Role(LocatorRole {
            role: "button".into(),
            name: Some(NameMatch::Plain("Save".into())),
            scope: None,
            tolerate: None,
        })));
        let heal = Heal {
            strategy: "test",
            from: "Save".into(),
            to: "Save changes".into(),
            locator: Locator::Role(LocatorRole {
                role: "button".into(),
                name: Some(NameMatch::Plain("Save changes".into())),
                scope: None,
                tolerate: None,
            }),
        };
        let fixed = apply_correction(&step, &heal);
        let Step::Do {
            on: Some(Locator::Role(l)),
            ..
        } = fixed
        else {
            panic!("expected role locator")
        };
        assert!(matches!(
            l.name,
            Some(NameMatch::Plain(ref n)) if n == "Save changes"
        ));
    }

    #[test]
    fn persist_correction_writes_patch_and_audit_row() {
        let tmp = tempfile::TempDir::new().unwrap();
        let run = crate::sidecar::prepare_run_root(tmp.path(), "r1").unwrap();
        let heal = Heal {
            strategy: "digits-tolerant",
            from: "Row 41".into(),
            to: "Row 42".into(),
            locator: Locator::Role(LocatorRole {
                role: "button".into(),
                name: Some(NameMatch::Plain("Row 42".into())),
                scope: None,
                tolerate: None,
            }),
        };
        persist_correction(&run, "hashabc", "s1", &heal).unwrap();

        let patch: serde_json::Value = serde_json::from_slice(
            &fs::read(run.run_root.join("diffs").join("s1.patch.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(patch["schema"], "heal-patch/v1");
        assert_eq!(patch["scenarioContentHash"], "hashabc");
        assert_eq!(patch["newLocator"]["name"], "Row 42");

        let rows = fs::read_to_string(run.run_root.join("heal.jsonl")).unwrap();
        let row: serde_json::Value = serde_json::from_str(rows.trim()).unwrap();
        assert_eq!(row["mode"], "locator-correction");
        assert_eq!(row["strategy"], "digits-tolerant");
        assert_eq!(row["to"], "Row 42");
    }

    #[test]
    fn persist_rejection_records_evidence() {
        let tmp = tempfile::TempDir::new().unwrap();
        let run = crate::sidecar::prepare_run_root(tmp.path(), "r1").unwrap();
        persist_rejection(&run, "s2", &["Email already taken".to_string()]).unwrap();
        let rows = fs::read_to_string(run.run_root.join("heal.jsonl")).unwrap();
        let row: serde_json::Value = serde_json::from_str(rows.trim()).unwrap();
        assert_eq!(row["mode"], "value-rejection");
        assert!(row["rationale"]
            .as_str()
            .unwrap()
            .contains("Email already taken"));
    }

    #[test]
    fn parse_names_reads_json_payload() {
        assert_eq!(
            parse_names(r#"{"names":["Save","Cancel"]}"#),
            vec!["Save", "Cancel"]
        );
        // agent-browser may wrap the serialized string in quotes.
        assert_eq!(
            parse_names(r#""{\"names\":[\"Save\"]}""#),
            vec![] as Vec<String>
        ); // escaped JSON — unparseable → empty
        assert_eq!(parse_names(""), vec![] as Vec<String>);
    }
}
