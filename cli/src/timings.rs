//! `timings` — compare how long scenarios, steps, and cases take against a
//! committed baseline, so a fresh CI checkout can see a sudden slowdown.
//!
//! `audit slow` needs several replay dirs on the same disk. A pull request
//! runner starts empty, so the comparison lives in a file you commit:
//!
//! ```text
//! <scenarios_root>/_timings/baseline.json
//! ```
//!
//! A row is slow when it is more than `--pct` (default 100, so 2×) over its
//! baseline **and** at least `--min-delta-ms` (default 5000) slower, or when
//! it crosses that entry's `budgetMs` ceiling. A run that takes about as
//! long as last time stays quiet. A run that used to take 10s and now takes
//! 2min does not. Failed runs are skipped: their `ms` is the timeout budget,
//! not the cost.
//!
//! ```text
//! agent-qa timings check [--baseline <path>] [--samples <path>] [--root <dir>]
//!                        [--pct N] [--min-delta-ms N] [--no-steps]
//!                        [--step-floor-ms N] [--format text|json|markdown]
//!                        [--out <path>] [--report]
//! agent-qa timings accept [--baseline <path>] [--samples <path>] [--root <dir>]
//!                         [--no-steps] [--step-floor-ms N] [--prune]
//! ```
//!
//! `--samples` reads an explicit list (a golden-suite rollup, for example)
//! instead of walking replay dirs. `--report` prints the same body and exits
//! 0 even when a row is slow, so a comment step can post the table without
//! failing the job. The default exit code is 1 when anything is slow.
//!
//! Directories whose names start with `_` are reserved (`_cases`, `_plans`,
//! the baseline itself) and are not scenarios.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::time::parse_iso_ms;

const SCHEMA_BASELINE: &str = "agent-qa-timings/v1";
const SCHEMA_SAMPLES: &str = "agent-qa-timing-samples/v1";
const SCHEMA_REPORT: &str = "agent-qa-timing-report/v1";

const DEFAULT_PCT: f64 = 100.0;
const DEFAULT_MIN_DELTA_MS: u64 = 5_000;
const DEFAULT_STEP_FLOOR_MS: u64 = 500;
/// Healthy rows beyond this are summarized so a PR comment stays readable.
const MAX_LISTED_HEALTHY: usize = 40;

pub fn run(args: &[String]) -> Result<u8> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(0);
    }
    let cmd = args[0].as_str();
    if cmd.starts_with('-') {
        print_help();
        bail!("timings: expected `check` or `accept` before flags");
    }
    let opts = parse_args(&args[1..])?;
    match cmd {
        "check" => cmd_check(&opts),
        "accept" => cmd_accept(&opts),
        "help" => {
            print_help();
            Ok(0)
        }
        other => bail!("timings: unknown command {other:?} (expected check or accept)"),
    }
}

