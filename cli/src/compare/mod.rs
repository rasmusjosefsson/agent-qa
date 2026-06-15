//! `compare` verb — diff per-step ARIA snapshots between two replay runs.
//!
//! Scope (the N-way star pattern lands later):
//!
//!   agent-qa compare <sid> [<runA>] [<runB>]
//!
//!   - No run ids → latest two run ids under `<sid>/replays/`
//!   - One run id  → compare that run against the previous one
//!   - Two run ids → compare them directly
//!
//! For each step that has a snapshot file in BOTH runs, generate a
//! unified diff and write it to:
//!
//!   <sid>/compare/<TS>__<runA>-vs-<runB>/snapshots/<stepId>.diff
//!
//! Plus a top-level compare.md with a one-line summary per step
//! (CHANGED / SAME / MISSING).
//!
//! Exit code: 0 always (compare reports; the caller decides what to do
//! with the results). `--strict` flips that to non-zero on any change.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use similar::TextDiff;

use crate::paths;

mod screenshots;

pub fn run(args: &[String]) -> Result<u8> {
    let mut positional: Vec<String> = Vec::new();
    let mut strict = false;
    let mut pixel_threshold: f64 = 0.0; // tolerated fraction of differing pixels
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--strict" => strict = true,
            "--pixel-threshold" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--pixel-threshold requires a value"))?;
                pixel_threshold = v
                    .parse::<f64>()
                    .map_err(|_| anyhow!("--pixel-threshold must be a number"))?;
            }
            s if s.starts_with("--pixel-threshold=") => {
                pixel_threshold = s["--pixel-threshold=".len()..]
                    .parse::<f64>()
                    .map_err(|_| anyhow!("--pixel-threshold must be a number"))?;
            }
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => positional.push(other.to_string()),
        }
    }
    if positional.is_empty() {
        bail!("usage: compare <sid> [<runA>] [<runB>]");
    }
    let sid = positional.remove(0);
    let scenario_dir = paths::scenario_dir(&sid)?;
    if !scenario_dir.is_dir() {
        bail!("no scenario directory at {}", scenario_dir.display());
    }

    let (run_a, run_b) = pick_runs(&scenario_dir, &positional)?;
    let report = build_compare(&scenario_dir, &run_a, &run_b)?;
    let shots = screenshots::build(
        &scenario_dir,
        &run_a,
        &run_b,
        &report.out_dir,
        pixel_threshold,
    )?;
    write_outputs(&scenario_dir, &report, &shots)?;
    render_text(&report, &shots);

    let changed = report
        .entries
        .iter()
        .any(|e| matches!(e.outcome, Outcome::Changed))
        || shots
            .entries
            .iter()
            .any(|e| matches!(e.outcome, screenshots::ShotOutcome::Changed));
    Ok(if strict && changed { 1 } else { 0 })
}

fn print_help() {
    println!(
        "agent-qa compare — diff per-step ARIA snapshots + screenshots between two replays

Usage:
  agent-qa compare <sid>                       Latest two runs under <sid>
  agent-qa compare <sid> <runA>                <runA> vs the previous run
  agent-qa compare <sid> <runA> <runB>         <runA> vs <runB>

Flags:
  --strict                                     Exit non-zero on any change
  --pixel-threshold <0..1>                     Tolerated fraction of differing pixels (default 0)

Writes:
  <sid>/compare/<TS>__<runA>-vs-<runB>/snapshots/<stepId>.diff
  <sid>/compare/<TS>__<runA>-vs-<runB>/screenshots/<stepId>.diff.png
  <sid>/compare/<TS>__<runA>-vs-<runB>/compare.md"
    );
}

fn pick_runs(scenario_dir: &Path, args: &[String]) -> Result<(String, String)> {
    match args {
        [] => {
            let mut runs = list_run_ids(scenario_dir)?;
            if runs.len() < 2 {
                bail!(
                    "need at least 2 runs under {} to compare (got {})",
                    scenario_dir.join("replays").display(),
                    runs.len()
                );
            }
            let b = runs.pop().unwrap();
            let a = runs.pop().unwrap();
            Ok((a, b))
        }
        [one] => {
            let mut runs = list_run_ids(scenario_dir)?;
            let idx = runs
                .iter()
                .position(|r| r == one)
                .ok_or_else(|| anyhow!("run id {one:?} not found under replays/"))?;
            if idx == 0 {
                bail!("run id {one:?} has no predecessor to compare against");
            }
            let b = runs.remove(idx);
            let a = runs.remove(idx - 1);
            Ok((a, b))
        }
        [a, b] => Ok((a.clone(), b.clone())),
        _ => bail!("at most two positional run ids; got {}", args.len()),
    }
}

fn list_run_ids(scenario_dir: &Path) -> Result<Vec<String>> {
    let replays = scenario_dir.join("replays");
    let mut out: Vec<String> = fs::read_dir(&replays)
        .with_context(|| format!("read {}", replays.display()))?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    Ok(out)
}

