//! `heal-chronic` verb — flag steps that self-heal run after run.
//!
//! The inline auto-heal loop repairs locator drift silently: a corrected
//! step produces a `heal.jsonl` row in every run it fires on, and a
//! `diffs/<stepId>.patch.json` patch that `heal-promote` can absorb into
//! the scenario. A step that needs healing in *every* run is not flaky —
//! its recorded locator is simply stale, and the silence is the bug:
//! nobody learns the scenario needs a permanent update.
//!
//! This verb walks `<sid>/replays/<runId>/heal.jsonl`, groups rows by
//! `stepId`, and reports steps that auto-healed in at least `--min-runs`
//! distinct runs (default 2). Each row carries the promote command for
//! the most recent run that produced a patch.
//!
//! CLI shape:
//!
//!   agent-qa heal-chronic <sid | --all> [--min-runs N] [--json] [--issue]
//!
//! `--issue` renders the board as a paste-ready markdown issue body — the
//! handoff path when the debt belongs to a human rather than the operator
//! running the audit.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::Value as Json;

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let entries = collect(&opts)?;
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if opts.issue {
        print!("{}", render_issue(&opts, &entries));
    } else {
        render_text(&opts, &entries);
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa heal-chronic \u{2014} flag steps that self-heal run after run\n\nUsage:\n  agent-qa heal-chronic <sid | --all> [--min-runs N] [--json] [--issue]\n\nWalks <sid>/replays/*/heal.jsonl and reports steps that auto-healed in\nat least --min-runs distinct runs (default 2). --all scans every scenario\nunder the root and prints one cross-scenario board. Chronic steps are\nstable locator bugs wearing a flaky costume \u{2014} absorb the patch\npermanently with the printed heal-promote command. --issue renders the\nboard as a paste-ready markdown issue body instead of the table.\n\nExit code is always 0 on success; a non-empty list means debt exists."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    /// `None` under `--all` (every scenario dir under the root).
    sid: Option<String>,
    all: bool,
    min_runs: usize,
    json: bool,
    issue: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Chronic {
    /// Which scenario this row belongs to — populated always ("sid" in
    /// json) so `--all` rows stay attributable without a wrapper object.
    pub(crate) sid: String,
    pub(crate) step_id: String,
    runs_count: usize,
    runs: Vec<String>,
    last_run_id: String,
    modes: Vec<String>,
    promote: String,
}

#[derive(Default)]
struct Acc {
    runs: BTreeSet<String>,
    modes: BTreeSet<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut sid: Option<String> = None;
    let mut all = false;
    let mut min_runs = 2usize;
    let mut json = false;
    let mut issue = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--json" => json = true,
            "--issue" => issue = true,
            "--all" => all = true,
            "--min-runs" => {
                min_runs = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-runs expects a positive integer"))?;
            }
            s if s.starts_with("--min-runs=") => {
                min_runs = s["--min-runs=".len()..]
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-runs expects a positive integer"))?;
            }
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!("unexpected positional {other:?}; usage: heal-chronic <sid>");
                }
                sid = Some(other.to_string());
            }
        }
    }
    if sid.is_some() == all {
        bail!("usage: heal-chronic <sid | --all> [--min-runs N] [--json] [--issue]");
    }
    if json && issue {
        bail!("--json and --issue are mutually exclusive");
    }
    Ok(Opts {
        sid,
        all,
        min_runs,
        json,
        issue,
    })
}

fn collect(opts: &Opts) -> Result<Vec<Chronic>> {
    if !opts.all {
        let sid = opts.sid.as_deref().unwrap();
        let scenario_dir = paths::scenario_dir(sid)?;
        return Ok(collect_dir(&scenario_dir, opts.min_runs, sid));
    }
    // --all: every directory under the scenarios root gets a scan — dirs
    // without replays/ simply contribute nothing.
    let root = paths::scenarios_root();
    let mut out: Vec<Chronic> = Vec::new();
    let mut dirs: Vec<_> = match fs::read_dir(&root) {
        Ok(it) => it.flatten().map(|e| e.path()).collect(),
        Err(_) => return Ok(out),
    };
    dirs.sort();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        let sid = dir.file_name().unwrap_or_default().to_string_lossy();
        out.extend(collect_dir(&dir, opts.min_runs, &sid));
    }
    // Cross-scenario order: most-chronic first.
    out.sort_by(|a, b| {
        b.runs_count
            .cmp(&a.runs_count)
            .then(a.sid.cmp(&b.sid))
            .then(a.step_id.cmp(&b.step_id))
    });
    Ok(out)
}

/// Collect chronic-heal rows for one scenario directory. `sid` only feeds the
/// printed `heal-promote` command — pass the scenario id, not the path.
pub(crate) fn collect_dir(
    scenario_dir: &std::path::Path,
    min_runs: usize,
    sid: &str,
) -> Vec<Chronic> {
    let replays = scenario_dir.join("replays");
    let mut by_step: BTreeMap<String, Acc> = BTreeMap::new();
    let runs = match fs::read_dir(&replays) {
        Ok(it) => it,
        Err(_) => return Vec::new(),
    };
    for entry in runs.flatten() {
        let run_dir = entry.path();
        if !run_dir.is_dir() {
            continue;
        }
        let run_id = entry.file_name().to_string_lossy().into_owned();
        let heal_file = run_dir.join("heal.jsonl");
        let body = match fs::read_to_string(&heal_file) {
            Ok(b) => b,
            Err(_) => continue,
        };
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: Json = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let step_id = match v.get("stepId").and_then(|s| s.as_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let acc = by_step.entry(step_id).or_default();
            acc.runs.insert(run_id.clone());
            if let Some(m) = v.get("mode").and_then(|m| m.as_str()) {
                acc.modes.insert(m.to_string());
            }
        }
    }
    let mut out: Vec<Chronic> = by_step
        .into_iter()
        .filter(|(_, acc)| acc.runs.len() >= min_runs)
        .map(|(step_id, acc)| {
            let last_run_id = acc.runs.iter().next_back().cloned().unwrap_or_default();
            Chronic {
                sid: sid.to_string(),
                runs_count: acc.runs.len(),
                runs: acc.runs.into_iter().collect(),
                last_run_id: last_run_id.clone(),
                modes: acc.modes.into_iter().collect(),
                promote: format!(
                    "agent-qa heal-promote {} --run {} --steps {}",
                    sid, last_run_id, step_id
                ),
                step_id,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.runs_count
            .cmp(&a.runs_count)
            .then(a.step_id.cmp(&b.step_id))
    });
    out
}

fn render_text(opts: &Opts, entries: &[Chronic]) {
    if entries.is_empty() {
        println!("(no chronic self-heals — min-runs={})", opts.min_runs);
        return;
    }
    if opts.all {
        println!(
            "{:<22} {:<10} {:<6} {:<20} promote",
            "scenario", "stepId", "runs", "modes"
        );
        println!("{}", "-".repeat(120));
        for e in entries {
            println!(
                "{:<22} {:<10} {:<6} {:<20} {}",
                e.sid,
                e.step_id,
                e.runs_count,
                e.modes.join(","),
                e.promote
            );
        }
    } else {
        println!("{:<10} {:<6} {:<20} promote", "stepId", "runs", "modes");
        println!("{}", "-".repeat(100));
        for e in entries {
            println!(
                "{:<10} {:<6} {:<20} {}",
                e.step_id,
                e.runs_count,
                e.modes.join(","),
                e.promote
            );
        }
    }
    println!();
    println!(
        "{} chronic step(s) self-healed across >= {} runs — promote the patch to end the drift.",
        entries.len(),
        opts.min_runs
    );
}

/// Paste-ready markdown issue body for the same board — the handoff path
/// when the debt belongs to a human, not the operator running the audit.
fn render_issue(opts: &Opts, entries: &[Chronic]) -> String {
    let mut out = String::new();
    let scope = if opts.all {
        "all scenarios".to_string()
    } else {
        format!("`{}`", opts.sid.as_deref().unwrap_or_default())
    };
    out.push_str(&format!(
        "## Self-heal debt — {}\n\n{} step(s) auto-healed in >= {} distinct run(s). Each keeps a stale locator alive: the run passes, but only because auto-heal silently rewrote it. Promote the patches below to make the fix permanent.\n\n",
        scope,
        entries.len(),
        opts.min_runs
    ));
    out.push_str(
        "| scenario | step | runs | modes | last run |\n| --- | --- | --- | --- | --- |\n",
    );
    for e in entries {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} | `{}` |\n",
            e.sid,
            e.step_id,
            e.runs_count,
            e.modes.join(", "),
            e.last_run_id
        ));
    }
    out.push_str("\nPromote:\n\n```sh\n");
    for e in entries {
        out.push_str(&format!("{}\n", e.promote));
    }
    out.push_str("```\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp);
    }
    fn teardown() {
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
    }

    fn write_heal_row(jdir: &Path, run: &str, step: &str, mode: &str) {
        let dir = jdir.join("replays").join(run);
        fs::create_dir_all(&dir).unwrap();
        let row = json!({
            "schema": "heal-row/v1",
            "ts": "now",
            "runId": run,
            "stepId": step,
            "mode": mode,
        });
        let mut body = serde_json::to_string(&row).unwrap();
        body.push('\n');
        fs::write(dir.join("heal.jsonl"), body).unwrap();
    }

    fn append_heal_row(jdir: &Path, run: &str, step: &str, mode: &str) {
        let dir = jdir.join("replays").join(run);
        fs::create_dir_all(&dir).unwrap();
        let row = json!({
            "schema": "heal-row/v1",
            "ts": "now",
            "runId": run,
            "stepId": step,
            "mode": mode,
        });
        let mut body = fs::read_to_string(dir.join("heal.jsonl")).unwrap_or_default();
        body.push_str(&serde_json::to_string(&row).unwrap());
        body.push('\n');
        fs::write(dir.join("heal.jsonl"), body).unwrap();
    }

    fn opts(sid: &str, min_runs: usize) -> Opts {
        Opts {
            sid: Some(sid.into()),
            all: false,
            min_runs,
            json: false,
            issue: false,
        }
    }

    #[test]
    fn collect_all_aggregates_across_scenarios_sorted() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        // j1: one step healing in 3 runs; j2: one step in 2 runs; j3: none.
        let j1 = tmp.path().join("j1");
        for r in ["rA", "rB", "rC"] {
            write_heal_row(&j1, r, "s9", "locator-correction");
        }
        let j2 = tmp.path().join("j2");
        for r in ["rA", "rB"] {
            write_heal_row(&j2, r, "s1", "value-override");
        }
        fs::create_dir_all(tmp.path().join("j3")).unwrap();
        let opts = Opts {
            sid: None,
            all: true,
            min_runs: 2,
            json: false,
            issue: false,
        };
        let entries = collect(&opts).unwrap();
        assert_eq!(entries.len(), 2);
        // Most-chronic first, sid attribution intact.
        assert_eq!(entries[0].sid, "j1");
        assert_eq!(entries[0].step_id, "s9");
        assert_eq!(entries[1].sid, "j2");
        assert!(entries[1].promote.contains("heal-promote j2"));
        teardown();
    }

    #[test]
    fn parse_args_all_and_sid_are_exclusive() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let o = parse_args(&["--all".into()]).unwrap();
        assert!(o.all && o.sid.is_none());
        parse_args(&["j1".into(), "--all".into()]).unwrap_err();
        parse_args(&Vec::<String>::new()).unwrap_err();
        teardown();
    }

    #[test]
    fn parse_args_json_and_issue_are_exclusive() {
        parse_args(&["j1".into(), "--json".into(), "--issue".into()]).unwrap_err();
    }

    #[test]
    fn render_issue_emits_table_and_promote_block() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_heal_row(&jdir, "rA", "s1", "locator-correction");
        write_heal_row(&jdir, "rB", "s1", "locator-correction");

        let mut o = opts("j1", 2);
        o.issue = true;
        let entries = collect(&o).unwrap();
        let md = render_issue(&o, &entries);
        assert!(md.contains("## Self-heal debt — `j1`"));
        assert!(md.contains("| `j1` | `s1` | 2 | locator-correction | `rB` |"));
        assert!(md.contains("agent-qa heal-promote j1 --run rB --steps s1"));
        teardown();
    }

    #[test]
    fn chronic_flags_step_healed_in_two_runs() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_heal_row(&jdir, "rA", "s1", "locator-correction");
        write_heal_row(&jdir, "rB", "s1", "locator-correction");
        append_heal_row(&jdir, "rB", "s2", "locator-correction");

        let entries = collect(&opts("j1", 2)).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].step_id, "s1");
        assert_eq!(entries[0].runs_count, 2);
        assert_eq!(entries[0].last_run_id, "rB");
        assert_eq!(entries[0].modes, vec!["locator-correction".to_string()]);
        assert!(entries[0]
            .promote
            .contains("heal-promote j1 --run rB --steps s1"));
        teardown();
    }

    #[test]
    fn duplicate_rows_in_one_run_count_once() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_heal_row(&jdir, "rA", "s1", "locator-correction");
        append_heal_row(&jdir, "rA", "s1", "locator-correction");

        let entries = collect(&opts("j1", 2)).unwrap();
        assert!(entries.is_empty());
        teardown();
    }

    #[test]
    fn min_runs_threshold_filters() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        for run in ["rA", "rB", "rC"] {
            write_heal_row(&jdir, run, "s1", "locator-correction");
        }
        append_heal_row(&jdir, "rC", "s2", "value-rejection");
        write_heal_row(&jdir, "rD", "s2", "value-rejection");

        assert_eq!(collect(&opts("j1", 3)).unwrap().len(), 1);
        assert_eq!(collect(&opts("j1", 2)).unwrap().len(), 2);
        teardown();
    }

    #[test]
    fn empty_scenario_returns_empty() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        fs::create_dir_all(tmp.path().join("j1/replays/rA")).unwrap();
        assert!(collect(&opts("j1", 2)).unwrap().is_empty());
        teardown();
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        for run in ["rA", "rB"] {
            let dir = jdir.join("replays").join(run);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("heal.jsonl"),
                "not json\n{\"noStep\": true}\n{\"stepId\": \"s1\", \"mode\": \"locator-correction\"}\n",
            )
            .unwrap();
        }
        let entries = collect(&opts("j1", 2)).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].step_id, "s1");
        teardown();
    }
}