fn print_help() {
    println!(
        "\
agent-qa timings — compare durations to a committed baseline

Usage:
  agent-qa timings check [--baseline <path>] [--samples <path>] [--root <dir>]
                         [--pct N] [--min-delta-ms N] [--no-steps]
                         [--step-floor-ms N] [--format text|json|markdown]
                         [--json] [--markdown] [--out <path>] [--report]
  agent-qa timings accept [--baseline <path>] [--samples <path>] [--root <dir>]
                          [--no-steps] [--step-floor-ms N] [--prune]

`check` reads the latest passing full run of each scenario (and, unless
--no-steps, each step at or above --step-floor-ms) and compares it to the
baseline. A row is slow when it is more than --pct percent over the baseline
(default 100, so more than 2×) AND at least --min-delta-ms slower (default
5000), or when it exceeds that entry's budgetMs. New ids are reported and do
not fail the command. Failed and partial (--from/--until) runs are skipped.

Exit 0 when nothing is slow. Exit 1 when a row is slow, unless --report,
which still prints the table and exits 0 so a pull-request comment can post
it. --format markdown is the comment body. --samples reads a
agent-qa-timing-samples/v1 file instead of walking the scenarios root.

`accept` writes the baseline from the current observations. An existing
budgetMs on the same id is kept. Entries that were not observed this run
stay put unless --prune. The default path is <scenarios>/_timings/baseline.json.

Directories named _* are reserved and are not treated as scenarios."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    baseline: Option<PathBuf>,
    samples: Option<PathBuf>,
    root: Option<PathBuf>,
    pct: f64,
    min_delta_ms: u64,
    steps: bool,
    step_floor_ms: u64,
    format: Format,
    out: Option<PathBuf>,
    report_only: bool,
    prune: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    id: String,
    kind: String,
    ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct BaselineEntry {
    kind: String,
    ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineFile {
    schema: String,
    entries: BTreeMap<String, BaselineEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SamplesFile {
    schema: String,
    entries: Vec<SampleEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SampleEntry {
    id: String,
    #[serde(default = "default_sample_kind")]
    kind: String,
    ms: u64,
}

fn default_sample_kind() -> String {
    "case".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok,
    Slow,
    New,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: String,
    kind: String,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_ms: Option<u64>,
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    ratio: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema: String,
    healthy: bool,
    baseline_present: bool,
    pct: f64,
    min_delta_ms: u64,
    slow: usize,
    compared: usize,
    new: usize,
    missing: usize,
    rows: Vec<Row>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut opts = Opts {
        baseline: None,
        samples: None,
        root: None,
        pct: DEFAULT_PCT,
        min_delta_ms: DEFAULT_MIN_DELTA_MS,
        steps: true,
        step_floor_ms: DEFAULT_STEP_FLOOR_MS,
        format: Format::Text,
        out: None,
        report_only: false,
        prune: false,
    };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let value_of = |flag: &str| -> Result<String> {
            args.get(i + 1)
                .cloned()
                .filter(|v| !v.starts_with('-'))
                .ok_or_else(|| anyhow!("{flag} requires a value"))
        };
        match a {
            "--baseline" => {
                opts.baseline = Some(PathBuf::from(value_of(a)?));
                i += 2;
            }
            "--samples" => {
                opts.samples = Some(PathBuf::from(value_of(a)?));
                i += 2;
            }
            "--root" => {
                opts.root = Some(PathBuf::from(value_of(a)?));
                i += 2;
            }
            "--out" => {
                opts.out = Some(PathBuf::from(value_of(a)?));
                i += 2;
            }
            "--pct" => {
                let raw = value_of(a)?;
                let pct: f64 = raw.parse().with_context(|| format!("--pct {raw:?}"))?;
                if !pct.is_finite() || pct < 0.0 {
                    bail!("--pct must be a non-negative number, got {raw}");
                }
                opts.pct = pct;
                i += 2;
            }
            "--min-delta-ms" => {
                let raw = value_of(a)?;
                opts.min_delta_ms = raw
                    .parse()
                    .with_context(|| format!("--min-delta-ms {raw:?}"))?;
                i += 2;
            }
            "--step-floor-ms" => {
                let raw = value_of(a)?;
                opts.step_floor_ms = raw
                    .parse()
                    .with_context(|| format!("--step-floor-ms {raw:?}"))?;
                i += 2;
            }
            "--no-steps" => {
                opts.steps = false;
                i += 1;
            }
            "--report" => {
                opts.report_only = true;
                i += 1;
            }
            "--prune" => {
                opts.prune = true;
                i += 1;
            }
            "--json" => {
                opts.format = Format::Json;
                i += 1;
            }
            "--markdown" => {
                opts.format = Format::Markdown;
                i += 1;
            }
            "--format" => {
                opts.format = parse_format(&value_of(a)?)?;
                i += 2;
            }
            other if other.starts_with("--format=") => {
                opts.format = parse_format(&other["--format=".len()..])?;
                i += 1;
            }
            other => bail!("timings: unknown argument {other}"),
        }
    }
    Ok(opts)
}

fn parse_format(raw: &str) -> Result<Format> {
    match raw {
        "text" => Ok(Format::Text),
        "json" => Ok(Format::Json),
        "markdown" | "md" => Ok(Format::Markdown),
        other => bail!("--format must be text, json, or markdown, got {other:?}"),
    }
}

fn cmd_check(opts: &Opts) -> Result<u8> {
    let root = opts
        .root
        .clone()
        .unwrap_or_else(crate::paths::scenarios_root);
    let baseline_path = baseline_path(opts, &root);
    let (baseline, present) = load_baseline(&baseline_path)?;
    let observations = load_observations(opts, &root)?;
    let report = build_report(
        &observations,
        &baseline,
        present,
        opts.pct,
        opts.min_delta_ms,
    );
    let body = render(&report, opts.format);
    if let Some(path) = &opts.out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(path, &body).with_context(|| format!("write {}", path.display()))?;
    }
    print!("{body}");
    if report.healthy || opts.report_only {
        Ok(0)
    } else {
        Ok(1)
    }
}

fn cmd_accept(opts: &Opts) -> Result<u8> {
    let root = opts
        .root
        .clone()
        .unwrap_or_else(crate::paths::scenarios_root);
    let baseline_path = baseline_path(opts, &root);
    let (existing, _) = load_baseline(&baseline_path)?;
    let observations = load_observations(opts, &root)?;
    if observations.is_empty() {
        bail!("timings accept: no passing durations to record");
    }
    // Observed rows replace `ms` and keep a budget that was already set.
    // Unobserved rows stay unless --prune, which drops them without
    // dropping the budget on a row this run did see.
    let mut entries = BTreeMap::new();
    for obs in &observations {
        let budget_ms = existing.get(&obs.id).and_then(|e| e.budget_ms);
        entries.insert(
            obs.id.clone(),
            BaselineEntry {
                kind: obs.kind.clone(),
                ms: obs.ms,
                budget_ms,
            },
        );
    }
    if !opts.prune {
        for (id, entry) in existing {
            entries.entry(id).or_insert(entry);
        }
    }
    let file = BaselineFile {
        schema: SCHEMA_BASELINE.to_string(),
        entries,
    };
    if let Some(parent) = baseline_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(&file)? + "\n";
    fs::write(&baseline_path, &body)
        .with_context(|| format!("write {}", baseline_path.display()))?;
    println!(
        "wrote {} entries to {}",
        file.entries.len(),
        baseline_path.display()
    );
    Ok(0)
}

fn baseline_path(opts: &Opts, root: &Path) -> PathBuf {
    opts.baseline
        .clone()
        .unwrap_or_else(|| root.join("_timings").join("baseline.json"))
}

fn load_baseline(path: &Path) -> Result<(BTreeMap<String, BaselineEntry>, bool)> {
    if !path.is_file() {
        return Ok((BTreeMap::new(), false));
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let file: BaselineFile = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse {} as a timing baseline", path.display()))?;
    if file.schema != SCHEMA_BASELINE {
        bail!(
            "{}: schema {:?} is not {SCHEMA_BASELINE}",
            path.display(),
            file.schema
        );
    }
    Ok((file.entries, true))
}

fn load_observations(opts: &Opts, root: &Path) -> Result<Vec<Observation>> {
    if let Some(path) = &opts.samples {
        return load_samples(path);
    }
    Ok(collect_scenarios(root, opts.steps, opts.step_floor_ms))
}

fn load_samples(path: &Path) -> Result<Vec<Observation>> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let file: SamplesFile = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse {} as timing samples", path.display()))?;
    if file.schema != SCHEMA_SAMPLES {
        bail!(
            "{}: schema {:?} is not {SCHEMA_SAMPLES}",
            path.display(),
            file.schema
        );
    }
    let mut out = Vec::with_capacity(file.entries.len());
    let mut seen = BTreeMap::<String, ()>::new();
    for entry in file.entries {
        if entry.id.is_empty() {
            bail!("{}: a sample id is empty", path.display());
        }
        if seen.insert(entry.id.clone(), ()).is_some() {
            bail!("{}: duplicate sample id {}", path.display(), entry.id);
        }
        out.push(Observation {
            id: entry.id,
            kind: entry.kind,
            ms: entry.ms,
        });
    }
    Ok(out)
}

fn collect_scenarios(root: &Path, steps: bool, step_floor_ms: u64) -> Vec<Observation> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    let mut dirs: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.to_string();
            if name.starts_with('_') || !p.join("scenario.json").is_file() {
                return None;
            }
            Some((name, p))
        })
        .collect();
    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    for (sid, dir) in dirs {
        let Some((ms, run)) = latest_comparable_run(&dir) else {
            continue;
        };
        out.push(Observation {
            id: format!("scenario:{sid}"),
            kind: "scenario".to_string(),
            ms,
        });
        if steps {
            out.extend(step_observations(&sid, &run, step_floor_ms));
        }
    }
    out
}