// ---------- compare result ----------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    Same,
    Changed,
    OnlyA,
    OnlyB,
}

impl Outcome {
    fn label(&self) -> &'static str {
        match self {
            Outcome::Same => "SAME",
            Outcome::Changed => "CHANGED",
            Outcome::OnlyA => "ONLY-A",
            Outcome::OnlyB => "ONLY-B",
        }
    }
}

#[derive(Debug, Clone)]
struct Entry {
    step_id: String,
    outcome: Outcome,
    diff_text: Option<String>,
}

#[derive(Debug)]
struct ReportData {
    run_a: String,
    run_b: String,
    out_dir: PathBuf,
    entries: Vec<Entry>,
}

fn build_compare(scenario_dir: &Path, run_a: &str, run_b: &str) -> Result<ReportData> {
    let dir_a = scenario_dir.join("replays").join(run_a).join("snapshots");
    let dir_b = scenario_dir.join("replays").join(run_b).join("snapshots");

    let mut ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if dir_a.is_dir() {
        for e in fs::read_dir(&dir_a)?.flatten() {
            if let Some(stem) = e.path().file_stem().and_then(|s| s.to_str()) {
                ids.insert(stem.to_string());
            }
        }
    }
    if dir_b.is_dir() {
        for e in fs::read_dir(&dir_b)?.flatten() {
            if let Some(stem) = e.path().file_stem().and_then(|s| s.to_str()) {
                ids.insert(stem.to_string());
            }
        }
    }

    let mut entries: Vec<Entry> = Vec::new();
    for id in ids {
        let pa = dir_a.join(format!("{id}.txt"));
        let pb = dir_b.join(format!("{id}.txt"));
        match (pa.is_file(), pb.is_file()) {
            (true, true) => {
                let a =
                    fs::read_to_string(&pa).with_context(|| format!("read {}", pa.display()))?;
                let b =
                    fs::read_to_string(&pb).with_context(|| format!("read {}", pb.display()))?;
                if a == b {
                    entries.push(Entry {
                        step_id: id,
                        outcome: Outcome::Same,
                        diff_text: None,
                    });
                } else {
                    let diff = unified_diff(&a, &b);
                    entries.push(Entry {
                        step_id: id,
                        outcome: Outcome::Changed,
                        diff_text: Some(diff),
                    });
                }
            }
            (true, false) => entries.push(Entry {
                step_id: id,
                outcome: Outcome::OnlyA,
                diff_text: None,
            }),
            (false, true) => entries.push(Entry {
                step_id: id,
                outcome: Outcome::OnlyB,
                diff_text: None,
            }),
            (false, false) => {}
        }
    }

    let ts = chrono::Utc::now()
        .format("%Y-%m-%dT%H-%M-%S-%3fZ")
        .to_string();
    let folder = format!("{ts}__{run_a}-vs-{run_b}");
    let out_dir = scenario_dir.join("compare").join(folder);

    Ok(ReportData {
        run_a: run_a.to_string(),
        run_b: run_b.to_string(),
        out_dir,
        entries,
    })
}

fn unified_diff(a: &str, b: &str) -> String {
    TextDiff::from_lines(a, b)
        .unified_diff()
        .context_radius(3)
        .header("a", "b")
        .to_string()
}

// ---------- outputs ----------

fn write_outputs(
    _scenario_dir: &Path,
    r: &ReportData,
    shots: &screenshots::ShotReport,
) -> Result<()> {
    fs::create_dir_all(&r.out_dir).with_context(|| format!("mkdir -p {}", r.out_dir.display()))?;

    // Per-step snapshot diffs.
    let snap_dir = r.out_dir.join("snapshots");
    if r.entries.iter().any(|e| e.diff_text.is_some()) {
        fs::create_dir_all(&snap_dir)?;
    }
    for e in &r.entries {
        if let Some(text) = &e.diff_text {
            let path = snap_dir.join(format!("{}.diff", e.step_id));
            let mut f =
                fs::File::create(&path).with_context(|| format!("create {}", path.display()))?;
            f.write_all(text.as_bytes())?;
        }
    }

    // Screenshot diff outputs (the PNG files were already written by
    // screenshots::build into r.out_dir/screenshots/).

    // compare.md
    let mut md = String::new();
    md.push_str(&format!("# compare {} vs {}\n\n", r.run_a, r.run_b));
    md.push_str("## snapshots\n\n| step | outcome |\n|---|---|\n");
    for e in &r.entries {
        md.push_str(&format!("| {} | {} |\n", e.step_id, e.outcome.label()));
    }
    md.push_str("\n## screenshots\n\n| step | outcome | differing pixels |\n|---|---|---|\n");
    for e in &shots.entries {
        let frac = e
            .differing_fraction
            .map(|f| format!("{:.4}", f))
            .unwrap_or_else(|| "-".into());
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            e.step_id,
            e.outcome.label(),
            frac
        ));
    }
    let md_path = r.out_dir.join("compare.md");
    let mut f =
        fs::File::create(&md_path).with_context(|| format!("create {}", md_path.display()))?;
    f.write_all(md.as_bytes())?;
    Ok(())
}

