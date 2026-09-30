//! `plan` — list, inspect, and run test plans from the terminal.
//!
//! Plans/cases/sets are the workbench's "execute + track" primitive, stored
//! as JSON under the scenarios root:
//!
//!   _plans/<id>/plan.json   { schema:"plan/1", scope:{ setIds, caseIds } }
//!   _sets/<id>/set.json     { schema:"set/1", mode:"manual"|"tag",
//!                             caseIds, tagQuery }
//!   _cases/<id>/case.json   { schema:"case/1", title, tags, scenarioSid }
//!
//! `plan run` is the CLI twin of the workbench's Run action (POST
//! /api/plans/:id/run): resolve the member cases — sets first (in scope
//! order, each set expanded against every case), then explicit caseIds,
//! deduped — then replay each case's linked scenario and roll up
//! pass/fail/skipped. This is the hook a CI gate calls once the plan
//! exists: `agent-qa plan run regression --json` exits non-zero when any
//! member scenario fails.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde::Serialize;

use crate::paths;
use crate::runner::{RunOptions, ScenarioSource};

const PLANS_DIRNAME: &str = "_plans";
const SETS_DIRNAME: &str = "_sets";
const CASES_DIRNAME: &str = "_cases";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanFile {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    scope: PlanScope,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanScope {
    #[serde(default)]
    set_ids: Vec<String>,
    #[serde(default)]
    case_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetFile {
    id: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    case_ids: Vec<String>,
    #[serde(default)]
    tag_query: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaseFile {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    scenario_sid: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanRow {
    id: String,
    name: String,
    case_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberRow {
    case_id: String,
    title: String,
    scenario_sid: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunRow {
    case_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sid: Option<String>,
    /// `pass` | `fail` | `skipped`
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn plans_dir(root: &Path) -> PathBuf {
    root.join(PLANS_DIRNAME)
}
fn sets_dir(root: &Path) -> PathBuf {
    root.join(SETS_DIRNAME)
}
fn cases_dir(root: &Path) -> PathBuf {
    root.join(CASES_DIRNAME)
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// List-loaders skip unreadable entries but say so — a corrupt set/case
/// file silently vanishing from every plan's scope is the kind of silent
/// wrong-answer these warnings exist to surface.
fn load_json_list<T: for<'de> Deserialize<'de>>(dir: &Path, file_name: &str) -> Vec<T> {
    list_ids(dir)
        .into_iter()
        .filter_map(|id| {
            let file = dir.join(&id).join(file_name);
            match read_json_file(&file) {
                Some(v) => Some(v),
                None if file.exists() => {
                    eprintln!("[plan] warning: unreadable {} — skipped", file.display());
                    None
                }
                None => None,
            }
        })
        .collect()
}

/// Sorted, safe-segmented subdir ids under `dir` (missing dir → empty).
fn list_ids(dir: &Path) -> Vec<String> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };
    let mut ids: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| paths::is_safe(n))
        .collect();
    ids.sort();
    ids
}

fn load_plan(root: &Path, id: &str) -> Result<PlanFile> {
    paths::is_safe(id)
        .then_some(())
        .ok_or_else(|| anyhow!("plan id must be a safe path segment, got {id:?}"))?;
    let file = plans_dir(root).join(id).join("plan.json");
    let bytes =
        fs::read(&file).map_err(|_| anyhow!("plan: no such plan {id:?} ({})", file.display()))?;
    // A present-but-broken plan file is not "no such plan" — say it plainly.
    serde_json::from_slice(&bytes)
        .with_context(|| format!("plan {id:?}: unparseable {}", file.display()))
}

fn load_sets(root: &Path) -> Vec<SetFile> {
    load_json_list(&sets_dir(root), "set.json")
}

fn load_cases(root: &Path) -> Vec<CaseFile> {
    load_json_list(&cases_dir(root), "case.json")
}

/// Members of one set: tag mode matches any case carrying ≥1 of the set's
/// tags (in case-list order); manual keeps stored caseIds order filtered to
/// ids that still exist. Mirrors report-server.js::resolveSetCaseIds.
fn resolve_set_case_ids(set: &SetFile, all_cases: &[CaseFile]) -> Vec<String> {
    if set.mode.as_deref() == Some("tag") {
        let want: BTreeSet<&str> = set.tag_query.iter().map(String::as_str).collect();
        if want.is_empty() {
            return Vec::new();
        }
        return all_cases
            .iter()
            .filter(|c| c.tags.iter().any(|t| want.contains(t.as_str())))
            .map(|c| c.id.clone())
            .collect();
    }
    let by_id: BTreeSet<&str> = all_cases.iter().map(|c| c.id.as_str()).collect();
    set.case_ids
        .iter()
        .filter(|cid| by_id.contains(cid.as_str()))
        .cloned()
        .collect()
}

/// Deduped member case ids: plan's sets in scope order, then explicit
/// caseIds filtered to existing cases. Mirrors
/// report-server.js::resolvePlanCaseIds.
fn resolve_plan_case_ids(plan: &PlanFile, sets: &[SetFile], cases: &[CaseFile]) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut add = |cid: String| {
        if seen.insert(cid.clone()) {
            order.push(cid);
        }
    };
    let set_by_id: BTreeMap<&str, &SetFile> = sets.iter().map(|s| (s.id.as_str(), s)).collect();
    for sid in &plan.scope.set_ids {
        match set_by_id.get(sid.as_str()) {
            Some(set) => {
                for cid in resolve_set_case_ids(set, cases) {
                    add(cid);
                }
            }
            // A scope entry that resolves to nothing used to shrink the run
            // silently — a typo'd setId is indistinguishable from an empty
            // set. Name it.
            None => eprintln!(
                "[plan] warning: plan {:?} references unknown set {sid:?}",
                plan.id
            ),
        }
    }
    let case_ids: BTreeSet<&str> = cases.iter().map(|c| c.id.as_str()).collect();
    for cid in &plan.scope.case_ids {
        if case_ids.contains(cid.as_str()) {
            add(cid.clone());
        } else {
            eprintln!(
                "[plan] warning: plan {:?} references unknown case {cid:?}",
                plan.id
            );
        }
    }
    order
}

fn list_plan_rows(root: &Path) -> Vec<PlanRow> {
    let cases = load_cases(root);
    let sets = load_sets(root);
    let mut out = Vec::new();
    for id in list_ids(&plans_dir(root)) {
        let file = plans_dir(root).join(&id).join("plan.json");
        match read_json_file::<PlanFile>(&file) {
            Some(plan) => out.push(PlanRow {
                name: plan.name.clone().unwrap_or_else(|| plan.id.clone()),
                case_count: resolve_plan_case_ids(&plan, &sets, &cases).len(),
                id: plan.id,
            }),
            None if file.exists() => {
                eprintln!("[plan] warning: unreadable {} — skipped", file.display());
            }
            None => {}
        }
    }
    out
}

fn member_rows(root: &Path, plan: &PlanFile) -> Vec<MemberRow> {
    let cases = load_cases(root);
    let sets = load_sets(root);
    let by_id: BTreeMap<&str, &CaseFile> = cases.iter().map(|c| (c.id.as_str(), c)).collect();
    resolve_plan_case_ids(plan, &sets, &cases)
        .into_iter()
        .filter_map(|cid| {
            by_id.get(cid.as_str()).map(|c| MemberRow {
                case_id: cid.clone(),
                title: c.title.clone().unwrap_or_else(|| c.id.clone()),
                scenario_sid: c.scenario_sid.clone(),
            })
        })
        .collect()
}

/// Per-member replay outcome: (row, ok) where ok=false means the scenario
/// ran and failed; skipped members are rows with `status: "skipped"`.
fn run_plan(root: &Path, plan: &PlanFile, opts: &RunOpts) -> Result<(Vec<RunRow>, bool)> {
    let members = member_rows(root, plan);
    if members.is_empty() {
        bail!("plan {:?} resolves to no member cases", plan.id);
    }
    let mut rows: Vec<RunRow> = Vec::new();
    let mut all_ok = true;
    for m in &members {
        let sid = match &m.scenario_sid {
            Some(sid) if paths::is_safe(sid) => sid.clone(),
            _ => {
                rows.push(RunRow {
                    case_id: m.case_id.clone(),
                    sid: None,
                    status: "skipped".into(),
                    reason: Some("no recorded scenario".into()),
                });
                continue;
            }
        };
        if !paths::scenario_dir(&sid)?.join("scenario.json").exists() {
            rows.push(RunRow {
                case_id: m.case_id.clone(),
                sid: Some(sid),
                status: "skipped".into(),
                reason: Some("scenario missing".into()),
            });
            continue;
        }
        eprintln!("[plan] {} → replay {sid}", m.case_id);
        let run_opts = RunOptions {
            source: ScenarioSource::Sid(sid.clone()),
            profile: opts.profile.clone(),
            persona: None,
            environment: None,
            session_name: opts
                .profile
                .as_ref()
                .map(|p| format!("{p}-session"))
                .unwrap_or_else(|| format!("replay-{sid}")),
            heal_from_run: None,
            headed: opts.headed,
            browser_profile: None,
            input_overrides: opts.params.clone(),
            dry_run: opts.dry_run,
            no_sidecars: opts.no_sidecars,
            quiet: true,
            plain: true,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
            fresh_browser: false,
        };
        match crate::runner::run(&run_opts) {
            Ok(summary) => {
                all_ok &= summary.ok;
                rows.push(RunRow {
                    case_id: m.case_id.clone(),
                    sid: Some(sid),
                    status: if summary.ok { "pass" } else { "fail" }.into(),
                    reason: None,
                });
            }
            Err(e) => {
                all_ok = false;
                rows.push(RunRow {
                    case_id: m.case_id.clone(),
                    sid: Some(sid),
                    status: "fail".into(),
                    reason: Some(format!("{e:#}")),
                });
            }
        }
    }
    Ok((rows, all_ok))
}

struct RunOpts {
    profile: Option<String>,
    params: BTreeMap<String, String>,
    headed: bool,
    dry_run: bool,
    no_sidecars: bool,
}

pub fn run(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    let mut positionals: Vec<String> = Vec::new();
    let mut opts = RunOpts {
        profile: None,
        params: BTreeMap::new(),
        headed: false,
        dry_run: false,
        no_sidecars: false,
    };
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json_out = true,
            "--profile" => {
                opts.profile = Some(
                    it.next()
                        .cloned()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| anyhow!("--profile requires a value"))?,
                )
            }
            s if s.starts_with("--profile=") => {
                opts.profile = Some(s["--profile=".len()..].to_string())
            }
            "--param" | "-p" => {
                let pair = it
                    .next()
                    .ok_or_else(|| anyhow!("--param requires name=value"))?;
                let (k, v) = pair
                    .split_once('=')
                    .ok_or_else(|| anyhow!("--param expects name=value, got {pair:?}"))?;
                opts.params.insert(k.to_string(), v.to_string());
            }
            s if s.starts_with("--param=") => {
                let pair = &s["--param=".len()..];
                let (k, v) = pair
                    .split_once('=')
                    .ok_or_else(|| anyhow!("--param= expects name=value, got {pair:?}"))?;
                opts.params.insert(k.to_string(), v.to_string());
            }
            "--headed" => opts.headed = true,
            "--headless" => opts.headed = false,
            "--dry-run" => opts.dry_run = true,
            "--no-sidecars" => opts.no_sidecars = true,
            "-h" | "--help" => {
                print_help();
                return Ok(0);
            }
            s if s.starts_with("--") => bail!("plan: unknown flag {s:?}"),
            other => positionals.push(other.to_string()),
        }
    }
    let root = paths::scenarios_root();
    match positionals.first().map(String::as_str) {
        Some("list") | None => {
            let rows = list_plan_rows(&root);
            if json_out {
                // single-line JSON on the LAST stdout line (workbench contract)
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({ "plans": rows }))?
                );
            } else if rows.is_empty() {
                println!("no plans under {}", plans_dir(&root).display());
            } else {
                for r in rows {
                    println!("{:<24} {:>5}  {}", r.id, r.case_count, r.name);
                }
            }
            Ok(0)
        }
        Some("cases") => {
            let id = positionals
                .get(1)
                .ok_or_else(|| anyhow!("usage: plan cases <planId>"))?;
            let plan = load_plan(&root, id)?;
            let rows = member_rows(&root, &plan);
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({ "cases": rows }))?
                );
            } else {
                for r in rows {
                    println!(
                        "{:<24} {:<24} {}",
                        r.case_id,
                        r.scenario_sid.as_deref().unwrap_or("-"),
                        r.title
                    );
                }
            }
            Ok(0)
        }
        Some("run") => {
            let id = positionals
                .get(1)
                .ok_or_else(|| anyhow!("usage: plan run <planId>"))?;
            let plan = load_plan(&root, id)?;
            let (rows, all_ok) = run_plan(&root, &plan, &opts)?;
            let started = rows.iter().filter(|r| r.status != "skipped").count();
            let passed = rows.iter().filter(|r| r.status == "pass").count();
            let failed = rows.iter().filter(|r| r.status == "fail").count();
            let skipped = rows.iter().filter(|r| r.status == "skipped").count();
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({
                        "planId": plan.id,
                        "ok": all_ok,
                        "started": started,
                        "passed": passed,
                        "failed": failed,
                        "skipped": skipped,
                        "results": rows,
                    }))?
                );
            } else {
                for r in &rows {
                    match r.status.as_str() {
                        "skipped" => println!(
                            "SKIP {:<24} {}",
                            r.case_id,
                            r.reason.as_deref().unwrap_or("")
                        ),
                        s => println!(
                            "{} {:<24} {}",
                            s.to_uppercase(),
                            r.case_id,
                            r.sid.as_deref().unwrap_or("")
                        ),
                    }
                }
                println!(
                    "plan {} — {passed}/{started} passed, {skipped} skipped",
                    plan.id
                );
            }
            if started == 0 {
                // Every member was skipped — an exit-0 "0/0 passed" would read
                // as a green suite that verified nothing.
                bail!(
                    "plan {:?}: 0 of {} members ran — all skipped (unrecorded/missing scenarios); record the scenarios or fix the case links",
                    plan.id,
                    rows.len()
                );
            }
            Ok(if all_ok { 0 } else { 1 })
        }
        Some(other) => bail!("plan: unknown sub-command {other:?} (list|cases|run)"),
    }
}