/// Newest passing full run: exit 0, not quarantined, not a `--from`/`--until`
/// window. `run_dirs` is lexicographic and run ids lead with an ISO timestamp,
/// so the last dir is the newest. A run that cannot be read is skipped so an
/// older passing run still counts.
fn latest_comparable_run(scenario_dir: &Path) -> Option<(u64, PathBuf)> {
    let runs = crate::paths::run_dirs(&scenario_dir.join("replays"));
    for run in runs.iter().rev() {
        if let Some(ms) = comparable_ms(run) {
            return Some((ms, run.clone()));
        }
    }
    None
}

fn comparable_ms(run: &Path) -> Option<u64> {
    let bytes = fs::read(run.join("audit.json")).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    if v.get("quarantined").and_then(|x| x.as_bool()) == Some(true) {
        return None;
    }
    if v.get("windowFrom")
        .and_then(|x| x.as_str())
        .is_some_and(|s| !s.is_empty())
        || v.get("windowUntil")
            .and_then(|x| x.as_str())
            .is_some_and(|s| !s.is_empty())
    {
        return None;
    }
    if v.get("exitCode").and_then(|x| x.as_i64()) != Some(0) {
        return None;
    }
    let started = v.get("startedAt").and_then(|x| x.as_str())?;
    let finished = v.get("finishedAt").and_then(|x| x.as_str())?;
    Some(
        parse_iso_ms(finished)
            .ok()?
            .saturating_sub(parse_iso_ms(started).ok()?),
    )
}

fn step_observations(sid: &str, run: &Path, floor: u64) -> Vec<Observation> {
    let Ok(body) = fs::read_to_string(run.join("events.jsonl")) else {
        return Vec::new();
    };
    let mut terminal: BTreeMap<String, (bool, Option<u64>)> = BTreeMap::new();
    for line in body.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
        if status != "pass" && status != "fail" {
            continue;
        }
        let Some(id) = v.get("id").and_then(|s| s.as_str()) else {
            continue;
        };
        let ms = v.get("ms").and_then(|m| m.as_u64());
        terminal.insert(id.to_string(), (status == "pass", ms));
    }
    terminal
        .into_iter()
        .filter_map(|(step_id, (passed, ms))| {
            if !passed {
                return None;
            }
            let ms = ms?;
            if ms < floor {
                return None;
            }
            Some(Observation {
                id: format!("step:{sid}/{step_id}"),
                kind: "step".to_string(),
                ms,
            })
        })
        .collect()
}

