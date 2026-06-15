//! `heal-list` verb — enumerate heal-response files under a scenario.
//!
//! Walks `<sid>/replays/<runId>/heal-responses/*.json`, classifies each
//! entry as `value-correction` or `reject`, and reports whether the
//! response has been consumed by `heal-apply` (file renamed to
//! `<stepId>.applied.json`). Useful to answer 'what heal decisions has
//! the team recorded against this scenario, and which have been
//! applied?' without grepping.
//!
//! CLI shape:
//!
//!   agent-qa heal-list <sid> [--run <runId>] [--json]
//!
//! Without `--run`, every run that has a `heal-responses/` directory is
//! scanned. `--json` emits a structured array on stdout for automation.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::Value as Json;

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let entries = collect(&opts)?;
    if opts.json {
        let body = serde_json::to_string_pretty(&entries)?;
        println!("{body}");
    } else {
        render_text(&entries);
    }
    Ok(0)
}

fn print_help() {
    println!(
                "agent-qa heal-list \u{2014} list heal-responses under a scenario\n\nUsage:\n  agent-qa heal-list <sid> [--run <runId>] [--mode <m>] [--applied | --unapplied] [--json]\n\nWalks <sid>/replays/<runId>/heal-responses/*.json (or only --run when\ngiven). Reports stepId, mode (value-correction / reject), value, and\nwhether the response was consumed by heal-apply (file renamed to\n<stepId>.applied.json).\n\nFilters:\n  --mode value-correction|reject   Narrow by heal kind\n  --applied / --unapplied           Show only consumed / pending"
    );
}

#[derive(Debug, Clone)]
struct Opts {
    sid: String,
    run_filter: Option<String>,
    mode_filter: Option<String>,
    only_applied: bool,
    only_unapplied: bool,
    json: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    run_id: String,
    step_id: String,
    mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    applied: bool,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut sid: Option<String> = None;
    let mut run_filter: Option<String> = None;
    let mut mode_filter: Option<String> = None;
    let mut only_applied = false;
    let mut only_unapplied = false;
    let mut json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--json" => json = true,
            "--applied" => only_applied = true,
            "--unapplied" => only_unapplied = true,
            "--mode" => mode_filter = it.next().cloned(),
            s if s.starts_with("--mode=") => mode_filter = Some(s["--mode=".len()..].to_string()),
            "--run" => run_filter = it.next().cloned(),
            s if s.starts_with("--run=") => run_filter = Some(s["--run=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!("unexpected positional {other:?}; usage: heal-list <sid>");
                }
                sid = Some(other.to_string());
            }
        }
    }
    let sid = sid.ok_or_else(|| anyhow!("usage: heal-list <sid> [--run <runId>] [--json]"))?;
    if only_applied && only_unapplied {
        bail!("--applied and --unapplied are mutually exclusive");
    }
    if let Some(m) = mode_filter.as_deref() {
        if m != "value-correction" && m != "reject" {
            bail!("--mode expects 'value-correction' or 'reject', got {m:?}");
        }
    }
    Ok(Opts {
        sid,
        run_filter,
        mode_filter,
        only_applied,
        only_unapplied,
        json,
    })
}

fn collect(opts: &Opts) -> Result<Vec<Entry>> {
    let scenario_dir = paths::scenario_dir(&opts.sid)?;
    let replays = scenario_dir.join("replays");
    let mut out: Vec<Entry> = Vec::new();
    let runs = match fs::read_dir(&replays) {
        Ok(it) => it,
        Err(_) => return Ok(out),
    };
    for entry in runs.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let run_id = entry.file_name().to_string_lossy().into_owned();
        if let Some(filter) = &opts.run_filter {
            if &run_id != filter {
                continue;
            }
        }
        let hr_dir = path.join("heal-responses");
        if !hr_dir.is_dir() {
            continue;
        }
        scan_run(&run_id, &hr_dir, &mut out)?;
    }
    out.sort_by(|a, b| {
        (a.run_id.as_str(), a.step_id.as_str()).cmp(&(b.run_id.as_str(), b.step_id.as_str()))
    });
    if let Some(m) = opts.mode_filter.as_deref() {
        out.retain(|e| e.mode == m);
    }
    if opts.only_applied {
        out.retain(|e| e.applied);
    }
    if opts.only_unapplied {
        out.retain(|e| !e.applied);
    }
    Ok(out)
}