fn print_help() {
    println!(
        "agent-qa plan — run + inspect workbench test plans from the terminal.

Usage:
  plan list [--json]              Plans under <root>/_plans + resolved case counts
  plan cases <planId> [--json]    Resolved member cases (id → sid → title)
  plan run <planId> [flags]       Replay every member case's scenario, roll up
                                  pass/fail/skipped. Exit 0 iff all started
                                  runs pass.
    --profile <p>                 Replay with a persona (session <p>-session)
    --param k=v / -p k=v          Input overrides applied to every member run
    --headed / --headless         Browser mode (default headless)
    --dry-run                     Validate + mint run ids, skip browser dispatch
    --no-sidecars                 Skip per-step screenshots/snapshots
    --json                        Emit a single-line JSON rollup as the last line"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_json(dir: &Path, name: &str, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), body).unwrap();
    }

    fn write_case(root: &Path, id: &str, title: &str, tags: &[&str], sid: Option<&str>) {
        let tags_json = serde_json::to_string(tags).unwrap();
        let sid_json = sid.map(|s| format!("\"{s}\"")).unwrap_or("null".into());
        write_json(
            &root.join(CASES_DIRNAME).join(id),
            "case.json",
            &format!(
                r#"{{"schema":"case/1","id":"{id}","title":"{title}","tags":{tags_json},"scenarioSid":{sid_json}}}"#
            ),
        );
    }

    fn write_set(root: &Path, id: &str, mode: &str, case_ids: &[&str], tag_query: &[&str]) {
        write_json(
            &root.join(SETS_DIRNAME).join(id),
            "set.json",
            &format!(
                r#"{{"schema":"set/1","id":"{id}","mode":"{mode}","caseIds":{},"tagQuery":{}}}"#,
                serde_json::to_string(case_ids).unwrap(),
                serde_json::to_string(tag_query).unwrap()
            ),
        );
    }

    fn write_plan(root: &Path, id: &str, set_ids: &[&str], case_ids: &[&str]) {
        write_json(
            &root.join(PLANS_DIRNAME).join(id),
            "plan.json",
            &format!(
                r#"{{"schema":"plan/1","id":"{id}","name":"{id}","scope":{{"setIds":{},"caseIds":{}}}}}"#,
                serde_json::to_string(set_ids).unwrap(),
                serde_json::to_string(case_ids).unwrap()
            ),
        );
    }

    #[test]
    fn resolve_plan_case_ids_unions_sets_then_explicits_deduped() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_case(root, "c-a", "A", &["smoke"], Some("s-a"));
        write_case(root, "c-b", "B", &["reg"], Some("s-b"));
        write_case(root, "c-c", "C", &["smoke", "reg"], Some("s-c"));
        write_case(root, "c-dead", "D", &["gone"], None);
        write_set(root, "set-smoke", "tag", &[], &["smoke"]);
        write_set(root, "set-manual", "manual", &["c-b", "c-missing"], &[]);
        write_plan(root, "p", &["set-smoke", "set-manual"], &["c-a", "c-dead"]);

        let plan = load_plan(root, "p").unwrap();
        let ids = resolve_plan_case_ids(&plan, &load_sets(root), &load_cases(root));
        // tag set yields c-a + c-c (case-list order); manual adds c-b;
        // explicit c-a dedupes, c-dead exists but has no sid (still a member).
        assert_eq!(ids, vec!["c-a", "c-c", "c-b", "c-dead"]);
    }

    #[test]
    fn list_plan_rows_reports_resolved_counts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_case(root, "c-a", "A", &[], Some("s-a"));
        write_case(root, "c-b", "B", &[], Some("s-b"));
        write_set(root, "set-x", "manual", &["c-a", "c-b"], &[]);
        write_plan(root, "p-full", &["set-x"], &[]);
        write_plan(root, "p-one", &[], &["c-a"]);

        let rows = list_plan_rows(root);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "p-full");
        assert_eq!(rows[0].case_count, 2);
        assert_eq!(rows[1].case_count, 1);
    }

    #[test]
    fn member_rows_pairs_case_with_sid() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_case(root, "c-a", "Login A", &[], Some("s-a"));
        write_case(root, "c-b", "Login B", &[], None);
        write_plan(root, "p", &[], &["c-b", "c-a"]);

        let plan = load_plan(root, "p").unwrap();
        let rows = member_rows(root, &plan);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].case_id, "c-b");
        assert_eq!(rows[0].scenario_sid, None);
        assert_eq!(rows[1].scenario_sid.as_deref(), Some("s-a"));
        assert_eq!(rows[1].title, "Login A");
    }

    #[test]
    fn load_plan_rejects_unknown_or_unsafe_id() {
        let tmp = TempDir::new().unwrap();
        assert!(load_plan(tmp.path(), "nope").is_err());
        assert!(load_plan(tmp.path(), "../x").is_err());
    }

    #[test]
    fn load_plan_names_unparseable_instead_of_no_such_plan() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("_plans").join("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plan.json"), b"{corrupt!!!").unwrap();
        let err = load_plan(tmp.path(), "broken").unwrap_err().to_string();
        assert!(err.contains("unparseable"), "{err}");
        assert!(!err.contains("no such plan"), "{err}");
    }

    #[test]
    fn plan_run_dry_run_replays_members_and_skips_unlinked() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_json(
            &root.join("s-a"),
            "scenario.json",
            r#"{"schema":"scenario/2","id":"s-a","intent":"m","steps":[{"id":"s1","kind":"do","verb":"goto","value":{"from":"literal","literal":"https://example.com"},"intent":"go"}]}"#,
        );
        write_case(root, "c-a", "A", &[], Some("s-a"));
        write_case(root, "c-b", "B", &[], None);
        write_plan(root, "p", &[], &["c-b", "c-a"]);

        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", root);
        let plan = load_plan(root, "p").unwrap();
        let out = run_plan(
            root,
            &plan,
            &RunOpts {
                profile: None,
                params: BTreeMap::new(),
                headed: false,
                dry_run: true,
                no_sidecars: true,
            },
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
        let (rows, all_ok) = out.unwrap();
        assert!(all_ok, "{rows:#?}");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].case_id, "c-b");
        assert_eq!(rows[0].status, "skipped");
        assert_eq!(rows[1].status, "pass");
        // the dry run still minted a run dir under the scenario
        assert!(root
            .join("s-a")
            .join("replays")
            .read_dir()
            .unwrap()
            .next()
            .is_some());
    }

    #[test]
    fn plan_run_fails_when_every_member_is_skipped() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // Both members exist but neither has a scenario link — the run would
        // print SKIP rows and exit 0 with "0/0 passed" before the guard.
        write_case(root, "c-a", "A", &[], None);
        write_case(root, "c-b", "B", &[], None);
        write_plan(root, "p", &[], &["c-a", "c-b"]);

        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", root);
        let out = run(&["run".to_string(), "p".to_string()]);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
        let err = out.unwrap_err().to_string();
        assert!(err.contains("0 of 2 members ran"), "got: {err}");
    }
}