fn build_report(
    observations: &[Observation],
    baseline: &BTreeMap<String, BaselineEntry>,
    baseline_present: bool,
    pct: f64,
    min_delta_ms: u64,
) -> Report {
    let mut rows: Vec<Row> = observations
        .iter()
        .map(|obs| row_for_observation(obs, baseline.get(&obs.id), pct, min_delta_ms))
        .collect();
    if let Some(suite) = suite_row(observations, baseline, pct, min_delta_ms) {
        rows.push(suite);
    }
    let mut missing = 0usize;
    for (id, entry) in baseline {
        if observations.iter().any(|o| &o.id == id) {
            continue;
        }
        missing += 1;
        rows.push(Row {
            id: id.clone(),
            kind: entry.kind.clone(),
            label: label_of(id, &entry.kind),
            ms: None,
            baseline_ms: Some(entry.ms),
            budget_ms: entry.budget_ms,
            status: Status::Missing,
            ratio: None,
        });
    }
    rows.sort_by(|a, b| row_order(a).cmp(&row_order(b)).then(a.label.cmp(&b.label)));
    let slow = rows.iter().filter(|r| r.status == Status::Slow).count();
    let new = rows.iter().filter(|r| r.status == Status::New).count();
    let compared = rows
        .iter()
        .filter(|r| matches!(r.status, Status::Ok | Status::Slow))
        .count();
    Report {
        schema: SCHEMA_REPORT.to_string(),
        healthy: slow == 0,
        baseline_present,
        pct,
        min_delta_ms,
        slow,
        compared,
        new,
        missing,
        rows,
    }
}

fn row_for_observation(
    obs: &Observation,
    entry: Option<&BaselineEntry>,
    pct: f64,
    min_delta_ms: u64,
) -> Row {
    let (status, baseline_ms, budget_ms, ratio) = match entry {
        None => (Status::New, None, None, None),
        Some(entry) => {
            // A build's wall clock swings with the compile cache. Warm and
            // cold runs of the same sources are both fine; only the ceiling
            // means the build actually got slower.
            let status = if obs.kind == "build" {
                judge_budget_only(obs.ms, entry.budget_ms)
            } else {
                judge(obs.ms, entry.ms, entry.budget_ms, pct, min_delta_ms)
            };
            let ratio = if entry.ms == 0 {
                None
            } else {
                Some(obs.ms as f64 / entry.ms as f64)
            };
            (status, Some(entry.ms), entry.budget_ms, ratio)
        }
    };
    Row {
        id: obs.id.clone(),
        kind: obs.kind.clone(),
        label: label_of(&obs.id, &obs.kind),
        ms: Some(obs.ms),
        baseline_ms,
        budget_ms,
        status,
        ratio,
    }
}

/// A `build` row ignores the ratio. Slow only when a ceiling is set and crossed.
fn judge_budget_only(current: u64, budget_ms: Option<u64>) -> Status {
    if budget_ms.is_some_and(|budget| current > budget) {
        Status::Slow
    } else {
        Status::Ok
    }
}

/// Slow when `current` crosses `budget_ms`, or when it is both more than
/// `pct` percent over `baseline_ms` and at least `min_delta_ms` slower.
/// Exactly `pct` percent over (the 2× line at the default) stays ok.
fn judge(
    current: u64,
    baseline_ms: u64,
    budget_ms: Option<u64>,
    pct: f64,
    min_delta_ms: u64,
) -> Status {
    if budget_ms.is_some_and(|budget| current > budget) {
        return Status::Slow;
    }
    let delta = current.saturating_sub(baseline_ms);
    if delta <= min_delta_ms {
        return Status::Ok;
    }
    let ceiling = baseline_ms as f64 * (1.0 + pct / 100.0);
    if (current as f64) > ceiling {
        Status::Slow
    } else {
        Status::Ok
    }
}

/// Sum of case/scenario ids that exist on both sides. One overlapping id is
/// already its own row, so the suite row starts at two.
fn suite_row(
    observations: &[Observation],
    baseline: &BTreeMap<String, BaselineEntry>,
    pct: f64,
    min_delta_ms: u64,
) -> Option<Row> {
    let overlapping: Vec<&Observation> = observations
        .iter()
        .filter(|o| o.kind == "scenario" || o.kind == "case")
        .filter(|o| baseline.contains_key(&o.id))
        .collect();
    if overlapping.len() < 2 {
        return None;
    }
    let current: u64 = overlapping.iter().map(|o| o.ms).sum();
    let base: u64 = overlapping
        .iter()
        .map(|o| baseline.get(&o.id).map(|e| e.ms).unwrap_or(0))
        .sum();
    let budget_ms = overlapping
        .iter()
        .map(|o| baseline.get(&o.id).and_then(|e| e.budget_ms))
        .collect::<Option<Vec<u64>>>()
        .map(|v| v.into_iter().sum());
    let obs = Observation {
        id: "suite".to_string(),
        kind: "suite".to_string(),
        ms: current,
    };
    let entry = BaselineEntry {
        kind: "suite".to_string(),
        ms: base,
        budget_ms,
    };
    Some(row_for_observation(&obs, Some(&entry), pct, min_delta_ms))
}