fn render_text(r: &ReportData, shots: &screenshots::ShotReport) {
    println!("compare {} vs {}", r.run_a, r.run_b);
    println!("output: {}", r.out_dir.display());
    if r.entries.is_empty() && shots.entries.is_empty() {
        println!("(no snapshot or screenshot files in either run)");
        return;
    }
    if !r.entries.is_empty() {
        println!("snapshots:");
        for e in &r.entries {
            println!("  {:<7}  {}", e.outcome.label(), e.step_id);
        }
    }
    if !shots.entries.is_empty() {
        println!("screenshots:");
        for e in &shots.entries {
            let pct = e
                .differing_fraction
                .map(|f| format!("  ({:.2}% differing)", f * 100.0))
                .unwrap_or_default();
            println!("  {:<7}  {}{}", e.outcome.label(), e.step_id, pct);
        }
    }
    let changed_snap = r
        .entries
        .iter()
        .filter(|e| matches!(e.outcome, Outcome::Changed))
        .count();
    let changed_shot = shots
        .entries
        .iter()
        .filter(|e| matches!(e.outcome, screenshots::ShotOutcome::Changed))
        .count();
    println!();
    println!(
        "{} snapshot diff(s); {} screenshot diff(s)",
        changed_snap, changed_shot,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_snap(jdir: &Path, run: &str, step: &str, body: &str) {
        let dir = jdir.join("replays").join(run).join("snapshots");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{step}.txt")), body).unwrap();
    }

    #[test]
    fn pick_runs_no_args_picks_latest_two() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        fs::create_dir_all(jdir.join("replays").join("2026-01-01__a")).unwrap();
        fs::create_dir_all(jdir.join("replays").join("2026-01-02__b")).unwrap();
        fs::create_dir_all(jdir.join("replays").join("2026-01-03__c")).unwrap();
        let (a, b) = pick_runs(&jdir, &[]).unwrap();
        assert_eq!(a, "2026-01-02__b");
        assert_eq!(b, "2026-01-03__c");
    }

    #[test]
    fn pick_runs_one_arg_uses_predecessor() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        fs::create_dir_all(jdir.join("replays").join("r1")).unwrap();
        fs::create_dir_all(jdir.join("replays").join("r2")).unwrap();
        fs::create_dir_all(jdir.join("replays").join("r3")).unwrap();
        let (a, b) = pick_runs(&jdir, &["r3".into()]).unwrap();
        assert_eq!(a, "r2");
        assert_eq!(b, "r3");
    }

    #[test]
    fn build_compare_classifies_outcomes() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        write_snap(&jdir, "rA", "s1", "hello\nworld\n");
        write_snap(&jdir, "rA", "s2", "left-only\n");
        write_snap(&jdir, "rB", "s1", "hello\nWORLD\n");
        write_snap(&jdir, "rB", "s3", "right-only\n");
        write_snap(&jdir, "rA", "s4", "same\n");
        write_snap(&jdir, "rB", "s4", "same\n");

        let r = build_compare(&jdir, "rA", "rB").unwrap();
        let by_id: std::collections::HashMap<&str, &Outcome> = r
            .entries
            .iter()
            .map(|e| (e.step_id.as_str(), &e.outcome))
            .collect();
        assert_eq!(by_id.get("s1"), Some(&&Outcome::Changed));
        assert_eq!(by_id.get("s2"), Some(&&Outcome::OnlyA));
        assert_eq!(by_id.get("s3"), Some(&&Outcome::OnlyB));
        assert_eq!(by_id.get("s4"), Some(&&Outcome::Same));
    }

    #[test]
    fn write_outputs_creates_md_and_diff() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        write_snap(&jdir, "rA", "s1", "before\n");
        write_snap(&jdir, "rB", "s1", "after\n");
        let r = build_compare(&jdir, "rA", "rB").unwrap();
        write_outputs(
            &jdir,
            &r,
            &screenshots::ShotReport {
                entries: Vec::new(),
            },
        )
        .unwrap();
        assert!(r.out_dir.join("compare.md").is_file());
        let diff = r.out_dir.join("snapshots").join("s1.diff");
        assert!(diff.is_file());
        let body = fs::read_to_string(&diff).unwrap();
        assert!(body.contains("-before"), "got: {body}");
        assert!(body.contains("+after"), "got: {body}");
    }

    #[test]
    fn build_compare_handles_no_overlap() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        // both runs exist but only run B has snapshots.
        fs::create_dir_all(jdir.join("replays").join("rA")).unwrap();
        write_snap(&jdir, "rB", "s1", "x\n");
        let r = build_compare(&jdir, "rA", "rB").unwrap();
        assert_eq!(r.entries.len(), 1);
        assert!(matches!(r.entries[0].outcome, Outcome::OnlyB));
    }
}
