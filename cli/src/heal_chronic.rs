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
//!   agent-qa heal-chronic <sid> [--min-runs N] [--json]

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
    } else {
        render_text(&opts, &entries);
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa heal-chronic \u{2014} flag steps that self-heal run after run\n\nUsage:\n  agent-qa heal-chronic <sid> [--min-runs N] [--json]\n\nWalks <sid>/replays/*/heal.jsonl and reports steps that auto-healed in\nat least --min-runs distinct runs (default 2). Chronic steps are stable\nlocator bugs wearing a flaky costume \u{2014} absorb the patch permanently\nwith the printed heal-promote command.\n\nExit code is always 0 on success; a non-empty list means debt exists."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    sid: String,
    min_runs: usize,
    json: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Chronic {
    step_id: String,
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
    let mut min_runs = 2usize;
    let mut json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--json" => json = true,
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
    let sid = sid.ok_or_else(|| anyhow!("usage: heal-chronic <sid> [--min-runs N] [--json]"))?;
    Ok(Opts {
        sid,
        min_runs,
        json,
    })
}

fn collect(opts: &Opts) -> Result<Vec<Chronic>> {
    let scenario_dir = paths::scenario_dir(&opts.sid)?;
    let replays = scenario_dir.join("replays");
    let mut by_step: BTreeMap<String, Acc> = BTreeMap::new();
    let runs = match fs::read_dir(&replays) {
        Ok(it) => it,
        Err(_) => return Ok(Vec::new()),
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
        .filter(|(_, acc)| acc.runs.len() >= opts.min_runs)
        .map(|(step_id, acc)| {
            let last_run_id = acc.runs.iter().next_back().cloned().unwrap_or_default();
            Chronic {
                runs_count: acc.runs.len(),
                runs: acc.runs.into_iter().collect(),
                last_run_id: last_run_id.clone(),
                modes: acc.modes.into_iter().collect(),
                promote: format!(
                    "agent-qa heal-promote {} --run {} --steps {}",
                    opts.sid, last_run_id, step_id
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
    Ok(out)
}

fn render_text(opts: &Opts, entries: &[Chronic]) {
    if entries.is_empty() {
        println!("(no chronic self-heals — min-runs={})", opts.min_runs);
        return;
    }
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
    println!();
    println!(
        "{} chronic step(s) self-healed across >= {} runs — promote the patch to end the drift.",
        entries.len(),
        opts.min_runs
    );
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
            sid: sid.into(),
            min_runs,
            json: false,
        }
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