fn label_of(id: &str, kind: &str) -> String {
    if kind == "suite" || id == "suite" {
        return "suite".to_string();
    }
    let rest = id.split_once(':').map(|(_, r)| r).unwrap_or(id);
    if kind == "build" {
        return format!("{rest} build");
    }
    if kind == "step" {
        if let Some((sid, step)) = rest.split_once('/') {
            return format!("{sid} / {step}");
        }
    }
    rest.to_string()
}

fn row_order(row: &Row) -> (u8, u8, String) {
    let status = match row.status {
        Status::Slow => 0,
        Status::Ok => 1,
        Status::New => 2,
        Status::Missing => 3,
    };
    let kind = match row.kind.as_str() {
        "suite" => 2,
        "step" => 1,
        _ => 0,
    };
    (status, kind, row.label.clone())
}

fn render(report: &Report, format: Format) -> String {
    match format {
        Format::Json => {
            serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}\n".to_string()) + "\n"
        }
        Format::Text => render_text(report),
        Format::Markdown => render_markdown(report),
    }
}

fn render_text(report: &Report) -> String {
    let mut lines = Vec::new();
    lines.push(heading_plain(report));
    lines.push(String::new());
    if report.rows.iter().all(|r| r.status == Status::Missing) && report.compared == 0 {
        lines.push("No passing durations in this run.".to_string());
    } else {
        lines.push(format!(
            "{:<28} {:>10} {:>10} {:>10}  {}",
            "name", "now", "baseline", "budget", "status"
        ));
        let (shown, hidden) = listed_rows(report);
        for row in shown {
            lines.push(format!(
                "{:<28} {:>10} {:>10} {:>10}  {}",
                trim_label(&row.label, 28),
                row.ms.map(fmt_ms).unwrap_or_else(|| "—".to_string()),
                row.baseline_ms
                    .map(fmt_ms)
                    .unwrap_or_else(|| "—".to_string()),
                row.budget_ms.map(fmt_ms).unwrap_or_else(|| "—".to_string()),
                status_word(row),
            ));
        }
        if hidden > 0 {
            lines.push(format!("{hidden} more within budget"));
        }
    }
    lines.push(String::new());
    lines.push(footer_plain(report));
    lines.push(String::new());
    lines.join("\n")
}

fn render_markdown(report: &Report) -> String {
    let mut lines = Vec::new();
    lines.push(format!("#### {}", heading_plain(report)));
    lines.push(String::new());
    lines.push(rule_sentence(report));
    lines.push(String::new());
    let (shown, hidden) = listed_rows(report);
    let visible: Vec<&Row> = shown
        .into_iter()
        .filter(|r| r.status != Status::Missing)
        .collect();
    if visible.is_empty() {
        lines.push("No passing durations in this run.".to_string());
        lines.push(String::new());
    } else {
        lines.push("| name | now | baseline | |".to_string());
        lines.push("| --- | ---: | ---: | --- |".to_string());
        for row in visible {
            let flag = match row.status {
                Status::Slow => format!("**slow** · {}", status_detail(row)),
                Status::New => "new".to_string(),
                Status::Ok => "ok".to_string(),
                Status::Missing => continue,
            };
            lines.push(format!(
                "| `{}` | {} | {} | {flag} |",
                row.label.replace('|', "\\|"),
                row.ms.map(fmt_ms).unwrap_or_else(|| "—".to_string()),
                row.baseline_ms
                    .map(fmt_ms)
                    .unwrap_or_else(|| "—".to_string()),
            ));
        }
        if hidden > 0 {
            lines.push(String::new());
            lines.push(format!("{hidden} more within budget."));
        }
        lines.push(String::new());
    }
    lines.push(format!("<sub>{}</sub>", footer_plain(report)));
    lines.push(String::new());
    lines.join("\n")
}

fn listed_rows(report: &Report) -> (Vec<&Row>, usize) {
    let mut slow = Vec::new();
    let mut healthy = Vec::new();
    for row in &report.rows {
        if row.status == Status::Missing {
            continue;
        }
        // Step rows are the detail under a scenario. Show them when they
        // slowed; a healthy click should not fill the comment.
        if row.kind == "step" && row.status != Status::Slow {
            continue;
        }
        if row.status == Status::Slow {
            slow.push(row);
        } else {
            healthy.push(row);
        }
    }
    let hidden = healthy.len().saturating_sub(MAX_LISTED_HEALTHY);
    healthy.truncate(MAX_LISTED_HEALTHY);
    slow.extend(healthy);
    (slow, hidden)
}

fn heading_plain(report: &Report) -> String {
    if !report.baseline_present {
        return "Timing — no baseline".to_string();
    }
    if report.compared == 0 && report.new == 0 {
        return "Timing — no passing runs".to_string();
    }
    if report.slow == 0 {
        "Timing — healthy".to_string()
    } else if report.slow == 1 {
        "Timing — 1 slowed".to_string()
    } else {
        format!("Timing — {} slowed", report.slow)
    }
}