fn scan_run(run_id: &str, hr_dir: &Path, out: &mut Vec<Entry>) -> Result<()> {
    for entry in fs::read_dir(hr_dir).with_context(|| format!("read {}", hr_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        // Two file shapes co-exist:
        //   <stepId>.json         — pending (not yet consumed)
        //   <stepId>.applied.json — already consumed by heal-apply
        let (step_id, applied) = if let Some(stem) = name.strip_suffix(".applied.json") {
            (stem.to_string(), true)
        } else if let Some(stem) = name.strip_suffix(".json") {
            (stem.to_string(), false)
        } else {
            continue;
        };
        let body = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let v: Json = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mode = v
            .get("mode")
            .and_then(|m| m.as_str())
            .unwrap_or("(unknown)")
            .to_string();
        let value = v.get("value").and_then(|m| m.as_str()).map(str::to_string);
        out.push(Entry {
            run_id: run_id.to_string(),
            step_id,
            mode,
            value,
            applied,
        });
    }
    Ok(())
}

fn render_text(entries: &[Entry]) {
    if entries.is_empty() {
        println!("(no heal-responses)");
        return;
    }
    println!(
        "{:<32} {:<10} {:<18} {:<8} value",
        "runId", "stepId", "mode", "applied"
    );
    println!("{}", "-".repeat(95));
    for e in entries {
        let value = e.value.as_deref().unwrap_or("-");
        let truncated: String = value.chars().take(40).collect();
        let suffix = if value.chars().count() > 40 {
            "\u{2026}"
        } else {
            ""
        };
        let applied = if e.applied { "yes" } else { "no" };
        println!(
            "{:<32} {:<10} {:<18} {:<8} {}{}",
            e.run_id, e.step_id, e.mode, applied, truncated, suffix
        );
    }
    let applied = entries.iter().filter(|e| e.applied).count();
    let pending = entries.len() - applied;
    println!();
    println!(
        "{} response(s): {} applied, {} pending",
        entries.len(),
        applied,
        pending
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp);
    }
    fn teardown() {
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
    }

    fn write_response(
        jdir: &Path,
        run: &str,
        step: &str,
        mode: &str,
        value: Option<&str>,
        applied: bool,
    ) {
        let dir = jdir.join("replays").join(run).join("heal-responses");
        fs::create_dir_all(&dir).unwrap();
        let mut obj = serde_json::Map::new();
        obj.insert("stepId".into(), json!(step));
        obj.insert("mode".into(), json!(mode));
        if let Some(v) = value {
            obj.insert("value".into(), json!(v));
        }
        obj.insert("recordedAt".into(), json!("now"));
        let filename = if applied {
            format!("{step}.applied.json")
        } else {
            format!("{step}.json")
        };
        fs::write(
            dir.join(filename),
            serde_json::to_string_pretty(&Json::Object(obj)).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn collect_finds_pending_and_applied_across_runs() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_response(&jdir, "rA", "s1", "value-correction", Some("v1"), false);
        write_response(&jdir, "rA", "s2", "reject", None, false);
        write_response(&jdir, "rB", "s1", "value-correction", Some("v2"), true);

        let entries = collect(&Opts {
            sid: "j1".into(),
            run_filter: None,
            mode_filter: None,
            only_applied: false,
            only_unapplied: false,
            json: false,
        })
        .unwrap();
        assert_eq!(entries.len(), 3);
        let by_id: std::collections::HashMap<(String, String), &Entry> = entries
            .iter()
            .map(|e| ((e.run_id.clone(), e.step_id.clone()), e))
            .collect();
        let r_a_s1 = by_id.get(&("rA".into(), "s1".into())).unwrap();
        assert!(!r_a_s1.applied);
        assert_eq!(r_a_s1.value.as_deref(), Some("v1"));
        let r_a_s2 = by_id.get(&("rA".into(), "s2".into())).unwrap();
        assert_eq!(r_a_s2.mode, "reject");
        assert!(r_a_s2.value.is_none());
        let r_b_s1 = by_id.get(&("rB".into(), "s1".into())).unwrap();
        assert!(r_b_s1.applied);
        teardown();
    }

    #[test]
    fn run_filter_narrows_results() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_response(&jdir, "rA", "s1", "value-correction", Some("v"), false);
        write_response(&jdir, "rB", "s1", "value-correction", Some("v"), false);
        let entries = collect(&Opts {
            sid: "j1".into(),
            run_filter: Some("rB".into()),
            mode_filter: None,
            only_applied: false,
            only_unapplied: false,
            json: false,
        })
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].run_id, "rB");
        teardown();
    }

    #[test]
    fn empty_when_no_heal_responses() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        fs::create_dir_all(tmp.path().join("j1/replays/rA")).unwrap();
        let entries = collect(&Opts {
            sid: "j1".into(),
            run_filter: None,
            mode_filter: None,
            only_applied: false,
            only_unapplied: false,
            json: false,
        })
        .unwrap();
        assert!(entries.is_empty());
        teardown();
    }

    #[test]
    fn mode_filter_drops_other_kinds() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_response(&jdir, "rA", "s1", "value-correction", Some("v"), false);
        write_response(&jdir, "rA", "s2", "reject", None, false);
        let only_rejects = collect(&Opts {
            sid: "j1".into(),
            run_filter: None,
            mode_filter: Some("reject".into()),
            only_applied: false,
            only_unapplied: false,
            json: false,
        })
        .unwrap();
        assert_eq!(only_rejects.len(), 1);
        assert_eq!(only_rejects[0].mode, "reject");
        teardown();
    }

    #[test]
    fn applied_unapplied_flags_partition_results() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_response(&jdir, "rA", "s1", "value-correction", Some("v"), true);
        write_response(&jdir, "rA", "s2", "value-correction", Some("v"), false);
        let applied = collect(&Opts {
            sid: "j1".into(),
            run_filter: None,
            mode_filter: None,
            only_applied: true,
            only_unapplied: false,
            json: false,
        })
        .unwrap();
        assert_eq!(applied.len(), 1);
        assert!(applied[0].applied);
        let unapplied = collect(&Opts {
            sid: "j1".into(),
            run_filter: None,
            mode_filter: None,
            only_applied: false,
            only_unapplied: true,
            json: false,
        })
        .unwrap();
        assert_eq!(unapplied.len(), 1);
        assert!(!unapplied[0].applied);
        teardown();
    }
}