fn rule_sentence(report: &Report) -> String {
    if !report.baseline_present {
        return "Nothing is compared until a baseline file is committed. `agent-qa timings accept` writes one from the latest passing runs.".to_string();
    }
    if report.compared == 0 && report.new == 0 {
        return "Durations are compared only for runs that passed. This run had none.".to_string();
    }
    let pct = report.pct;
    let floor = fmt_ms(report.min_delta_ms);
    let mut sentence = format!(
        "A row is slow when it is more than {pct:.0}% over its baseline and at least {floor} slower, or when it crosses that entry's budget."
    );
    if report.rows.iter().any(|r| r.kind == "build") {
        sentence.push_str(
            " A build is slow only when it crosses its budget; a warm cache and a cold compile are not a ratio.",
        );
    }
    sentence
}

fn footer_plain(report: &Report) -> String {
    format!(
        "{} compared, {} slowed, {} new, {} not in this run. Failed runs are left out.",
        report.compared, report.slow, report.new, report.missing
    )
}

fn status_word(row: &Row) -> String {
    match row.status {
        Status::Ok => "ok".to_string(),
        Status::New => "new".to_string(),
        Status::Missing => "absent".to_string(),
        Status::Slow => format!("SLOW {}", status_detail(row)),
    }
}

fn status_detail(row: &Row) -> String {
    match row.ratio {
        Some(ratio) => format!("{ratio:.1}×"),
        None => "over budget".to_string(),
    }
}

fn fmt_ms(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{:.1}m", ms as f64 / 60_000.0)
    }
}

fn trim_label(label: &str, width: usize) -> String {
    if label.chars().count() <= width {
        return label.to_string();
    }
    let mut out: String = label.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn limits_default(current: u64, base: u64, budget: Option<u64>) -> Status {
        judge(current, base, budget, DEFAULT_PCT, DEFAULT_MIN_DELTA_MS)
    }

    #[test]
    fn ten_seconds_to_two_minutes_is_slow_and_exact_double_is_not() {
        // 10s → 20s is exactly 2×, which stays inside the default line.
        assert_eq!(limits_default(20_000, 10_000, None), Status::Ok);
        // 10s → 21s is over 2× and more than 5s slower.
        assert_eq!(limits_default(21_000, 10_000, None), Status::Slow);
        // The example: 10s → 2min.
        assert_eq!(limits_default(120_000, 10_000, None), Status::Slow);
        // 10s → 14s is under the 5s absolute floor, so jitter stays quiet.
        assert_eq!(limits_default(14_000, 10_000, None), Status::Ok);
        // A 1s step that becomes 3s is under the floor.
        assert_eq!(limits_default(3_000, 1_000, None), Status::Ok);
        // A 1s step that becomes 8s is both over 2× and over the floor.
        assert_eq!(limits_default(8_000, 1_000, None), Status::Slow);
    }

    #[test]
    fn a_build_ignores_cache_swing_and_fails_only_past_its_budget() {
        let mut baseline = BTreeMap::new();
        baseline.insert(
            "build:cli".to_string(),
            BaselineEntry {
                kind: "build".to_string(),
                ms: 96_000,
                budget_ms: Some(300_000),
            },
        );
        baseline.insert(
            "case:login".to_string(),
            BaselineEntry {
                kind: "case".to_string(),
                ms: 10_000,
                budget_ms: Some(60_000),
            },
        );
        let under = build_report(
            &[
                Observation {
                    id: "build:cli".into(),
                    kind: "build".into(),
                    ms: 200_000,
                },
                Observation {
                    id: "case:login".into(),
                    kind: "case".into(),
                    ms: 10_000,
                },
            ],
            &baseline,
            true,
            DEFAULT_PCT,
            DEFAULT_MIN_DELTA_MS,
        );
        assert!(under.healthy);
        let build = under.rows.iter().find(|r| r.id == "build:cli").unwrap();
        assert_eq!(build.status, Status::Ok);
        assert_eq!(build.label, "cli build");
        let md = render_markdown(&under);
        assert!(md.contains("`cli build`"));
        assert!(md.contains("A build is slow only when it crosses its budget"));
        // The suite total is the case only, so one case does not invent a suite row.
        assert!(under.rows.iter().all(|r| r.kind != "suite"));

        let over = build_report(
            &[Observation {
                id: "build:cli".into(),
                kind: "build".into(),
                ms: 300_001,
            }],
            &baseline,
            true,
            DEFAULT_PCT,
            DEFAULT_MIN_DELTA_MS,
        );
        assert!(!over.healthy);
        assert_eq!(
            over.rows
                .iter()
                .find(|r| r.id == "build:cli")
                .unwrap()
                .status,
            Status::Slow
        );
    }

    #[test]
    fn budget_is_a_hard_ceiling_under_the_ratio() {
        // 10s baseline, 15s budget, 16s run: under 2×, over the ceiling.
        assert_eq!(limits_default(16_000, 10_000, Some(15_000)), Status::Slow);
        assert_eq!(limits_default(15_000, 10_000, Some(15_000)), Status::Ok);
    }

    #[test]
    fn report_flags_a_case_that_jumped_and_ignores_a_stable_neighbor() {
        let mut baseline = BTreeMap::new();
        baseline.insert(
            "case:checkout".to_string(),
            BaselineEntry {
                kind: "case".to_string(),
                ms: 10_000,
                budget_ms: Some(60_000),
            },
        );
        baseline.insert(
            "case:login".to_string(),
            BaselineEntry {
                kind: "case".to_string(),
                ms: 8_000,
                budget_ms: Some(60_000),
            },
        );
        let obs = vec![
            Observation {
                id: "case:checkout".into(),
                kind: "case".into(),
                ms: 120_000,
            },
            Observation {
                id: "case:login".into(),
                kind: "case".into(),
                ms: 8_400,
            },
            Observation {
                id: "case:new-flow".into(),
                kind: "case".into(),
                ms: 5_000,
            },
        ];
        let report = build_report(&obs, &baseline, true, DEFAULT_PCT, DEFAULT_MIN_DELTA_MS);
        assert!(!report.healthy);
        assert_eq!(report.slow, 2, "the case and the suite both jumped");
        let md = render_markdown(&report);
        assert!(md.contains("#### Timing — 2 slowed"));
        assert!(md.contains("`checkout`"));
        assert!(md.contains("**slow**"));
        assert!(md.contains("12.0×"));
        assert!(md.contains("`login`"));
        assert!(md.contains("| `login` | 8.4s | 8.0s | ok |"));
        assert!(md.contains("| `new-flow` | 5.0s | — | new |"));
        assert!(md.contains("`suite`"));
        // A failed case that is only in the baseline is counted, not listed
        // as a slowdown.
        assert_eq!(report.missing, 0);
    }

    #[test]
    fn missing_baseline_does_not_fail_and_says_so() {
        let obs = vec![Observation {
            id: "scenario:checkout".into(),
            kind: "scenario".into(),
            ms: 12_000,
        }];
        let report = build_report(
            &obs,
            &BTreeMap::new(),
            false,
            DEFAULT_PCT,
            DEFAULT_MIN_DELTA_MS,
        );
        assert!(report.healthy);
        assert_eq!(report.new, 1);
        let md = render_markdown(&report);
        assert!(md.contains("#### Timing — no baseline"));
        assert!(md.contains("timings accept"));
    }

    #[test]
    fn no_passing_runs_is_not_called_healthy() {
        let mut baseline = BTreeMap::new();
        baseline.insert(
            "case:checkout".into(),
            BaselineEntry {
                kind: "case".into(),
                ms: 10_000,
                budget_ms: None,
            },
        );
        let report = build_report(&[], &baseline, true, DEFAULT_PCT, DEFAULT_MIN_DELTA_MS);
        assert!(report.healthy, "absence is not a duration regression");
        assert_eq!(report.missing, 1);
        let md = render_markdown(&report);
        assert!(md.contains("#### Timing — no passing runs"));
        assert!(!md.contains("**slow**"));
    }

    #[test]
    fn healthy_steps_stay_out_of_the_comment_and_slow_steps_stay_in() {
        let mut baseline = BTreeMap::new();
        baseline.insert(
            "scenario:checkout".into(),
            BaselineEntry {
                kind: "scenario".into(),
                ms: 10_000,
                budget_ms: None,
            },
        );
        baseline.insert(
            "step:checkout/s1".into(),
            BaselineEntry {
                kind: "step".into(),
                ms: 800,
                budget_ms: None,
            },
        );
        baseline.insert(
            "step:checkout/s2".into(),
            BaselineEntry {
                kind: "step".into(),
                ms: 1_000,
                budget_ms: None,
            },
        );
        let obs = vec![
            Observation {
                id: "scenario:checkout".into(),
                kind: "scenario".into(),
                ms: 11_000,
            },
            Observation {
                id: "step:checkout/s1".into(),
                kind: "step".into(),
                ms: 900,
            },
            Observation {
                id: "step:checkout/s2".into(),
                kind: "step".into(),
                ms: 8_000,
            },
        ];
        let report = build_report(&obs, &baseline, true, DEFAULT_PCT, DEFAULT_MIN_DELTA_MS);
        assert!(!report.healthy);
        let md = render_markdown(&report);
        assert!(md.contains("checkout / s2"));
        assert!(!md.contains("checkout / s1"));
        assert!(md.contains("`checkout`"));
    }

    #[test]
    fn walk_uses_latest_passing_full_run_and_skips_reserved_dirs() {
        let root = scratch("walk");
        write_run(
            &root
                .join("checkout")
                .join("replays")
                .join("2026-01-01T00-00-00Z__aaaa"),
            "2026-01-01T00:00:00.000Z",
            "2026-01-01T00:00:10.000Z",
            0,
            None,
            "s1 400\ns2 1200\n",
        );
        // Newer, but it failed — the timeout must not become the duration.
        write_run(
            &root
                .join("checkout")
                .join("replays")
                .join("2026-01-02T00-00-00Z__bbbb"),
            "2026-01-02T00:00:00.000Z",
            "2026-01-02T00:02:00.000Z",
            1,
            None,
            "s1 120000\n",
        );
        // Newer still, but a partial window.
        write_run(
            &root
                .join("checkout")
                .join("replays")
                .join("2026-01-03T00-00-00Z__cccc"),
            "2026-01-03T00:00:00.000Z",
            "2026-01-03T00:00:04.000Z",
            0,
            Some("s2"),
            "s2 4000\n",
        );
        fs::create_dir_all(root.join("_timings")).unwrap();
        fs::write(root.join("_timings").join("scenario.json"), "{}").unwrap();
        fs::write(root.join("checkout").join("scenario.json"), "{}").unwrap();

        let obs = collect_scenarios(&root, true, 500);
        assert_eq!(obs.len(), 2, "scenario + the step over the floor: {obs:?}");
        assert_eq!(obs[0].id, "scenario:checkout");
        assert_eq!(obs[0].ms, 10_000);
        assert_eq!(obs[1].id, "step:checkout/s2");
        assert_eq!(obs[1].ms, 1_200);
    }

    #[test]
    fn accept_keeps_budget_and_unseen_entries_until_prune() {
        let root = scratch("accept");
        let baseline = root.join("_timings").join("baseline.json");
        fs::create_dir_all(baseline.parent().unwrap()).unwrap();
        fs::write(
            &baseline,
            r#"{
  "schema": "agent-qa-timings/v1",
  "entries": {
    "case:kept": { "kind": "case", "ms": 1000, "budgetMs": 60000 },
    "case:gone": { "kind": "case", "ms": 2000, "budgetMs": 60000 }
  }
}
"#,
        )
        .unwrap();
        let samples = root.join("samples.json");
        fs::write(
            &samples,
            r#"{
  "schema": "agent-qa-timing-samples/v1",
  "entries": [ { "id": "case:kept", "kind": "case", "ms": 1500 } ]
}
"#,
        )
        .unwrap();
        let opts = Opts {
            baseline: Some(baseline.clone()),
            samples: Some(samples.clone()),
            root: Some(root.clone()),
            pct: DEFAULT_PCT,
            min_delta_ms: DEFAULT_MIN_DELTA_MS,
            steps: true,
            step_floor_ms: 500,
            format: Format::Text,
            out: None,
            report_only: false,
            prune: false,
        };
        assert_eq!(cmd_accept(&opts).unwrap(), 0);
        let (entries, present) = load_baseline(&baseline).unwrap();
        assert!(present);
        assert_eq!(entries["case:kept"].ms, 1_500);
        assert_eq!(entries["case:kept"].budget_ms, Some(60_000));
        assert!(entries.contains_key("case:gone"));

        let mut pruning = opts;
        pruning.prune = true;
        assert_eq!(cmd_accept(&pruning).unwrap(), 0);
        let (entries, _) = load_baseline(&baseline).unwrap();
        assert!(!entries.contains_key("case:gone"));
        assert_eq!(entries["case:kept"].budget_ms, Some(60_000));
    }

    #[test]
    fn check_exits_1_on_slow_and_0_when_reporting() {
        let root = scratch("check");
        let baseline = root.join("baseline.json");
        let samples = root.join("samples.json");
        fs::write(
            &baseline,
            r#"{ "schema": "agent-qa-timings/v1", "entries": {
              "case:checkout": { "kind": "case", "ms": 10000, "budgetMs": 60000 }
            } }"#,
        )
        .unwrap();
        fs::write(
            &samples,
            r#"{ "schema": "agent-qa-timing-samples/v1", "entries": [
              { "id": "case:checkout", "kind": "case", "ms": 120000 }
            ] }"#,
        )
        .unwrap();
        let out = root.join("out.md");
        let opts = Opts {
            baseline: Some(baseline),
            samples: Some(samples),
            root: Some(root),
            pct: DEFAULT_PCT,
            min_delta_ms: DEFAULT_MIN_DELTA_MS,
            steps: false,
            step_floor_ms: 500,
            format: Format::Markdown,
            out: Some(out.clone()),
            report_only: false,
            prune: false,
        };
        assert_eq!(cmd_check(&opts).unwrap(), 1);
        let body = fs::read_to_string(&out).unwrap();
        assert!(body.contains("**slow**"));
        assert!(body.contains("2.0m"));
        let mut reporting = opts;
        reporting.report_only = true;
        assert_eq!(cmd_check(&reporting).unwrap(), 0);
    }

    fn scratch(name: &str) -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("aq-timings-{name}-{n}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_run(
        run: &Path,
        started: &str,
        finished: &str,
        exit_code: i32,
        window_from: Option<&str>,
        steps: &str,
    ) {
        fs::create_dir_all(run).unwrap();
        let window = match window_from {
            Some(step) => format!(",\"windowFrom\":\"{step}\""),
            None => String::new(),
        };
        fs::write(
            run.join("audit.json"),
            format!(
                "{{\"schema\":\"scenario-replay-audit/v1\",\"runId\":\"r\",\"scenarioId\":\"s\",\"startedAt\":\"{started}\",\"finishedAt\":\"{finished}\",\"exitCode\":{exit_code},\"scenarioContentHash\":\"abc\"{window}}}"
            ),
        )
        .unwrap();
        let mut events = String::new();
        for line in steps.lines() {
            let mut parts = line.split_whitespace();
            let id = parts.next().unwrap();
            let ms: u64 = parts.next().unwrap().parse().unwrap();
            events.push_str(&format!(
                "{{\"id\":\"{id}\",\"status\":\"pass\",\"ms\":{ms}}}\n"
            ));
        }
        fs::write(run.join("events.jsonl"), events).unwrap();
    }
}
