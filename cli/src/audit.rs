//! `audit show <sid> [<runId | latest>]` — pretty-print one replay's audit.json.
//!
//! Useful when investigating a single failure without reaching for a JSON
//! viewer; complements `list <sid>` which only shows summary rows.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::paths;
use crate::sidecar::StepEvent;

pub fn run(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    let mut github_format = false;
    let mut positionals: Vec<String> = Vec::new();
    let mut filter_tag: Option<String> = None;
    let mut filter_profile: Option<String> = None;
    let mut only_failed = false;
    let mut only_passed = false;
    let mut limit: Option<usize> = None;
    let mut slow_min_secs: Option<f64> = None;
    let mut sort_by_duration_desc = false;
    let mut sort_runid_desc = false;
    let mut since_ms: Option<u64> = None;
    let mut trend_all = false;
    let mut until_ms: Option<u64> = None;
    let mut min_flips: usize = 2;
    let mut min_runs: usize = 3;
    let mut slow_pct: f64 = 50.0;
    let mut slow_min_ms: u64 = 250;
    let mut recent_n: usize = 2;
    let mut cluster_min: usize = 2;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            "--json" => json_out = true,
            "--all" => trend_all = true,
            "--format" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--format requires a value"))?;
                match v.as_str() {
                    "text" => {}
                    "json" => json_out = true,
                    "github" => github_format = true,
                    other => bail!("--format expects 'text', 'json', or 'github', got {other:?}"),
                }
            }
            s if s.starts_with("--format=") => {
                let v = &s["--format=".len()..];
                match v {
                    "text" => {}
                    "json" => json_out = true,
                    "github" => github_format = true,
                    other => bail!("--format expects 'text', 'json', or 'github', got {other:?}"),
                }
            }
            "--failed" => only_failed = true,
            "--passed" => only_passed = true,
            "--tag" => filter_tag = it.next().cloned(),
            s if s.starts_with("--tag=") => filter_tag = Some(s["--tag=".len()..].to_string()),
            "--profile" => filter_profile = it.next().cloned(),
            s if s.starts_with("--profile=") => {
                filter_profile = Some(s["--profile=".len()..].to_string())
            }
            "--limit" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--limit requires a positive integer"))?;
                let n: usize = v
                    .parse()
                    .map_err(|_| anyhow!("--limit expects a positive integer, got {v:?}"))?;
                if n == 0 {
                    bail!("--limit must be > 0");
                }
                limit = Some(n);
            }
            s if s.starts_with("--limit=") => {
                let v = &s["--limit=".len()..];
                let n: usize = v
                    .parse()
                    .map_err(|_| anyhow!("--limit expects a positive integer, got {v:?}"))?;
                if n == 0 {
                    bail!("--limit must be > 0");
                }
                limit = Some(n);
            }
            "--slow" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--slow requires a positive number of seconds"))?;
                let n: f64 = v
                    .parse()
                    .map_err(|_| anyhow!("--slow expects a number, got {v:?}"))?;
                if n <= 0.0 {
                    bail!("--slow must be > 0");
                }
                slow_min_secs = Some(n);
            }
            s if s.starts_with("--slow=") => {
                let v = &s["--slow=".len()..];
                let n: f64 = v
                    .parse()
                    .map_err(|_| anyhow!("--slow expects a number, got {v:?}"))?;
                if n <= 0.0 {
                    bail!("--slow must be > 0");
                }
                slow_min_secs = Some(n);
            }
            "--sort" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--sort requires a value"))?;
                match v.as_str() {
                    "duration" => sort_by_duration_desc = true,
                    "runId-desc" => sort_runid_desc = true,
                    other => bail!("--sort: expected 'duration' or 'runId-desc', got {other:?}"),
                }
            }
            s if s.starts_with("--sort=") => {
                let v = &s["--sort=".len()..];
                match v {
                    "duration" => sort_by_duration_desc = true,
                    "runId-desc" => sort_runid_desc = true,
                    other => bail!("--sort: expected 'duration' or 'runId-desc', got {other:?}"),
                }
            }
            "--since" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--since requires an ISO-8601 timestamp"))?;
                since_ms = Some(crate::time::parse_iso_ms(v.as_str())?);
            }
            s if s.starts_with("--since=") => {
                let v = &s["--since=".len()..];
                since_ms = Some(crate::time::parse_iso_ms(v)?);
            }
            "--until" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--until requires an ISO-8601 timestamp"))?;
                until_ms = Some(crate::time::parse_iso_ms(v.as_str())?);
            }
            s if s.starts_with("--until=") => {
                let v = &s["--until=".len()..];
                until_ms = Some(crate::time::parse_iso_ms(v)?);
            }
            "--min-flips" => {
                min_flips = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-flips expects a positive integer"))?;
            }
            s if s.starts_with("--min-flips=") => {
                min_flips = s["--min-flips=".len()..]
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-flips expects a positive integer"))?;
            }
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
            "--pct" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--pct requires a number"))?;
                slow_pct = v
                    .parse()
                    .map_err(|_| anyhow!("--pct expects a number, got {v:?}"))?;
                if slow_pct <= 0.0 {
                    bail!("--pct must be > 0");
                }
            }
            s if s.starts_with("--pct=") => {
                slow_pct = s["--pct=".len()..]
                    .parse()
                    .map_err(|_| anyhow!("--pct expects a number"))?;
                if slow_pct <= 0.0 {
                    bail!("--pct must be > 0");
                }
            }
            "--min-ms" => {
                slow_min_ms = it
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-ms expects a positive integer"))?;
            }
            s if s.starts_with("--min-ms=") => {
                slow_min_ms = s["--min-ms=".len()..]
                    .parse::<u64>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-ms expects a positive integer"))?;
            }
            "--recent" => {
                recent_n = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--recent expects a positive integer"))?;
            }
            "--min-size" => {
                cluster_min = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-size expects a positive integer"))?;
            }
            s if s.starts_with("--min-size=") => {
                cluster_min = s["--min-size=".len()..]
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--min-size expects a positive integer"))?;
            }
            s if s.starts_with("--recent=") => {
                recent_n = s["--recent=".len()..]
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow!("--recent expects a positive integer"))?;
            }
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => positionals.push(other.to_string()),
        }
    }
    if only_passed && only_failed {
        bail!("--passed and --failed are mutually exclusive");
    }
    let sub = positionals
        .first()
        .ok_or_else(|| anyhow!("usage: audit <show|list|stats|stats-all> <sid> [...]"))?
        .clone();
    let filters = ListFilters {
        tag: filter_tag,
        profile: filter_profile,
        only_failed,
        only_passed,
        limit,
        slow_min_secs,
        sort_by_duration_desc,
        sort_runid_desc,
        since_ms,
        until_ms,
    };
    match sub.as_str() {
        "show" => show(&positionals, json_out, github_format),
        "list" => list(&positionals, json_out, github_format, &filters),
        "stats" => stats(&positionals, json_out, since_ms, until_ms),
        "stats-all" => stats_all(json_out, since_ms, until_ms),
        "diff" => diff(&positionals),
        "summary" => summary(&positionals),
        "exit-code" => exit_code(&positionals),
        "field" => field(&positionals),
        "count" => count(&positionals),
        "duration" => duration(&positionals),
        "flaky" => flaky(&positionals, json_out, min_flips, min_runs),
        "slow" => slow(&positionals, json_out, slow_pct, slow_min_ms, recent_n, min_runs),
        "health" => health(json_out),
        "trend" => trend(&positionals, json_out, limit, trend_all),

        "cluster" => cluster(json_out, cluster_min),
        "explain" => explain(&positionals, json_out),
        "verdict" => {
            if trend_all {
                verdict_all(json_out)
            } else {
                verdict(&positionals, json_out)
            }
        }

        other => bail!(
            "unknown audit subverb {other:?} (try: show | list | stats | stats-all | diff | summary | exit-code | field | count | duration | flaky | slow | health | cluster | verdict | trend)"

        ),
    }
}

#[derive(Default, Debug, Clone)]
struct ListFilters {
    tag: Option<String>,
    profile: Option<String>,
    only_failed: bool,
    only_passed: bool,
    limit: Option<usize>,
    slow_min_secs: Option<f64>,
    sort_by_duration_desc: bool,
    sort_runid_desc: bool,
    since_ms: Option<u64>,
    until_ms: Option<u64>,
}

fn show(positionals: &[String], json_out: bool, github_format: bool) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit show <sid> [<runId | latest>]"))?;
    let run_ref = positionals.get(2).map(|s| s.as_str()).unwrap_or("latest");
    let dir = paths::scenario_dir(sid)?;
    let run_id = resolve_run_id(&dir, run_ref)?;
    let audit_path = dir.join("replays").join(&run_id).join("audit.json");
    if !audit_path.is_file() {
        bail!("audit show: no audit.json at {}", audit_path.display());
    }
    let bytes = fs::read(&audit_path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if json_out {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(0);
    }
    if github_format {
        // GH annotation only if the run failed; otherwise stay silent
        // (success isn't an annotation-worthy event).
        let exit = value.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
        if exit != 0 {
            let summary = value
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("failed");
            println!("::error file={sid},title=audit/{run_id}::{summary}");
        }
        return Ok(if exit == 0 { 0 } else { 1 });
    }
    render_text(&audit_path, &value);
    Ok(0)
}

fn list(
    positionals: &[String],
    json_out: bool,
    github_format: bool,
    filters: &ListFilters,
) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit list <sid>"))?;
    let dir = paths::scenario_dir(sid)?;
    let replays_dir = dir.join("replays");
    let runs = crate::paths::run_dirs(&replays_dir);

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        run_id: String,
        summary: Option<String>,
        exit_code: Option<i64>,
        profile: Option<String>,
        tag: Option<String>,
        started_at: Option<String>,
        duration_secs: Option<f64>,
    }
    let mut rows: Vec<Row> = Vec::with_capacity(runs.len());
    for run in &runs {
        let audit_path = run.join("audit.json");
        let audit: Option<Value> = fs::read(&audit_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        rows.push(Row {
            run_id: run
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            summary: audit
                .as_ref()
                .and_then(|a| a.get("summary")?.as_str().map(str::to_string)),
            exit_code: audit.as_ref().and_then(|a| a.get("exitCode")?.as_i64()),
            profile: audit
                .as_ref()
                .and_then(|a| a.get("profile")?.as_str().map(str::to_string)),
            tag: audit
                .as_ref()
                .and_then(|a| a.get("tag")?.as_str().map(str::to_string)),
            started_at: audit
                .as_ref()
                .and_then(|a| a.get("startedAt")?.as_str().map(str::to_string)),
            duration_secs: {
                let s = audit.as_ref().and_then(|a| a.get("startedAt")?.as_str());
                let f = audit.as_ref().and_then(|a| a.get("finishedAt")?.as_str());
                match (s, f) {
                    (Some(s), Some(f)) => match (parse_iso_ms(s), parse_iso_ms(f)) {
                        (Ok(sm), Ok(fm)) => Some((fm.saturating_sub(sm)) as f64 / 1000.0),
                        _ => None,
                    },
                    _ => None,
                }
            },
        });
    }
    if let Some(t) = filters.tag.as_deref() {
        let t = t.to_ascii_lowercase();
        rows.retain(|r| {
            r.tag
                .as_deref()
                .map(|x| x.to_ascii_lowercase().contains(&t))
                .unwrap_or(false)
        });
    }
    if let Some(p) = filters.profile.as_deref() {
        let p = p.to_ascii_lowercase();
        rows.retain(|r| {
            r.profile
                .as_deref()
                .map(|x| x.to_ascii_lowercase().contains(&p))
                .unwrap_or(false)
        });
    }
    if filters.only_failed {
        rows.retain(|r| r.exit_code.is_some_and(|c| c != 0));
    }
    if filters.only_passed {
        rows.retain(|r| r.exit_code == Some(0));
    }
    if let Some(min) = filters.slow_min_secs {
        rows.retain(|r| r.duration_secs.is_some_and(|d| d >= min));
    }
    if let Some(threshold_ms) = filters.since_ms {
        rows.retain(|r| {
            r.started_at
                .as_deref()
                .and_then(|s| crate::time::parse_iso_ms(s).ok())
                .is_some_and(|started| started >= threshold_ms)
        });
    }
    if let Some(threshold_ms) = filters.until_ms {
        rows.retain(|r| {
            r.started_at
                .as_deref()
                .and_then(|s| crate::time::parse_iso_ms(s).ok())
                .is_some_and(|started| started <= threshold_ms)
        });
    }
    if filters.sort_by_duration_desc {
        rows.sort_by(|a, b| {
            b.duration_secs
                .unwrap_or(0.0)
                .partial_cmp(&a.duration_secs.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    if filters.sort_runid_desc {
        rows.sort_by(|a, b| b.run_id.cmp(&a.run_id));
    }
    // Limit is applied LAST, tail-keep (most recent N) since rows are
    // chronologically lex-sorted by run_id.
    if let Some(n) = filters.limit {
        if rows.len() > n {
            let drop = rows.len() - n;
            rows.drain(0..drop);
        }
    }
    if json_out {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(0);
    }
    if github_format {
        // One ::error annotation per failed run; sid + runId in title.
        let sid = positionals.get(1).map(String::as_str).unwrap_or("?");
        for row in &rows {
            if row.exit_code.is_some_and(|c| c != 0) {
                let run = &row.run_id;
                let summary = row.summary.as_deref().unwrap_or("failed");
                println!("::error file={sid},title=audit/{run}::{summary}");
            }
        }
        return Ok(0);
    }
    println!(
        "audit list: {} ({} run(s))",
        replays_dir.display(),
        rows.len()
    );
    if rows.is_empty() {
        return Ok(0);
    }
    let run_h = "run";
    let summary_h = "summary";
    let exit_h = "exit";
    let profile_h = "profile";
    let tag_h = "tag";
    let dur_h = "dur(s)";
    println!("{run_h:<32}  {summary_h:<22}  {exit_h:>4}  {profile_h:<10}  {dur_h:>7}  {tag_h}");
    for row in &rows {
        let run_id = &row.run_id;
        let summary = row.summary.as_deref().unwrap_or("-");
        let exit = row
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into());
        let profile = row.profile.as_deref().unwrap_or("-");
        let dur = row
            .duration_secs
            .map(|d| format!("{d:.2}"))
            .unwrap_or_else(|| "-".into());
        let tag = row.tag.as_deref().unwrap_or("-");
        println!("{run_id:<32}  {summary:<22}  {exit:>4}  {profile:<10}  {dur:>7}  {tag}");
    }
    Ok(0)
}

/// Scenario-level flake score, 0–100 (100 = rock stable). Three weighted
/// signals over the scenario's run history:
///   - outcome churn: adjacent pass<->fail flips across scored runs (x0.5)
///   - fail rate:     failing runs / scored runs (x0.3)
///   - heal rate:     runs with auto-heals / all runs (x0.2)
/// None with fewer than 2 scored runs — one observation cannot flake.
fn flake_score(outcomes: &[bool], healed_runs: usize, total_runs: usize) -> Option<f64> {
    if outcomes.len() < 2 {
        return None;
    }
    let flips = outcomes.windows(2).filter(|w| w[0] != w[1]).count() as f64;
    let flip_rate = flips / (outcomes.len() - 1) as f64;
    let fail_rate = outcomes.iter().filter(|o| !**o).count() as f64 / outcomes.len() as f64;
    let heal_rate = healed_runs as f64 / total_runs.max(1) as f64;
    let s = 100.0 * (1.0 - (0.5 * flip_rate + 0.3 * fail_rate + 0.2 * heal_rate));
    Some(s.clamp(0.0, 100.0))
}

fn stats(
    positionals: &[String],
    json_out: bool,
    since_ms: Option<u64>,
    until_ms: Option<u64>,
) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit stats <sid>"))?;
    let dir = paths::scenario_dir(sid)?;
    let replays_dir = dir.join("replays");
    let runs = crate::paths::run_dirs(&replays_dir);

    let mut total = 0u32;
    let mut passes = 0u32;
    let mut failures = 0u32;
    let mut unknown = 0u32;
    let mut last_pass: Option<String> = None;
    let mut last_fail: Option<String> = None;
    let mut duration_ms_total: u64 = 0;
    let mut duration_ms_count: u64 = 0;
    let mut tag_counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let mut profile_counts: std::collections::BTreeMap<String, u32> =
        std::collections::BTreeMap::new();
    let mut outcomes: Vec<bool> = Vec::new();
    let mut healed_runs = 0usize;

    for run in &runs {
        let run_id = run
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let audit: Option<Value> = fs::read(run.join("audit.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        // Date range filter: skip runs whose startedAt is outside
        // [since, until]. Runs missing startedAt are kept (consistent
        // with how 'unknown' classification works).
        if since_ms.is_some() || until_ms.is_some() {
            if let Some(s) = audit.as_ref().and_then(|a| a.get("startedAt")?.as_str()) {
                if let Ok(started) = crate::time::parse_iso_ms(s) {
                    if let Some(min) = since_ms {
                        if started < min {
                            continue;
                        }
                    }
                    if let Some(max) = until_ms {
                        if started > max {
                            continue;
                        }
                    }
                }
            }
        }
        total += 1;
        match audit.as_ref().and_then(|a| a.get("exitCode")?.as_i64()) {
            Some(0) => {
                passes += 1;
                outcomes.push(true);
                last_pass = Some(run_id.clone());
            }
            Some(_) => {
                failures += 1;
                outcomes.push(false);
                last_fail = Some(run_id.clone());
            }
            None => unknown += 1,
        }
        if audit
            .as_ref()
            .and_then(|a| a.get("autoHealed")?.as_array())
            .is_some_and(|v| !v.is_empty())
        {
            healed_runs += 1;
        }
        if let Some(tag) = audit.as_ref().and_then(|a| a.get("tag")?.as_str()) {
            *tag_counts.entry(tag.to_string()).or_insert(0) += 1;
        }
        if let Some(profile) = audit.as_ref().and_then(|a| a.get("profile")?.as_str()) {
            *profile_counts.entry(profile.to_string()).or_insert(0) += 1;
        }
        // duration: only count runs with both startedAt + finishedAt.
        if let (Some(s), Some(f)) = (
            audit.as_ref().and_then(|a| a.get("startedAt")?.as_str()),
            audit.as_ref().and_then(|a| a.get("finishedAt")?.as_str()),
        ) {
            if let (Ok(sm), Ok(fm)) = (parse_iso_ms(s), parse_iso_ms(f)) {
                duration_ms_total = duration_ms_total.saturating_add(fm.saturating_sub(sm));
                duration_ms_count += 1;
            }
        }
    }

    let pass_rate = if passes + failures == 0 {
        0.0
    } else {
        passes as f64 / (passes + failures) as f64
    };
    let avg_duration_secs = if duration_ms_count == 0 {
        0.0
    } else {
        (duration_ms_total as f64 / duration_ms_count as f64) / 1000.0
    };

    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Report {
            sid: String,
            replays_dir: String,
            total: u32,
            passes: u32,
            failures: u32,
            unknown: u32,
            pass_rate: f64,
            avg_duration_secs: f64,
            last_pass: Option<String>,
            last_fail: Option<String>,
            tag_counts: std::collections::BTreeMap<String, u32>,
            profile_counts: std::collections::BTreeMap<String, u32>,
            flake_score: Option<f64>,
            healed_runs: usize,
        }
        let report = Report {
            sid: sid.clone(),
            replays_dir: replays_dir.display().to_string(),
            total,
            passes,
            failures,
            unknown,
            pass_rate,
            avg_duration_secs,
            last_pass,
            last_fail,
            tag_counts,
            profile_counts,
            flake_score: flake_score(&outcomes, healed_runs, total as usize),
            healed_runs,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(0);
    }

    println!("audit stats: {} ({total} run(s))", replays_dir.display());
    println!("  passes        : {passes}");
    println!("  failures      : {failures}");
    println!("  unknown       : {unknown}");
    println!("  pass rate     : {:.0}%", pass_rate * 100.0);
    match flake_score(&outcomes, healed_runs, total as usize) {
        Some(s) => println!("  flake score   : {:.0}/100", s),
        None => println!("  flake score   : n/a (<2 scored runs)"),
    }
    if duration_ms_count > 0 {
        println!("  avg duration  : {avg_duration_secs:.3}s");
    }
    if let Some(lp) = &last_pass {
        println!("  last pass     : {lp}");
    }
    if let Some(lf) = &last_fail {
        println!("  last fail     : {lf}");
    }
    if !tag_counts.is_empty() {
        println!("  tags          :");
        for (k, v) in &tag_counts {
            println!("    {k}: {v}");
        }
    }
    if !profile_counts.is_empty() {
        println!("  profiles      :");
        for (k, v) in &profile_counts {
            println!("    {k}: {v}");
        }
    }
    Ok(0)
}

fn duration(positionals: &[String]) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit duration <sid> [<runId | latest>]"))?;
    let run_ref = positionals.get(2).map(|s| s.as_str()).unwrap_or("latest");
    let dir = paths::scenario_dir(sid)?;
    let run_id = resolve_run_id(&dir, run_ref)?;
    let audit_path = dir.join("replays").join(&run_id).join("audit.json");
    if !audit_path.is_file() {
        bail!("audit duration: no audit.json at {}", audit_path.display());
    }
    let bytes = fs::read(&audit_path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let started = value
        .get("startedAt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("audit duration: startedAt missing or not a string"))?;
    let finished = value
        .get("finishedAt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow!(
                "audit duration: finishedAt missing or not a string (run may still be in flight)"
            )
        })?;
    // ISO-8601 with millisecond precision; chrono not in scope. Parse manually.
    let s = parse_iso_ms(started)?;
    let f = parse_iso_ms(finished)?;
    let dur_ms = f.saturating_sub(s);
    let dur_secs = dur_ms as f64 / 1000.0;
    println!("{dur_secs:.3}");
    Ok(0)
}

/// Parse an ISO-8601 timestamp like 2026-01-01T12:34:56.789Z into
/// milliseconds since the Unix epoch. Returns an error for any other
/// format. Pure-stdlib; doesn't pull in chrono.
use crate::time::parse_iso_ms;

fn count(positionals: &[String]) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit count <sid>"))?;
    let dir = paths::scenario_dir(sid)?;
    let replays_dir = dir.join("replays");
    let n = crate::paths::run_dirs(&replays_dir).len();
    println!("{n}");
    Ok(0)
}

fn field(positionals: &[String]) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit field <sid> <runId | latest> <fieldName>"))?;
    let run_ref = positionals
        .get(2)
        .ok_or_else(|| anyhow!("usage: audit field <sid> <runId | latest> <fieldName>"))?;
    let name = positionals
        .get(3)
        .ok_or_else(|| anyhow!("usage: audit field <sid> <runId | latest> <fieldName>"))?;
    let dir = paths::scenario_dir(sid)?;
    let run_id = resolve_run_id(&dir, run_ref)?;
    let audit_path = dir.join("replays").join(&run_id).join("audit.json");
    if !audit_path.is_file() {
        bail!("audit field: no audit.json at {}", audit_path.display());
    }
    let bytes = fs::read(&audit_path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let v = value.get(name).ok_or_else(|| {
        anyhow!(
            "audit field: {name:?} not present in {}",
            audit_path.display()
        )
    })?;
    match v {
        Value::String(s) => println!("{s}"),
        Value::Number(n) => println!("{n}"),
        Value::Bool(b) => println!("{b}"),
        Value::Null => println!(),
        // For objects/arrays, fall back to compact JSON so the verb stays
        // useful (e.g. for parameters[] / healOverridesApplied[]).
        other => println!("{}", serde_json::to_string(other)?),
    }
    Ok(0)
}

fn exit_code(positionals: &[String]) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit exit-code <sid> [<runId | latest>]"))?;
    let run_ref = positionals.get(2).map(|s| s.as_str()).unwrap_or("latest");
    let dir = paths::scenario_dir(sid)?;
    let run_id = resolve_run_id(&dir, run_ref)?;
    let audit_path = dir.join("replays").join(&run_id).join("audit.json");
    if !audit_path.is_file() {
        bail!("audit exit-code: no audit.json at {}", audit_path.display());
    }
    let bytes = fs::read(&audit_path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let exit = value.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
    println!("{exit}");
    Ok(0)
}

/// `audit verdict <sid> <runId|latest>` — the run-level triage signal a
/// CI gate or a human can act on without reading the audit tree:
///
///   PASS  (exit 0): the run was green and needed no self-correction.
///   FIX   (exit 2): the run was green BUT the runner had to self-correct
///                   — auto-healed steps or value-rejection evidence mean
///                   drift is accumulating; review heal.jsonl and promote.
///   BLOCK (exit 1): the run failed — a human decides product-bug vs
///                   scenario rot.
///
/// Reads `audit.json` (exitCode, autoHealed) + `heal.jsonl` rows (mode +
/// stepId) for that run only.
fn verdict(positionals: &[String], json_out: bool) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit verdict <sid> [<runId | latest>]"))?;
    let run_ref = positionals.get(2).map(|s| s.as_str()).unwrap_or("latest");
    let dir = paths::scenario_dir(sid)?;
    let run_id = resolve_run_id(&dir, run_ref)?;
    let run_dir = dir.join("replays").join(&run_id);
    let audit_path = run_dir.join("audit.json");
    if !audit_path.is_file() {
        bail!("audit verdict: no audit.json at {}", audit_path.display());
    }
    let (name, code, healed_steps, rejection_steps, summary_line, exit) = verdict_for(&run_dir)?;

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "scenario": sid,
                "runId": run_id,
                "verdict": name,
                "exitCode": exit,
                "healedSteps": healed_steps,
                "valueRejectionSteps": rejection_steps,
                "summary": summary_line,
            }))?
        );
    } else {
        match name {
            "PASS" => println!("PASS {sid} {run_id} — {summary_line}"),
            "FIX" => println!(
                "FIX {sid} {run_id} — green but self-corrected (healed: [{}], value-rejections: [{}]); review + promote",
                healed_steps.join(", "),
                rejection_steps.join(", "),
            ),
            _ => println!("BLOCK {sid} {run_id} — {summary_line}"),
        }
    }
    Ok(code)
}

/// Verdict computation shared by `audit verdict <sid>` and
/// `audit verdict --all`. Returns (name, exit-code, healed step ids,
/// value-rejection step ids, audit summary line).
type VerdictInfo = (&'static str, u8, Vec<String>, Vec<String>, String, i64);

fn verdict_for(run_dir: &std::path::Path) -> Result<VerdictInfo> {
    let bytes = fs::read(run_dir.join("audit.json"))?;
    let audit: Value = serde_json::from_slice(&bytes)?;
    let exit = audit.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
    let summary_line = audit
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut healed_steps: Vec<String> = audit
        .get("autoHealed")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // heal.jsonl rows carry the evidence classes that don't surface in
    // audit.autoHealed (value rejections land there, not as locator heals).
    let mut rejection_steps: Vec<String> = Vec::new();
    let heal_path = run_dir.join("heal.jsonl");
    if let Ok(body) = fs::read_to_string(&heal_path) {
        for line in body.lines() {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let step = row
                .get("stepId")
                .or_else(|| row.get("step_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let mode = row.get("mode").and_then(|v| v.as_str()).unwrap_or("");
            if mode == "value-rejection" && !rejection_steps.iter().any(|s| s == step) {
                rejection_steps.push(step.to_string());
            }
            if mode == "locator-correction" && !healed_steps.iter().any(|s| s == step) {
                healed_steps.push(step.to_string());
            }
        }
    }
    healed_steps.sort();
    rejection_steps.sort();

    let (name, code) = if exit != 0 {
        ("BLOCK", 1u8)
    } else if !healed_steps.is_empty() || !rejection_steps.is_empty() {
        ("FIX", 2u8)
    } else {
        ("PASS", 0u8)
    };
    Ok((
        name,
        code,
        healed_steps,
        rejection_steps,
        summary_line,
        exit,
    ))
}

/// `audit verdict --all`: one verdict row per scenario's latest run —
/// the suite triage board. Exit 1 if any scenario is BLOCK, else 0
/// (FIX rows still exit 0 — they're warnings, not failures).
fn verdict_all(json_out: bool) -> Result<u8> {
    let root = paths::scenarios_root();
    let mut sids: Vec<String> = fs::read_dir(&root)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| root.join(n).join("scenario.json").is_file())
        .collect();
    sids.sort();

    let mut rows: Vec<Value> = Vec::new();
    let mut any_block = false;
    for sid in &sids {
        let dir = paths::scenario_dir(sid)?;
        let run_id = match resolve_run_id(&dir, "latest") {
            Ok(r) => r,
            Err(_) => {
                rows.push(serde_json::json!({
                    "scenario": sid, "verdict": "NO RUNS",
                }));
                continue;
            }
        };
        let run_dir = dir.join("replays").join(&run_id);
        match verdict_for(&run_dir) {
            Ok((name, _, healed, rejected, _summary, _exit)) => {
                if name == "BLOCK" {
                    any_block = true;
                }
                rows.push(serde_json::json!({
                    "scenario": sid,
                    "runId": run_id,
                    "verdict": name,
                    "healedSteps": healed,
                    "valueRejectionSteps": rejected,
                }));
            }
            Err(e) => {
                rows.push(serde_json::json!({
                    "scenario": sid, "runId": run_id,
                    "verdict": "BLOCK", "error": e.to_string(),
                }));
                any_block = true;
            }
        }
    }

    if json_out {
        let blocked = rows
            .iter()
            .filter(|r| r.get("verdict").and_then(|v| v.as_str()) == Some("BLOCK"))
            .count();
        let fixed = rows
            .iter()
            .filter(|r| r.get("verdict").and_then(|v| v.as_str()) == Some("FIX"))
            .count();
        let passed = rows
            .iter()
            .filter(|r| r.get("verdict").and_then(|v| v.as_str()) == Some("PASS"))
            .count();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "scenarios": rows,
                "roll": { "pass": passed, "fix": fixed, "block": blocked },
            }))?
        );
    } else {
        println!("{:<28} {:<7} run", "scenario", "verdict");
        for r in &rows {
            println!(
                "{:<28} {:<7} {}",
                r["scenario"].as_str().unwrap_or(""),
                r["verdict"].as_str().unwrap_or(""),
                r["runId"].as_str().unwrap_or("—")
            );
        }
        let blocked = rows.iter().filter(|r| r["verdict"] == "BLOCK").count();
        let fixed = rows.iter().filter(|r| r["verdict"] == "FIX").count();
        let passed = rows.iter().filter(|r| r["verdict"] == "PASS").count();
        println!("{passed} PASS, {fixed} FIX, {blocked} BLOCK");
    }
    Ok(if any_block { 1 } else { 0 })
}

fn summary(positionals: &[String]) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit summary <sid> [<runId | latest>]"))?;
    let run_ref = positionals.get(2).map(|s| s.as_str()).unwrap_or("latest");
    let dir = paths::scenario_dir(sid)?;
    let run_id = resolve_run_id(&dir, run_ref)?;
    let audit_path = dir.join("replays").join(&run_id).join("audit.json");
    if !audit_path.is_file() {
        bail!("audit summary: no audit.json at {}", audit_path.display());
    }
    let bytes = fs::read(&audit_path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let s = value
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("(no summary recorded)");
    println!("{s}");
    Ok(0)
}

/// `audit explain <sid> [runId|latest] [--json]` — one-block failure
/// digest: verdict + failing steps + heal trail + console errors +
/// failed/pending requests, so an agent repairing a broken run reads a
/// single artifact instead of joining audit.json, events.jsonl,
/// heal.jsonl, console.json, and network.json by hand.
/// Exit mirrors `audit verdict`: 0 PASS, 1 BLOCK, 2 FIX.
fn explain(positionals: &[String], json_out: bool) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit explain <sid> [runId|latest]"))?;
    let run_ref = positionals.get(2).map(|s| s.as_str()).unwrap_or("latest");
    let dir = paths::scenario_dir(sid)?;
    let run_id = resolve_run_id(&dir, run_ref)?;
    let run_dir = dir.join("replays").join(&run_id);

    let (verdict, code, healed_steps, rejected_steps, summary_line, exit) = verdict_for(&run_dir)?;

    let mut failed: Vec<Value> = Vec::new();
    if let Ok(body) = fs::read_to_string(run_dir.join("events.jsonl")) {
        for line in body.lines() {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if row.get("status").and_then(|v| v.as_str()) != Some("fail") {
                continue;
            }
            let mut f = serde_json::json!({
                "stepId": row.get("id"),
                "intent": row.get("intent"),
                "kind": row.get("kind"),
                "error": row.get("error"),
            });
            if let Some(ms) = row.get("ms") {
                f["ms"] = ms.clone();
            }
            if let Some(shot) = row.get("screenshot").and_then(|v| v.as_str()) {
                f["screenshot"] = Value::String(shot.to_string());
            }
            if let Some(snap) = row.get("snapshot").and_then(|v| v.as_str()) {
                f["snapshot"] = Value::String(snap.to_string());
            }
            failed.push(f);
        }
    }

    let mut heals: Vec<Value> = Vec::new();
    if let Ok(body) = fs::read_to_string(run_dir.join("heal.jsonl")) {
        for line in body.lines() {
            let Ok(row) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            heals.push(row);
        }
    }

    let mut console_errors: Vec<String> = Vec::new();
    if let Ok(bytes) = fs::read(run_dir.join("console.json")) {
        if let Ok(c) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(msgs) = c.get("messages").and_then(|m| m.as_array()) {
                for m in msgs.iter() {
                    if m.get("type").and_then(|v| v.as_str()) == Some("error") {
                        if let Some(t) = m.get("text").and_then(|v| v.as_str()) {
                            if console_errors.len() < 10 {
                                console_errors.push(t.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let mut network_failures: Vec<Value> = Vec::new();
    if let Ok(bytes) = fs::read(run_dir.join("network.json")) {
        if let Ok(n) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(reqs) = n.get("requests").and_then(|r| r.as_array()) {
                for r in reqs.iter() {
                    let status = r.get("status").and_then(|v| v.as_i64());
                    let bad = match status {
                        Some(s) => s >= 400,
                        // A request with no status never got a response —
                        // dead backend, abort, or still pending at run end.
                        None => true,
                    };
                    if bad && network_failures.len() < 10 {
                        network_failures.push(serde_json::json!({
                            "method": r.get("method"),
                            "url": r.get("url"),
                            "status": status,
                        }));
                    }
                }
            }
        }
    }

    let has_shot_diff = run_dir.join("shots-diff").is_dir()
        || failed.iter().any(|f| {
            f.get("error")
                .and_then(|e| e.as_str())
                .is_some_and(|e| e.contains("differs from baseline"))
        });

    if json_out {
        let out = serde_json::json!({
            "scenario": sid,
            "runId": run_id,
            "verdict": verdict,
            "exitCode": exit,
            "summary": summary_line,
            "failed": failed,
            "heals": heals,
            "healedSteps": healed_steps,
            "rejectedSteps": rejected_steps,
            "consoleErrors": console_errors,
            "networkFailures": network_failures,
            "runDir": run_dir,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(code);
    }

    println!("{sid} @ {run_id} — {summary_line} (exit {exit})");
    println!("verdict: {verdict}");
    if !failed.is_empty() {
        println!("\nfailed steps ({}):", failed.len());
        for f in &failed {
            println!(
                "  {}  {}  {}",
                f.get("stepId").and_then(|v| v.as_str()).unwrap_or("?"),
                f.get("kind").and_then(|v| v.as_str()).unwrap_or("?"),
                f.get("intent").and_then(|v| v.as_str()).unwrap_or(""),
            );
            if let Some(e) = f.get("error").and_then(|v| v.as_str()) {
                println!("      {e}");
            }
            if let Some(s) = f.get("screenshot").and_then(|v| v.as_str()) {
                println!("      screenshot: replays/{run_id}/{s}");
            }
        }
    }
    if !heals.is_empty() {
        println!("\nheals ({}):", heals.len());
        for h in &heals {
            let step = h
                .get("stepId")
                .or_else(|| h.get("step_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let mode = h.get("mode").and_then(|v| v.as_str()).unwrap_or("?");
            if mode == "value-rejection" {
                let why = h.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
                println!("  {step}  {mode}  {why}");
            } else {
                let from = h.get("from").and_then(|v| v.as_str()).unwrap_or("?");
                let to = h.get("to").and_then(|v| v.as_str()).unwrap_or("?");
                let strat = h.get("strategy").and_then(|v| v.as_str()).unwrap_or("");
                println!("  {step}  {mode}  {from} → {to} [{strat}]");
            }
        }
    }
    if !console_errors.is_empty() {
        println!("\nconsole errors ({}):", console_errors.len());
        for e in &console_errors {
            println!("  {e}");
        }
    }
    if !network_failures.is_empty() {
        println!("\nnetwork failures ({}):", network_failures.len());
        for r in &network_failures {
            let status = r
                .get("status")
                .and_then(|v| v.as_i64())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "no response".into());
            println!(
                "  {} {} → {}",
                r.get("method").and_then(|v| v.as_str()).unwrap_or("?"),
                r.get("url").and_then(|v| v.as_str()).unwrap_or("?"),
                status
            );
        }
    }
    println!("\nrun dir: {}", run_dir.display());
    println!("\nnext:");
    println!("  agent-qa audit verdict {sid} {run_id} --json");
    println!("  agent-qa compare {sid}           # diff vs the previous run");
    if !heals.is_empty() {
        println!("  agent-qa heal-patch {sid} {run_id}   # review the heal diff");
    }
    if has_shot_diff {
        println!("  agent-qa shot-accept {sid}         # if the visual change is intended");
    }
    Ok(code)
}

fn diff(positionals: &[String]) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit diff <sid> <runIdA> <runIdB>"))?;
    let a_ref = positionals
        .get(2)
        .ok_or_else(|| anyhow!("usage: audit diff <sid> <runIdA> <runIdB>"))?;
    let b_ref = positionals
        .get(3)
        .ok_or_else(|| anyhow!("usage: audit diff <sid> <runIdA> <runIdB>"))?;
    let dir = paths::scenario_dir(sid)?;
    let a_id = resolve_run_id(&dir, a_ref)?;
    let b_id = resolve_run_id(&dir, b_ref)?;
    let a_path = dir.join("replays").join(&a_id).join("audit.json");
    let b_path = dir.join("replays").join(&b_id).join("audit.json");
    if !a_path.is_file() {
        bail!("audit diff: no audit.json at {}", a_path.display());
    }
    if !b_path.is_file() {
        bail!("audit diff: no audit.json at {}", b_path.display());
    }
    let a_body = fs::read_to_string(&a_path)?;
    let b_body = fs::read_to_string(&b_path)?;
    let a_v: Value = serde_json::from_str(&a_body)?;
    let b_v: Value = serde_json::from_str(&b_body)?;
    let a_pretty = serde_json::to_string_pretty(&a_v)?;
    let b_pretty = serde_json::to_string_pretty(&b_v)?;
    if a_pretty == b_pretty {
        println!("identical: {} == {}", a_path.display(), b_path.display());
        return Ok(0);
    }
    let body = similar::TextDiff::from_lines(&a_pretty, &b_pretty)
        .unified_diff()
        .context_radius(3)
        .header(&a_path.display().to_string(), &b_path.display().to_string())
        .to_string();
    print!("{body}");
    Ok(1)
}

fn stats_all(json_out: bool, since_ms: Option<u64>, until_ms: Option<u64>) -> Result<u8> {
    let root = paths::scenarios_root();
    let mut scenarios: Vec<std::path::PathBuf> = match fs::read_dir(&root) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    scenarios.sort();

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        sid: String,
        total: u32,
        passes: u32,
        failures: u32,
        unknown: u32,
        pass_rate: f64,
        avg_duration_secs: f64,
        last_pass: Option<String>,
        last_fail: Option<String>,
        flake_score: Option<f64>,
        healed_runs: usize,
    }
    let mut rows: Vec<Row> = Vec::with_capacity(scenarios.len());
    let mut tot_total = 0u32;
    let mut tot_dur_ms_total: u64 = 0;
    let mut tot_dur_ms_count: u64 = 0;
    let mut tot_passes = 0u32;
    let mut tot_failures = 0u32;
    let mut tot_unknown = 0u32;
    for jdir in &scenarios {
        let sid = jdir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let replays_dir = jdir.join("replays");
        let runs = crate::paths::run_dirs(&replays_dir);
        let (mut total, mut passes, mut failures, mut unknown) = (0u32, 0u32, 0u32, 0u32);
        let mut last_pass: Option<String> = None;
        let mut last_fail: Option<String> = None;
        let mut dur_ms_total: u64 = 0;
        let mut dur_ms_count: u64 = 0;
        let mut outcomes: Vec<bool> = Vec::new();
        let mut healed_runs = 0usize;
        for run in &runs {
            let run_id = run
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let audit: Option<Value> = fs::read(run.join("audit.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            if since_ms.is_some() || until_ms.is_some() {
                if let Some(s) = audit.as_ref().and_then(|a| a.get("startedAt")?.as_str()) {
                    if let Ok(started) = crate::time::parse_iso_ms(s) {
                        if let Some(min) = since_ms {
                            if started < min {
                                continue;
                            }
                        }
                        if let Some(max) = until_ms {
                            if started > max {
                                continue;
                            }
                        }
                    }
                }
            }
            total += 1;
            match audit.as_ref().and_then(|a| a.get("exitCode")?.as_i64()) {
                Some(0) => {
                    passes += 1;
                    outcomes.push(true);
                    last_pass = Some(run_id.clone());
                }
                Some(_) => {
                    failures += 1;
                    outcomes.push(false);
                    last_fail = Some(run_id.clone());
                }
                None => unknown += 1,
            }
            if audit
                .as_ref()
                .and_then(|a| a.get("autoHealed")?.as_array())
                .is_some_and(|v| !v.is_empty())
            {
                healed_runs += 1;
            }
            if let (Some(s), Some(f)) = (
                audit.as_ref().and_then(|a| a.get("startedAt")?.as_str()),
                audit.as_ref().and_then(|a| a.get("finishedAt")?.as_str()),
            ) {
                if let (Ok(sm), Ok(fm)) = (parse_iso_ms(s), parse_iso_ms(f)) {
                    dur_ms_total = dur_ms_total.saturating_add(fm.saturating_sub(sm));
                    dur_ms_count += 1;
                }
            }
        }
        let pass_rate = if passes + failures == 0 {
            0.0
        } else {
            passes as f64 / (passes + failures) as f64
        };
        let avg_duration_secs = if dur_ms_count == 0 {
            0.0
        } else {
            (dur_ms_total as f64 / dur_ms_count as f64) / 1000.0
        };
        tot_total += total;
        tot_passes += passes;
        tot_failures += failures;
        tot_unknown += unknown;
        tot_dur_ms_total = tot_dur_ms_total.saturating_add(dur_ms_total);
        tot_dur_ms_count += dur_ms_count;
        rows.push(Row {
            sid,
            total,
            passes,
            failures,
            unknown,
            pass_rate,
            avg_duration_secs,
            last_pass,
            last_fail,
            flake_score: flake_score(&outcomes, healed_runs, total as usize),
            healed_runs,
        });
    }
    let overall_pass_rate = if tot_passes + tot_failures == 0 {
        0.0
    } else {
        tot_passes as f64 / (tot_passes + tot_failures) as f64
    };
    let overall_avg_duration_secs = if tot_dur_ms_count == 0 {
        0.0
    } else {
        (tot_dur_ms_total as f64 / tot_dur_ms_count as f64) / 1000.0
    };
    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Report {
            scenarios_root: String,
            total: u32,
            passes: u32,
            failures: u32,
            unknown: u32,
            pass_rate: f64,
            avg_duration_secs: f64,
            scenarios: Vec<Row>,
        }
        let report = Report {
            scenarios_root: root.display().to_string(),
            total: tot_total,
            passes: tot_passes,
            failures: tot_failures,
            unknown: tot_unknown,
            pass_rate: overall_pass_rate,
            avg_duration_secs: overall_avg_duration_secs,
            scenarios: rows,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(0);
    }
    println!(
        "audit stats-all: {} ({} scenario(s))",
        root.display(),
        rows.len()
    );
    if rows.is_empty() {
        return Ok(0);
    }
    let sid_h = "sid";
    let total_h = "runs";
    let pass_h = "pass";
    let fail_h = "fail";
    let rate_h = "rate";
    let flake_h = "flake";
    println!("{sid_h:<24}  {total_h:>5}  {pass_h:>5}  {fail_h:>5}  {rate_h:>5}  {flake_h:>5}");
    for r in &rows {
        let sid = &r.sid;
        let total = r.total;
        let pass = r.passes;
        let fail = r.failures;
        let rate = format!("{:.0}%", r.pass_rate * 100.0);
        let flake = r
            .flake_score
            .map(|s| format!("{s:.0}"))
            .unwrap_or_else(|| "-".to_string());
        println!("{sid:<24}  {total:>5}  {pass:>5}  {fail:>5}  {rate:>5}  {flake:>5}");
    }
    let rate = format!("{:.0}%", overall_pass_rate * 100.0);
    println!(
        "\nOVERALL: total={tot_total} pass={tot_passes} fail={tot_failures} unknown={tot_unknown} rate={rate} avg_duration={overall_avg_duration_secs:.3}s"
    );
    Ok(0)
}

/// `audit flaky <sid>` — flag steps whose outcome interleaves across runs.
///
/// `heal-chronic` catches locator churn (a step that self-heals every run);
/// this catches *outcome* churn: a step that passes, then fails, then passes
/// again is flaky even when nothing heals. Per run we take the step's last
/// terminal `events.jsonl` row (`pass`/`fail`); a run where the step never
/// reached a terminal state contributes no observation (not a gap — early
/// aborts shouldn't count as flips). A step is flaky when its observed
/// outcome sequence flips at least `--min-flips` times (default 2, i.e. a
/// true P→F→P / F→P→F interleave) across at least `--min-runs` observations
/// (default 3). One flip (P→F staying failed) reads as a regression, not
/// flake, and `audit stats`/`list` already show that.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FlakyStep {
    step_id: String,
    flips: usize,
    seen: usize,
    seq: String,
    heals: usize,
    fail_runs: Vec<String>,
    last_error: Option<String>,
}

fn collect_flaky(dir: &std::path::Path, min_flips: usize, min_runs: usize) -> Vec<FlakyStep> {
    let replays_dir = dir.join("replays");
    let runs = crate::paths::run_dirs(&replays_dir);

    // Per step: ordered outcome observations + which runs failed + heal rows.
    #[derive(Default)]
    struct Acc {
        seq: Vec<char>,         // 'P' | 'F' in chronological run order
        seen: usize,            // runs where the step reached a terminal row
        fail_runs: Vec<String>, // run ids where the terminal row was 'fail'
        heals: usize,           // heal.jsonl rows across all runs
        last_error: Option<String>,
    }
    let mut by_step: std::collections::BTreeMap<String, Acc> = Default::default();

    for run in &runs {
        let run_id = run
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Last terminal status per step wins (a step can emit running → fail
        // → pass under a retry/self-heal within one run).
        let mut terminal: std::collections::BTreeMap<String, (char, Option<String>)> =
            Default::default();
        if let Ok(body) = fs::read_to_string(run.join("events.jsonl")) {
            for line in body.lines() {
                let v: Value = match serde_json::from_str(line.trim()) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                if status != "pass" && status != "fail" {
                    continue;
                }
                let id = match v.get("id").and_then(|s| s.as_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let err = v
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string());
                terminal.insert(id, (if status == "pass" { 'P' } else { 'F' }, err));
            }
        }
        let mut heal_steps: std::collections::BTreeSet<String> = Default::default();
        if let Ok(body) = fs::read_to_string(run.join("heal.jsonl")) {
            for line in body.lines() {
                if let Ok(v) = serde_json::from_str::<Value>(line.trim()) {
                    if let Some(id) = v.get("stepId").and_then(|s| s.as_str()) {
                        heal_steps.insert(id.to_string());
                    }
                }
            }
        }
        for id in &heal_steps {
            by_step.entry(id.clone()).or_default().heals += 1;
        }
        for (id, (mark, err)) in terminal {
            let acc = by_step.entry(id).or_default();
            acc.seq.push(mark);
            acc.seen += 1;
            if mark == 'F' {
                acc.fail_runs.push(run_id.clone());
                acc.last_error = err;
            }
        }
    }

    let mut out: Vec<FlakyStep> = by_step
        .into_iter()
        .filter_map(|(step_id, acc)| {
            let flips = acc.seq.windows(2).filter(|w| w[0] != w[1]).count();
            if flips < min_flips || acc.seen < min_runs {
                return None;
            }
            Some(FlakyStep {
                step_id,
                flips,
                seen: acc.seen,
                seq: acc.seq.iter().collect(),
                heals: acc.heals,
                fail_runs: acc.fail_runs,
                last_error: acc.last_error,
            })
        })
        .collect();
    out.sort_by(|a, b| b.flips.cmp(&a.flips).then(a.step_id.cmp(&b.step_id)));
    out
}

/// `audit slow <sid>` — flag steps whose duration regressed across runs.
///
/// Per run, the step's last terminal `events.jsonl` row carries `ms`. A step
/// is slow when EVERY one of its last `--recent` pass observations (default 2)
/// exceeds its earlier-run median by at least `--pct` percent (default 50)
/// AND `--min-ms` milliseconds (default 250) — min-of-window keeps a single
/// outlier run from flagging, and the absolute floor keeps jittery 40ms steps
/// quiet. Failed steps contribute no observation (a locator-timeout `ms` is
/// the timeout budget, not the step's cost); a run where the step never
/// reached a terminal row contributes nothing. Requires `--min-runs` pass
/// observations plus at least 2 baseline runs on top of the recent window.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SlowStep {
    step_id: String,
    base_ms: f64,
    recent_ms: f64,
    pct_delta: f64,
    seen: usize,
    fails: usize,
}

fn median_ms(v: &[u64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    let mid = s.len() / 2;
    if s.len() % 2 == 1 {
        s[mid] as f64
    } else {
        (s[mid - 1] as f64 + s[mid] as f64) / 2.0
    }
}

fn collect_slow(
    dir: &std::path::Path,
    pct: f64,
    min_ms: u64,
    recent_n: usize,
    min_runs: usize,
) -> Vec<SlowStep> {
    let replays_dir = dir.join("replays");
    let runs = crate::paths::run_dirs(&replays_dir);

    // Per step: chronological pass-ms observations + a fail count for context.
    #[derive(Default)]
    struct Acc {
        ms_seq: Vec<u64>,
        fails: usize,
    }
    let mut by_step: std::collections::BTreeMap<String, Acc> = Default::default();

    for run in &runs {
        let mut terminal: std::collections::BTreeMap<String, (bool, Option<u64>)> =
            Default::default();
        if let Ok(body) = fs::read_to_string(run.join("events.jsonl")) {
            for line in body.lines() {
                let v: Value = match serde_json::from_str(line.trim()) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
                if status != "pass" && status != "fail" {
                    continue;
                }
                let id = match v.get("id").and_then(|s| s.as_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let ms = v.get("ms").and_then(|m| m.as_u64());
                terminal.insert(id, (status == "pass", ms));
            }
        }
        for (id, (passed, ms)) in terminal {
            let acc = by_step.entry(id).or_default();
            if !passed {
                acc.fails += 1;
                continue;
            }
            if let Some(ms) = ms {
                acc.ms_seq.push(ms);
            }
        }
    }

    let mut out: Vec<SlowStep> = by_step
        .into_iter()
        .filter_map(|(step_id, acc)| {
            let n = acc.ms_seq.len();
            // Need the recent window plus at least two baseline observations.
            if n < min_runs || n < recent_n + 2 {
                return None;
            }
            let base = median_ms(&acc.ms_seq[..n - recent_n]);
            let recent_min = *acc.ms_seq[n - recent_n..].iter().min().unwrap() as f64;
            let delta = recent_min - base;
            if base <= 0.0 || delta < min_ms as f64 || recent_min < base * (1.0 + pct / 100.0) {
                return None;
            }
            Some(SlowStep {
                step_id,
                base_ms: base,
                recent_ms: recent_min,
                pct_delta: (delta / base) * 100.0,
                seen: n,
                fails: acc.fails,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.pct_delta
            .partial_cmp(&a.pct_delta)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.step_id.cmp(&b.step_id))
    });
    out
}

fn slow(
    positionals: &[String],
    json_out: bool,
    pct: f64,
    min_ms: u64,
    recent_n: usize,
    min_runs: usize,
) -> Result<u8> {
    let sid = positionals.get(1).ok_or_else(|| {
        anyhow!("usage: audit slow <sid> [--pct N] [--min-ms N] [--recent N] [--min-runs N]")
    })?;
    let dir = paths::scenario_dir(sid)?;
    let out = collect_slow(&dir, pct, min_ms, recent_n, min_runs);

    if json_out {
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(0);
    }
    if out.is_empty() {
        println!(
            "(no slow steps — pct={pct:.0} min-ms={min_ms} recent={recent_n} min-runs={min_runs})"
        );
        return Ok(0);
    }
    println!(
        "{:<10} {:>9} {:>9} {:>7} {:>6} {:>6}",
        "stepId", "baseMs", "worstMs", "+", "seen", "fails"
    );
    println!("{}", "-".repeat(60));
    for e in &out {
        println!(
            "{:<10} {:>9.0} {:>9.0} {:>6.0}% {:>6} {:>6}",
            e.step_id, e.base_ms, e.recent_ms, e.pct_delta, e.seen, e.fails
        );
    }
    println!();
    println!(
        "{} step(s) slowed >{pct:.0}% in their last {recent_n} run(s) — check for added waits or heavier pages.",
        out.len()
    );
    Ok(0)
}

/// `audit health` — cross-scenario rollup of the three silent-degradation
/// detectors: `flaky` (outcome churn), `slow` (duration regression), and
/// `heal-chronic` (locator churn). One filesystem walk per scenario at each
/// detector's defaults; a scenario only appears when it has at least one
/// flag, so an empty table means the whole suite is quiet. The workbench
/// calls this with --json to badge scenario rows.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthRow {
    scenario_id: String,
    flaky: Vec<String>,
    slow: Vec<String>,
    chronic: Vec<String>,
}

fn collect_health(root: &std::path::Path) -> Vec<HealthRow> {
    let mut sids: Vec<(String, PathBuf)> = match fs::read_dir(root) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter_map(|p| {
                let sid = p.file_name()?.to_string_lossy().into_owned();
                p.join("replays").is_dir().then_some((sid, p))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    sids.sort_by(|a, b| a.0.cmp(&b.0));

    sids.into_iter()
        .filter_map(|(sid, dir)| {
            let flaky = collect_flaky(&dir, 2, 3)
                .into_iter()
                .map(|f| f.step_id)
                .collect::<Vec<_>>();
            let slow = collect_slow(&dir, 50.0, 250, 2, 3)
                .into_iter()
                .map(|s| s.step_id)
                .collect::<Vec<_>>();
            let chronic = crate::heal_chronic::collect_dir(&dir, 2, &sid)
                .into_iter()
                .map(|c| c.step_id)
                .collect::<Vec<_>>();
            if flaky.is_empty() && slow.is_empty() && chronic.is_empty() {
                return None;
            }
            Some(HealthRow {
                scenario_id: sid,
                flaky,
                slow,
                chronic,
            })
        })
        .collect()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrendRun {
    run_id: String,
    exit_code: Option<i64>,
    duration_secs: Option<f64>,
    started_at: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrendOut {
    scenario_id: String,
    runs: Vec<TrendRun>,
    passed: usize,
    failed: usize,
    median_secs: f64,
    outcomes: String,
    sparkline: String,
}

fn sparkline(values: &[Option<f64>]) -> String {
    const BARS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let present: Vec<f64> = values.iter().flatten().copied().collect();
    if present.is_empty() {
        return String::new();
    }
    let min = present.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = present.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = max - min;
    values
        .iter()
        .map(|v| match v {
            None => ' ',
            Some(_) if span <= 0.0 => '▄',
            Some(v) => {
                let idx = ((v - min) / span * (BARS.len() - 1) as f64).round() as usize;
                BARS[idx.min(BARS.len() - 1)]
            }
        })
        .collect()
}

fn collect_trend(dir: &std::path::Path, sid: &str, limit: Option<usize>) -> TrendOut {
    let replays_dir = dir.join("replays");
    let mut runs = crate::paths::run_dirs(&replays_dir);
    if let Some(n) = limit {
        runs.drain(..runs.len().saturating_sub(n));
    }

    let mut trend_runs: Vec<TrendRun> = Vec::with_capacity(runs.len());
    for run in &runs {
        let audit: Option<Value> = fs::read(run.join("audit.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let duration_secs = match (
            audit.as_ref().and_then(|a| a.get("startedAt")?.as_str()),
            audit.as_ref().and_then(|a| a.get("finishedAt")?.as_str()),
        ) {
            (Some(s), Some(f)) => match (parse_iso_ms(s), parse_iso_ms(f)) {
                (Ok(sm), Ok(fm)) => Some(fm.saturating_sub(sm) as f64 / 1000.0),
                _ => None,
            },
            _ => None,
        };
        trend_runs.push(TrendRun {
            run_id: run
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            exit_code: audit.as_ref().and_then(|a| a.get("exitCode")?.as_i64()),
            duration_secs,
            started_at: audit
                .as_ref()
                .and_then(|a| a.get("startedAt")?.as_str().map(str::to_string)),
        });
    }

    let with_audit: Vec<&TrendRun> = trend_runs
        .iter()
        .filter(|r| r.exit_code.is_some())
        .collect();
    let passed = with_audit.iter().filter(|r| r.exit_code == Some(0)).count();
    let failed = with_audit.len() - passed;
    let outcomes: String = with_audit
        .iter()
        .map(|r| if r.exit_code == Some(0) { '✓' } else { '✗' })
        .collect();
    let spark = sparkline(
        &with_audit
            .iter()
            .map(|r| r.duration_secs)
            .collect::<Vec<_>>(),
    );
    let median_secs = median_ms(
        &with_audit
            .iter()
            .filter_map(|r| r.duration_secs.map(|d| (d * 1000.0) as u64))
            .collect::<Vec<_>>(),
    ) / 1000.0;
    TrendOut {
        scenario_id: sid.to_string(),
        runs: trend_runs,
        passed,
        failed,
        median_secs,
        outcomes,
        sparkline: spark,
    }
}

// Suite board: one trend row per scenario that has a replays/ dir.
fn collect_trend_all(root: &std::path::Path, limit: Option<usize>) -> Vec<TrendOut> {
    let mut sids: Vec<(String, PathBuf)> = match fs::read_dir(root) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter_map(|p| {
                let sid = p.file_name()?.to_string_lossy().into_owned();
                p.join("replays").is_dir().then_some((sid, p))
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    sids.sort_by(|a, b| a.0.cmp(&b.0));
    sids.iter()
        .map(|(sid, dir)| collect_trend(dir, sid, limit))
        .collect()
}

fn trend(positionals: &[String], json_out: bool, limit: Option<usize>, all: bool) -> Result<u8> {
    if all {
        let rows = collect_trend_all(&paths::scenarios_root(), limit);
        if json_out {
            println!("{}", serde_json::to_string(&rows)?);
            return Ok(0);
        }
        println!(
            "{:<24} {:>5} {:>5} {:>8}  outcomes / duration",
            "scenario", "pass%", "runs", "median"
        );
        println!("{}", "-".repeat(100));
        for r in &rows {
            let total = r.passed + r.failed;
            let pct = if total > 0 {
                (r.passed as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "{:<24} {:>4.0}% {:>5} {:>7.2}s  {} {}",
                r.scenario_id, pct, total, r.median_secs, r.outcomes, r.sparkline
            );
        }
        return Ok(0);
    }
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit trend <sid> [--limit N] [--json] | audit trend --all [--limit N] [--json]"))?;
    let dir = paths::scenario_dir(sid)?;
    let out = collect_trend(&dir, sid, limit);

    if json_out {
        // Compact single-line output: the workbench's lastJsonLine parser
        // (and shell pipes) expect one JSON value per line.
        println!("{}", serde_json::to_string(&out)?);
        return Ok(0);
    }
    if out.runs.is_empty() {
        println!("{sid}: (no runs)");
        return Ok(0);
    }
    let total = out.passed + out.failed;
    let pct = if total > 0 {
        (out.passed as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    println!(
        "{sid}: {total} run(s) — pass {pct:.0}% ({}/{}), median {:.2}s",
        out.passed, total, out.median_secs
    );
    println!("outcomes  {}", out.outcomes);
    println!("duration  {}", out.sparkline);
    Ok(0)
}

fn health(json_out: bool) -> Result<u8> {
    let out = collect_health(&paths::scenarios_root());
    if json_out {
        // Compact single-line output: the workbench's lastJsonLine parser
        // (and shell pipes) expect one JSON value per line.
        println!("{}", serde_json::to_string(&out)?);
        return Ok(0);
    }
    if out.is_empty() {
        println!("(all quiet — no flaky, slow, or chronic-heal steps across any scenario)");
        return Ok(0);
    }
    println!(
        "{:<24} {:>6} {:>6} {:>8}  steps",
        "scenario", "flaky", "slow", "chronic"
    );
    println!("{}", "-".repeat(80));
    for r in &out {
        let mut steps: Vec<String> = Vec::new();
        for id in &r.flaky {
            steps.push(format!("{id} (flaky)"));
        }
        for id in &r.slow {
            steps.push(format!("{id} (slow)"));
        }
        for id in &r.chronic {
            steps.push(format!("{id} (chronic)"));
        }
        println!(
            "{:<24} {:>6} {:>6} {:>8}  {}",
            r.scenario_id,
            r.flaky.len(),
            r.slow.len(),
            r.chronic.len(),
            steps.join(", ")
        );
    }
    Ok(0)
}

/// `audit cluster` — group step failures across every scenario's recent
/// runs by a normalized error signature, so one root cause surfacing in N
/// scenarios reads as ONE triage item, not N unrelated reds. A failing
/// step's `error` is normalized (quoted literals → `'`, digit runs → `#`,
/// whitespace collapsed, lowercased) — locator names, step ids, and timing
/// numbers don't fragment the signature.
///
/// Sorted by blast radius (distinct scenarios hit) then occurrence count.
/// A cluster needs >= `--min-size` (default 2) occurrences to print — a
/// lone failure is just a failure, `audit show`/`verdict` cover it.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ClusterMember {
    scenario: String,
    run_id: String,
    step_id: String,
    error: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Cluster {
    signature: String,
    count: usize,
    scenarios: Vec<String>,
    members: Vec<ClusterMember>,
}

/// Normalize a failure string into a clustering signature: lowercase,
/// quoted runs replaced by `'`, digit runs by `#`, whitespace collapsed.
fn error_signature(err: &str) -> String {
    let lower = err.to_lowercase();
    let mut sig = String::with_capacity(lower.len());
    let mut in_quote: Option<char> = None;
    let mut in_digits = false;
    for c in lower.chars() {
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            in_quote = Some(c);
            sig.push('\'');
            in_digits = false;
            continue;
        }
        if c.is_ascii_digit() {
            if !in_digits {
                sig.push('#');
                in_digits = true;
            }
            continue;
        }
        in_digits = false;
        if c.is_whitespace() {
            if !sig.ends_with(' ') && !sig.is_empty() {
                sig.push(' ');
            }
            continue;
        }
        sig.push(c);
    }
    sig.trim().chars().take(160).collect()
}

fn collect_clusters(root: &std::path::Path, min_size: usize) -> Vec<Cluster> {
    let mut by_sig: BTreeMap<String, Vec<ClusterMember>> = BTreeMap::new();
    let Ok(sids) = fs::read_dir(root) else {
        return Vec::new();
    };
    for sid_dir in sids.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let sid = sid_dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let replays = sid_dir.join("replays");
        for run_dir in crate::paths::run_dirs(&replays) {
            let run_id = run_dir
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let Ok(body) = fs::read_to_string(run_dir.join("events.jsonl")) else {
                continue;
            };
            for line in body.lines() {
                let Ok(ev) = serde_json::from_str::<StepEvent>(line) else {
                    continue;
                };
                if ev.status != "fail" {
                    continue;
                }
                let err = ev.error.unwrap_or_default();
                by_sig
                    .entry(error_signature(&err))
                    .or_default()
                    .push(ClusterMember {
                        scenario: sid.clone(),
                        run_id: run_id.clone(),
                        step_id: ev.id,
                        error: err,
                    });
            }
        }
    }
    let mut clusters: Vec<Cluster> = by_sig
        .into_iter()
        .filter(|(_, members)| members.len() >= min_size)
        .map(|(signature, members)| {
            let mut scenarios: Vec<String> = members
                .iter()
                .map(|m| m.scenario.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            scenarios.sort();
            Cluster {
                signature,
                count: members.len(),
                scenarios,
                members,
            }
        })
        .collect();
    clusters.sort_by(|a, b| {
        b.scenarios
            .len()
            .cmp(&a.scenarios.len())
            .then(b.count.cmp(&a.count))
    });
    clusters
}

fn cluster(json_out: bool, min_size: usize) -> Result<u8> {
    let out = collect_clusters(&paths::scenarios_root(), min_size);
    if json_out {
        println!("{}", serde_json::to_string(&out)?);
        return Ok(0);
    }
    if out.is_empty() {
        println!("(no failure clusters — no repeated error signature across runs)");
        return Ok(0);
    }
    for c in &out {
        println!(
            "{} hit(s) across {} scenario(s) — {}",
            c.count,
            c.scenarios.len(),
            c.signature
        );
        for m in c.members.iter().take(5) {
            let err: String = m.error.chars().take(100).collect();
            println!("  {} {} {}: {}", m.scenario, m.run_id, m.step_id, err);
        }
        if c.members.len() > 5 {
            println!("  … and {} more", c.members.len() - 5);
        }
        println!();
    }
    Ok(0)
}

fn flaky(positionals: &[String], json_out: bool, min_flips: usize, min_runs: usize) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit flaky <sid> [--min-flips N] [--min-runs N]"))?;
    let dir = paths::scenario_dir(sid)?;
    let out = collect_flaky(&dir, min_flips, min_runs);

    if json_out {
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(0);
    }
    if out.is_empty() {
        println!("(no flaky steps — min-flips={min_flips} min-runs={min_runs})");
        return Ok(0);
    }
    println!(
        "{:<10} {:<6} {:<6} {:<8} last error",
        "stepId", "flips", "seen", "seq"
    );
    println!("{}", "-".repeat(100));
    for e in &out {
        let err = e
            .last_error
            .as_deref()
            .map(|s| s.chars().take(60).collect::<String>())
            .unwrap_or_default();
        println!(
            "{:<10} {:<6} {:<6} {:<8} {}",
            e.step_id, e.flips, e.seen, e.seq, err
        );
    }
    println!();
    println!(
        "{} flaky step(s) interleaved pass/fail across {}+ observations — quarantine or fix the cause.",
        out.len(),
        min_runs
    );
    Ok(0)
}

pub(crate) fn resolve_run_id(scenario_dir: &std::path::Path, run_ref: &str) -> Result<String> {
    if run_ref != "latest" {
        return Ok(run_ref.to_string());
    }
    // 1) Prefer replays/latest.txt (written by the runner on each finish).
    let latest_txt = scenario_dir.join("replays").join("latest.txt");
    if let Ok(body) = fs::read_to_string(&latest_txt) {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    // 2) Fall back to the highest lex-sorted run dir.
    let last = crate::paths::run_dirs(&scenario_dir.join("replays"))
        .pop()
        .ok_or_else(|| anyhow!("audit show: no replays under {}", scenario_dir.display()))?;
    Ok(last
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default())
}

fn render_text(path: &std::path::Path, audit: &Value) {
    println!("audit: {}", path.display());
    let pick = |k: &str| {
        audit
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string()
    };
    let pick_i = |k: &str| {
        audit
            .get(k)
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "?".into())
    };
    println!("  runId       : {}", pick("runId"));
    println!("  scenarioId   : {}", pick("scenarioId"));
    println!("  startedAt   : {}", pick("startedAt"));
    println!("  finishedAt  : {}", pick("finishedAt"));
    println!("  summary     : {}", pick("summary"));
    println!("  exitCode    : {}", pick_i("exitCode"));
    println!("  profile     : {}", pick("profile"));
    if let Some(tag) = audit.get("tag").and_then(|v| v.as_str()) {
        println!("  tag         : {tag}");
    }
    if let Some(params) = audit.get("parameters").and_then(|v| v.as_array()) {
        if !params.is_empty() {
            println!("  parameters  :");
            for p in params {
                let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let value = p
                    .get("value")
                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                    .unwrap_or_else(|| "?".into());
                println!("    - {name} = {value}");
            }
        }
    }
    if let Some(applied) = audit.get("healOverridesApplied").and_then(|v| v.as_array()) {
        if !applied.is_empty() {
            println!("  healOverridesApplied:");
            for a in applied {
                if let Some(s) = a.as_str() {
                    println!("    - {s}");
                }
            }
        }
    }
}

fn print_help() {
    println!(
                "agent-qa audit \u{2014} inspect a replay's audit.json\n\nUsage:\n  agent-qa audit show <sid> [<runId | latest>] [--json | --format text|json|github]\n  agent-qa audit list <sid>                    Table view: every run's\n                                               summary / exit / profile / tag\n  agent-qa audit list <sid> --json             Structured rows on stdout\n  agent-qa audit list <sid> [--passed | --failed] [--tag <pat>] [--profile <pat>] [--limit N] [--slow <secs>] [--sort duration|runId-desc] [--since <iso-ts>] [--until <iso-ts>] [--format text|json|github]\n                                               Filters: case-insensitive substring\n                                               --passed/--failed are exit-code partitions\n  agent-qa audit stats <sid> [--since <iso-ts>] [--until <iso-ts>]\n                                               Pass/fail/tag rollup for one scenario\n  agent-qa audit stats <sid> --json            Structured rollup on stdout\n  agent-qa audit stats-all                     Per-scenario + overall pass/fail rollup\n  agent-qa audit stats-all --json              Structured rollup on stdout\n  agent-qa audit stats-all [--since <iso-ts>] [--until <iso-ts>]\n                                               Constrain to a date window\n  agent-qa audit diff <sid> <runIdA> <runIdB>  Unified diff between two replays'\n                                               audit.json (canonicalised JSON;\n                                               'latest' accepted for either side;\n                                               exit 1 on difference)\n  agent-qa audit summary <sid> [<runId | latest>]\n                                               Print just the summary line (one line out)\n  agent-qa audit exit-code <sid> [<runId | latest>]\n                                               Print just the run's exitCode (-1 if missing)\n  agent-qa audit field <sid> <runId | latest> <fieldName>\n                                               Print any top-level audit field. String/\n                                               number/bool print verbatim; null prints\n                                               empty; object/array prints compact JSON.\n  agent-qa audit count <sid>                   Print the number of runs under <sid>\n  agent-qa audit duration <sid> [<runId | latest>]\n                                               Print the run's duration in seconds\n                                               (finishedAt - startedAt, 3 decimals)\n  agent-qa audit flaky <sid> [--min-flips N] [--min-runs N] [--json]\n                                               Flag steps whose outcome interleaves\n                                               pass/fail across runs (outcome churn;\n                                               heal-chronic covers locator churn)\n  agent-qa audit slow <sid> [--pct N] [--min-ms N] [--recent N] [--min-runs N] [--json]\n                                               Flag steps whose recent pass median\n                                               regressed vs their earlier-run median\n                                               (default: last 2 runs >50% and >250ms\n                                               over baseline)\n  agent-qa audit health [--json]           Cross-scenario rollup of flaky + slow +
                                               heal-chronic — one row per scenario
                                               that has silent degradation\n  agent-qa audit verdict <sid> [<runId | latest>] [--json]\n                                               One-word run triage: PASS (exit 0) clean\n                                               green, FIX (exit 2) green but self-\n                                               corrected, BLOCK (exit 1) failed\n  agent-qa audit verdict --all [--json]\n                                               Suite triage board: one verdict row per\n                                               scenario's latest run; exit 1 if any\n                                               BLOCK (FIX rows are warnings only)\n  agent-qa audit cluster [--min-size N] [--json]\n                                               Group step failures across every scenario\n                                               by normalized error signature — one root\n                                               cause across N runs reads as one item\n  agent-qa audit explain <sid> [runId | latest] [--json]\n                                               One-block failure digest: verdict +\n                                               failing steps + heal trail + console\n                                               errors + failed requests, with the\n                                               follow-up commands that fit the\n                                               evidence (exit mirrors audit verdict)\n  agent-qa audit trend <sid> [--limit N] [--json]\n                                               Outcome + duration trend for the last N\n                                               runs (default all): pass%, median secs,\n                                               a ✓/✗ outcome line + a duration sparkline\n  agent-qa audit trend --all [--limit N] [--json]\n                                               Suite board: one trend row per scenario\n\n'latest' resolves to <sid>/replays/latest.txt if present, otherwise the\nhighest lex-sorted run directory (run_id is timestamp-prefixed)."


    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_audit(dir: &std::path::Path, run_id: &str, summary: &str, exit: i64) {
        let run_dir = dir.join("replays").join(run_id);
        std::fs::create_dir_all(&run_dir).unwrap();
        let body = format!(
            r#"{{"schema":"scenario-replay-audit/v1","runId":"{run_id}","scenarioId":"j","startedAt":"2026-01-01T00:00:00.000Z","finishedAt":"2026-01-01T00:00:01.000Z","summary":"{summary}","exitCode":{exit},"profile":"default","scenarioContentHash":"deadbeef","parameters":[{{"name":"x","value":"y"}}],"healOverridesApplied":["s1"]}}"#
        );
        std::fs::write(run_dir.join("audit.json"), body).unwrap();
    }

    #[test]
    fn resolve_run_id_prefers_latest_txt() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "2026-01-01__a", "SUMMARY: 1/1 (PASS)", 0);
        write_audit(&jdir, "2026-01-02__b", "SUMMARY: 1/1 (PASS)", 0);
        std::fs::write(jdir.join("replays").join("latest.txt"), "2026-01-01__a\n").unwrap();
        let got = resolve_run_id(&jdir, "latest").unwrap();
        assert_eq!(got, "2026-01-01__a");
    }

    #[test]
    fn resolve_run_id_falls_back_to_lex_max() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "2026-01-01__a", "SUMMARY: 1/1 (PASS)", 0);
        write_audit(&jdir, "2026-01-02__b", "SUMMARY: 1/1 (PASS)", 0);
        let got = resolve_run_id(&jdir, "latest").unwrap();
        assert_eq!(got, "2026-01-02__b");
    }

    #[test]
    fn resolve_run_id_explicit_passthrough() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        std::fs::create_dir_all(&jdir).unwrap();
        assert_eq!(
            resolve_run_id(&jdir, "2025-12-31__abc").unwrap(),
            "2025-12-31__abc"
        );
    }

    #[test]
    fn list_emits_zero_rows_when_no_replays() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        std::fs::create_dir_all(tmp.path().join("sid")).unwrap();
        let code = list(
            &["list".into(), "sid".into()],
            false,
            false,
            &ListFilters::default(),
        )
        .unwrap();
        assert_eq!(code, 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn stats_rollup_counts_pass_fail() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "2026-01-01__a", "SUMMARY: 3/3 (PASS)", 0);
        write_audit(&jdir, "2026-01-02__b", "SUMMARY: 2/3 (FAIL)", 1);
        write_audit(&jdir, "2026-01-03__c", "SUMMARY: 3/3 (PASS)", 0);
        assert_eq!(
            stats(&["stats".into(), "sid".into()], false, None, None).unwrap(),
            0
        );
        assert_eq!(
            stats(&["stats".into(), "sid".into()], true, None, None).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn stats_tolerates_no_replays() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        std::fs::create_dir_all(tmp.path().join("sid")).unwrap();
        assert_eq!(
            stats(&["stats".into(), "sid".into()], true, None, None).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn list_renders_multiple_runs() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "2026-01-01__a", "SUMMARY: 3/3 (PASS)", 0);
        write_audit(&jdir, "2026-01-02__b", "SUMMARY: 2/3 (FAIL)", 1);
        // Both text and json modes return 0; rows reflect lex-sort.
        assert_eq!(
            list(
                &["list".into(), "sid".into()],
                false,
                false,
                &ListFilters::default()
            )
            .unwrap(),
            0
        );
        assert_eq!(
            list(
                &["list".into(), "sid".into()],
                true,
                false,
                &ListFilters::default()
            )
            .unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn list_failed_filter_keeps_only_nonzero_exit() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "r1", "SUMMARY: 3/3 (PASS)", 0);
        write_audit(&jdir, "r2", "SUMMARY: 2/3 (FAIL)", 1);
        let filters = ListFilters {
            only_failed: true,
            ..Default::default()
        };
        assert_eq!(
            list(&["list".into(), "sid".into()], true, false, &filters).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn diff_returns_zero_for_identical_audits() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "a", "SUMMARY: 3/3 (PASS)", 0);
        write_audit(&jdir, "b", "SUMMARY: 3/3 (PASS)", 0);
        // 'b' was written with the same body modulo runId, but write_audit
        // embeds runId in the body, so the diff is non-empty. To get a
        // true 'identical' result we re-write 'a' equal to 'b'.
        std::fs::write(
            jdir.join("replays/a/audit.json"),
            std::fs::read_to_string(jdir.join("replays/b/audit.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            diff(&["diff".into(), "sid".into(), "a".into(), "b".into()]).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn diff_returns_one_for_different_audits() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "a", "SUMMARY: 3/3 (PASS)", 0);
        write_audit(&jdir, "b", "SUMMARY: 2/3 (FAIL)", 1);
        assert_eq!(
            diff(&["diff".into(), "sid".into(), "a".into(), "b".into()]).unwrap(),
            1
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn duration_prints_seconds() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "r1", "SUMMARY: 1/1 (PASS)", 0);
        assert_eq!(
            duration(&["duration".into(), "sid".into(), "r1".into()]).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn count_reports_number_of_runs() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        for i in 0..4 {
            write_audit(
                &jdir,
                &format!("2026-01-0{i}__h{i}"),
                "SUMMARY: 1/1 (PASS)",
                0,
            );
        }
        assert_eq!(count(&["count".into(), "sid".into()]).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn count_zero_when_no_replays() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        std::fs::create_dir_all(tmp.path().join("sid")).unwrap();
        assert_eq!(count(&["count".into(), "sid".into()]).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn field_prints_arbitrary_field() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "r1", "SUMMARY: 3/3 (PASS)", 0);
        assert_eq!(
            field(&["field".into(), "sid".into(), "r1".into(), "profile".into()]).unwrap(),
            0
        );
        assert_eq!(
            field(&[
                "field".into(),
                "sid".into(),
                "r1".into(),
                "parameters".into()
            ])
            .unwrap(),
            0
        );
        let err = field(&[
            "field".into(),
            "sid".into(),
            "r1".into(),
            "does-not-exist".into(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("not present"));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn exit_code_prints_audit_exit_code() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "r1", "SUMMARY: 3/3 (PASS)", 0);
        write_audit(&jdir, "r2", "SUMMARY: 2/3 (FAIL)", 1);
        assert_eq!(
            exit_code(&["exit-code".into(), "sid".into(), "r1".into()]).unwrap(),
            0
        );
        assert_eq!(
            exit_code(&["exit-code".into(), "sid".into(), "r2".into()]).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn cluster_groups_failures_by_normalized_signature() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());

        // Two scenarios, same underlying error modulo ids/numbers → ONE cluster.
        for (sid, run, step, err) in [
            (
                "sid-a",
                "r1",
                "s2",
                "locator miss: role=button name=\"Save 12 items\"",
            ),
            (
                "sid-b",
                "r9",
                "s1",
                "locator miss: role=button name=\"Save 3 items\"",
            ),
            // A different error — its own (sub-threshold) group.
            ("sid-b", "r9", "s4", "timeout waiting for navigation"),
        ] {
            let run_dir = tmp.path().join(sid).join("replays").join(run);
            std::fs::create_dir_all(&run_dir).unwrap();
            // Append — sid-b/r9 carries two fail rows.
            let line = format!(
                "{{\"idx\":1,\"total\":1,\"id\":\"{step}\",\"intent\":\"x\",\"kind\":\"do:click\",\"status\":\"fail\",\"error\":{}}}\n",
                serde_json::to_string(err).unwrap()
            );
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(run_dir.join("events.jsonl"))
                .unwrap();
            f.write_all(line.as_bytes()).unwrap();
        }

        let clusters = collect_clusters(tmp.path(), 2);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].count, 2);
        assert_eq!(
            clusters[0].scenarios,
            vec!["sid-a".to_string(), "sid-b".to_string()]
        );
        // Both raw errors retained per member.
        assert!(clusters[0].members.iter().any(|m| m.step_id == "s2"));
        assert!(clusters[0].members.iter().any(|m| m.step_id == "s1"));

        // min-size 1 also surfaces the lone timeout cluster.
        let all = collect_clusters(tmp.path(), 1);
        assert_eq!(all.len(), 2);

        assert!(
            error_signature("HTTP 404 on /api/users/82")
                == error_signature("http 7 on /api/users/9")
        );

        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]

    fn verdict_maps_pass_fix_block() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");

        // PASS: clean green run.
        write_audit(&jdir, "r-pass", "SUMMARY: 3/3 (PASS)", 0);
        assert_eq!(
            verdict(&["verdict".into(), "sid".into(), "r-pass".into()], true).unwrap(),
            0
        );

        // BLOCK: failed run.
        write_audit(&jdir, "r-block", "SUMMARY: 2/3 (FAIL)", 1);
        assert_eq!(
            verdict(&["verdict".into(), "sid".into(), "r-block".into()], true).unwrap(),
            1
        );

        // FIX via autoHealed: green run that self-corrected a locator.
        let run_dir = jdir.join("replays").join("r-fix-healed");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(
            run_dir.join("audit.json"),
            r#"{"runId":"r-fix-healed","summary":"SUMMARY: 3/3 (PASS)","exitCode":0,"autoHealed":["s2"]}"#,
        )
        .unwrap();
        assert_eq!(
            verdict(
                &["verdict".into(), "sid".into(), "r-fix-healed".into()],
                true
            )
            .unwrap(),
            2
        );

        // FIX via value-rejection row in heal.jsonl (not reflected in
        // audit.autoHealed).
        let run_dir = jdir.join("replays").join("r-fix-reject");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(
            run_dir.join("audit.json"),
            r#"{"runId":"r-fix-reject","summary":"SUMMARY: 3/3 (PASS)","exitCode":0}"#,
        )
        .unwrap();
        std::fs::write(
            run_dir.join("heal.jsonl"),
            "{\"schema\":\"heal-row/v1\",\"mode\":\"value-rejection\",\"stepId\":\"s4\"}\n",
        )
        .unwrap();
        assert_eq!(
            verdict(
                &["verdict".into(), "sid".into(), "r-fix-reject".into()],
                true
            )
            .unwrap(),
            2
        );

        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]

    fn summary_prints_audit_summary_line() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_audit(&jdir, "r1", "SUMMARY: 3/3 (PASS)", 0);
        assert_eq!(
            summary(&["summary".into(), "sid".into(), "r1".into()]).unwrap(),
            0
        );
        assert_eq!(
            summary(&["summary".into(), "sid".into(), "latest".into()]).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn list_limit_caps_to_most_recent_n() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        for i in 0..5 {
            write_audit(
                &jdir,
                &format!("2026-01-0{i}__h{i}"),
                "SUMMARY: 1/1 (PASS)",
                0,
            );
        }
        let filters = ListFilters {
            limit: Some(2),
            ..Default::default()
        };
        assert_eq!(
            list(&["list".into(), "sid".into()], true, false, &filters).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn stats_all_aggregates_across_scenarios() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        write_audit(&tmp.path().join("a"), "r1", "SUMMARY: 3/3 (PASS)", 0);
        write_audit(&tmp.path().join("a"), "r2", "SUMMARY: 2/3 (FAIL)", 1);
        write_audit(&tmp.path().join("b"), "r1", "SUMMARY: 3/3 (PASS)", 0);
        assert_eq!(stats_all(false, None, None).unwrap(), 0);
        assert_eq!(stats_all(true, None, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn flake_score_weights_flips_fails_and_heals() {
        // Perfectly stable: all pass, no heals.
        assert_eq!(flake_score(&[true, true, true], 0, 3), Some(100.0));
        // <2 scored runs — insufficient data.
        assert_eq!(flake_score(&[true], 0, 1), None);
        assert_eq!(flake_score(&[], 0, 0), None);
        // Constant alternation P/F/P/F over 4 runs: flip_rate=1,
        // fail_rate=0.5 → 100 - (50 + 15) = 35.
        let s = flake_score(&[true, false, true, false], 0, 4).unwrap();
        assert!((s - 35.0).abs() < 1e-9, "score {s}");
        // Healed-but-green runs degrade the score: 4 passes all healed →
        // heal_rate 1 → 100 - 20 = 80.
        let s = flake_score(&[true, true, true, true], 4, 4).unwrap();
        assert!((s - 80.0).abs() < 1e-9, "score {s}");
        // Never below zero.
        let s = flake_score(&[false, false], 2, 2).unwrap();
        assert!((0.0..=100.0).contains(&s));
    }

    #[test]
    fn stats_all_tolerates_empty_root() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path().join("empty"));
        assert_eq!(stats_all(false, None, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    fn write_events(dir: &std::path::Path, run_id: &str, outcomes: &[(&str, &str)]) {
        let run_dir = dir.join("replays").join(run_id);
        std::fs::create_dir_all(&run_dir).unwrap();
        let mut body = String::new();
        for (i, (id, status)) in outcomes.iter().enumerate() {
            let err = if *status == "fail" {
                r#","error":"boom""#
            } else {
                ""
            };
            body.push_str(&format!(
                r#"{{"idx":{},"total":{},"id":"{}","intent":"x","kind":"do:click","status":"{}"{}}}"#,
                i + 1,
                outcomes.len(),
                id,
                status,
                err
            ));
            body.push('\n');
        }
        std::fs::write(run_dir.join("events.jsonl"), body).unwrap();
    }

    #[test]
    fn flaky_flags_interleaved_outcome() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        // s1 interleaves P-F-P (2 flips) across 3 runs; s2 fails once and
        // stays failed (1 flip — a regression signature, not flake).
        write_events(&jdir, "2026-01-01__a", &[("s1", "pass"), ("s2", "pass")]);
        write_events(&jdir, "2026-01-02__b", &[("s1", "fail"), ("s2", "fail")]);
        write_events(&jdir, "2026-01-03__c", &[("s1", "pass"), ("s2", "fail")]);
        let out = collect_flaky(&jdir, 2, 3);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].step_id, "s1");
        assert_eq!(out[0].flips, 2);
        assert_eq!(out[0].seen, 3);
        assert_eq!(out[0].seq, "PFP");
        assert_eq!(out[0].fail_runs, vec!["2026-01-02__b".to_string()]);
        assert_eq!(out[0].last_error.as_deref(), Some("boom"));
        assert_eq!(
            flaky(&["flaky".into(), "sid".into()], true, 2, 3).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn flaky_json_reports_sequence_and_fail_runs() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        write_events(&jdir, "2026-01-01__a", &[("s1", "pass")]);
        write_events(&jdir, "2026-01-02__b", &[("s1", "fail")]);
        write_events(&jdir, "2026-01-03__c", &[("s1", "pass")]);
        write_events(&jdir, "2026-01-04__d", &[("s1", "fail")]);
        // heal row in one run gets counted for context
        let heal_dir = jdir.join("replays").join("2026-01-02__b");
        std::fs::write(
            heal_dir.join("heal.jsonl"),
            r#"{"stepId":"s1","mode":"locator-correction"}"#.to_string() + "\n",
        )
        .unwrap();
        let out = collect_flaky(&jdir, 2, 3);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seq, "PFPF");
        assert_eq!(out[0].flips, 3);
        assert_eq!(out[0].seen, 4);
        assert_eq!(out[0].heals, 1);
        assert_eq!(
            flaky(&["flaky".into(), "sid".into()], false, 2, 3).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn flaky_skips_runs_where_step_never_ran() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");
        // s2 absent in run b (early abort) — must not count as a flip.
        write_events(&jdir, "2026-01-01__a", &[("s1", "fail"), ("s2", "pass")]);
        write_events(&jdir, "2026-01-02__b", &[("s1", "fail")]);
        write_events(&jdir, "2026-01-03__c", &[("s1", "pass"), ("s2", "pass")]);
        // s1: F F P = 1 flip; s2: P (absent) P = 0 flips → neither flags.
        assert!(collect_flaky(&jdir, 2, 2).is_empty());
        // …but with min-flips=1, s1 does flag while absent-run s2 still doesn't.
        let out = collect_flaky(&jdir, 1, 2);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].step_id, "s1");
        assert_eq!(out[0].seen, 3);
        assert_eq!(
            flaky(&["flaky".into(), "sid".into()], true, 2, 2).unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    fn write_events_ms(dir: &std::path::Path, run_id: &str, rows: &[(&str, &str, u64)]) {
        let run_dir = dir.join("replays").join(run_id);
        std::fs::create_dir_all(&run_dir).unwrap();
        let mut body = String::new();
        for (i, (id, status, ms)) in rows.iter().enumerate() {
            body.push_str(&format!(
                r#"{{"idx":{},"total":{},"id":"{}","intent":"x","kind":"do:click","status":"{}","ms":{}}}"#,
                i + 1,
                rows.len(),
                id,
                status,
                ms
            ));
            body.push('\n');
        }
        std::fs::write(run_dir.join("events.jsonl"), body).unwrap();
    }

    #[test]
    fn slow_flags_duration_regression() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        // s1 regresses 400 → 900/1000ms in the last 2 runs (>50%, >250ms);
        // s2 stays steady; s3 jumps under the absolute floor (60 → 95ms).
        write_events_ms(
            &jdir,
            "r1",
            &[("s1", "pass", 400), ("s2", "pass", 500), ("s3", "pass", 60)],
        );
        write_events_ms(
            &jdir,
            "r2",
            &[("s1", "pass", 400), ("s2", "pass", 520), ("s3", "pass", 60)],
        );
        write_events_ms(
            &jdir,
            "r3",
            &[("s1", "pass", 900), ("s2", "pass", 500), ("s3", "pass", 95)],
        );
        write_events_ms(
            &jdir,
            "r4",
            &[
                ("s1", "pass", 1000),
                ("s2", "pass", 510),
                ("s3", "pass", 95),
            ],
        );
        let out = collect_slow(&jdir, 50.0, 250, 2, 3);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].step_id, "s1");
        assert_eq!(out[0].base_ms, 400.0);
        assert_eq!(out[0].recent_ms, 900.0);
        assert!(out[0].pct_delta > 100.0);
        assert_eq!(out[0].seen, 4);
    }

    #[test]
    fn slow_ignores_fail_rows_and_short_history() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        // s1's "slow" run is a fail (locator timeout 5000ms is the timeout
        // budget, not the step's cost) — pass-only history never regresses.
        write_events_ms(&jdir, "r1", &[("s1", "pass", 400)]);
        write_events_ms(&jdir, "r2", &[("s1", "pass", 400)]);
        write_events_ms(&jdir, "r3", &[("s1", "fail", 5000)]);
        write_events_ms(&jdir, "r4", &[("s1", "pass", 420)]);
        assert!(collect_slow(&jdir, 50.0, 250, 2, 3).is_empty());
        // A single slow pass can't self-flag (recent=2 window needs 2 rows).
        write_events_ms(&jdir, "r5", &[("s1", "pass", 2000)]);
        assert!(collect_slow(&jdir, 50.0, 250, 2, 3).is_empty());
        // …but two consecutive slow passes do.
        write_events_ms(&jdir, "r6", &[("s1", "pass", 2100)]);
        let out = collect_slow(&jdir, 50.0, 250, 2, 3);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].fails, 1);
    }

    #[test]
    fn slow_respects_pct_and_min_ms_thresholds() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        write_events_ms(&jdir, "r1", &[("s1", "pass", 400)]);
        write_events_ms(&jdir, "r2", &[("s1", "pass", 400)]);
        write_events_ms(&jdir, "r3", &[("s1", "pass", 700)]);
        write_events_ms(&jdir, "r4", &[("s1", "pass", 700)]);
        // +75% > default 50% but the 300ms delta clears min-ms too.
        assert_eq!(collect_slow(&jdir, 50.0, 250, 2, 3).len(), 1);
        // Raise the pct bar past +75% → nothing flags.
        assert!(collect_slow(&jdir, 80.0, 250, 2, 3).is_empty());
        // Raise the absolute floor past the 300ms delta → nothing flags.
        assert!(collect_slow(&jdir, 50.0, 400, 2, 3).is_empty());
    }

    #[test]
    fn health_rolls_up_the_three_detectors() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        // sid-a: flaky s1 (P-F-P) + chronic s2 (healed in 2 runs).
        let a = tmp.path().join("sid-a");
        write_events(&a, "r1", &[("s1", "pass"), ("s2", "pass")]);
        write_events(&a, "r2", &[("s1", "fail"), ("s2", "pass")]);
        write_events(&a, "r3", &[("s1", "pass"), ("s2", "pass")]);
        for r in ["r1", "r2"] {
            std::fs::write(
                a.join("replays").join(r).join("heal.jsonl"),
                "{\"stepId\":\"s2\",\"mode\":\"locator-correction\"}\n",
            )
            .unwrap();
        }
        // sid-b: clean runs — must not appear.
        let b = tmp.path().join("sid-b");
        write_events(&b, "r1", &[("s1", "pass")]);
        write_events(&b, "r2", &[("s1", "pass")]);
        write_events(&b, "r3", &[("s1", "pass")]);
        // sid-c: slow s9 (400 → 900/1000 in the last two runs).
        let c = tmp.path().join("sid-c");
        write_events_ms(&c, "r1", &[("s9", "pass", 400)]);
        write_events_ms(&c, "r2", &[("s9", "pass", 400)]);
        write_events_ms(&c, "r3", &[("s9", "pass", 900)]);
        write_events_ms(&c, "r4", &[("s9", "pass", 1000)]);
        let out = collect_health(tmp.path());
        assert_eq!(out.len(), 2);
        let a_row = out.iter().find(|r| r.scenario_id == "sid-a").unwrap();
        assert_eq!(a_row.flaky, vec!["s1"]);
        assert_eq!(a_row.chronic, vec!["s2"]);
        assert!(a_row.slow.is_empty());
        let c_row = out.iter().find(|r| r.scenario_id == "sid-c").unwrap();
        assert_eq!(c_row.slow, vec!["s9"]);
        assert!(c_row.flaky.is_empty() && c_row.chronic.is_empty());
    }

    fn write_audit_ms(dir: &std::path::Path, run_id: &str, exit: i64, dur_ms: u64) {
        let run_dir = dir.join("replays").join(run_id);
        std::fs::create_dir_all(&run_dir).unwrap();
        let body = format!(
            r#"{{"schema":"scenario-replay-audit/v1","runId":"{run_id}","scenarioId":"j","startedAt":"2026-01-01T00:00:00.000Z","finishedAt":"2026-01-01T00:00:{secs:02}.000Z","summary":"SUMMARY: x","exitCode":{exit}}}"#,
            secs = dur_ms / 1000
        );
        std::fs::write(run_dir.join("audit.json"), body).unwrap();
    }

    #[test]
    fn trend_reports_outcomes_median_and_sparkline() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        write_audit_ms(&jdir, "2026-01-01__a", 0, 2000);
        write_audit_ms(&jdir, "2026-01-02__b", 1, 5000);
        write_audit_ms(&jdir, "2026-01-03__c", 0, 3000);
        write_audit_ms(&jdir, "2026-01-04__d", 0, 2000);
        let out = collect_trend(&jdir, "sid", None);
        assert_eq!(out.passed, 3);
        assert_eq!(out.failed, 1);
        assert_eq!(out.outcomes, "✓✗✓✓");
        assert_eq!(out.median_secs, 2.5);
        // Durations 2,5,3,2 → min 2 max 5: ▁, █, ▃(idx2), ▁.
        assert_eq!(out.sparkline, "▁█▃▁");
    }

    #[test]
    fn trend_limit_keeps_latest_runs_and_skips_auditless_dirs() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        write_audit_ms(&jdir, "2026-01-01__a", 0, 1000);
        write_audit_ms(&jdir, "2026-01-02__b", 0, 1000);
        write_audit_ms(&jdir, "2026-01-03__c", 0, 1000);
        // A partial run (events written, audit never landed) still lists
        // but contributes no outcome.
        let partial = jdir.join("replays").join("2026-01-04__d");
        std::fs::create_dir_all(&partial).unwrap();
        std::fs::write(partial.join("events.jsonl"), "").unwrap();
        let out = collect_trend(&jdir, "sid", Some(2));
        assert_eq!(out.runs.len(), 2);
        assert_eq!(out.runs[0].run_id, "2026-01-03__c");
        assert_eq!(out.passed, 1);
        assert_eq!(out.outcomes, "✓");
        // Runs without audit.json are skipped in both lines — no misalignment.
        assert_eq!(out.sparkline, "▄");
    }

    #[test]
    fn trend_all_rows_every_scenario_with_replays() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("sid-a");
        write_audit_ms(&a, "2026-01-01__a", 0, 1000);
        write_audit_ms(&a, "2026-01-02__b", 1, 4000);
        let b = tmp.path().join("sid-b");
        write_audit_ms(&b, "2026-01-01__a", 0, 9000);
        // A scenario dir with no replays/ contributes no row.
        std::fs::create_dir_all(tmp.path().join("sid-c")).unwrap();
        let rows = collect_trend_all(tmp.path(), None);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].scenario_id, "sid-a");
        assert_eq!(rows[0].outcomes, "✓✗");
        assert_eq!(rows[1].scenario_id, "sid-b");
        assert_eq!(rows[1].median_secs, 9.0);
    }

    #[test]
    fn explain_exits_with_the_verdict_code_and_tolerates_sparse_runs() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let jdir = tmp.path().join("sid");

        // BLOCK: a failed run → exit 1.
        write_audit(&jdir, "r-block", "SUMMARY: 2/3 (FAIL)", 1);
        write_events(&jdir, "r-block", &[("s1", "pass"), ("s2", "fail")]);
        assert_eq!(
            explain(&["explain".into(), "sid".into()], false).unwrap(),
            1
        );
        // --json path prints the digest and still mirrors the verdict code.
        assert_eq!(explain(&["explain".into(), "sid".into()], true).unwrap(), 1);

        // PASS: a clean green run → exit 0 even with no events/heals/console/
        // network artifacts at all.
        write_audit(&jdir, "r-pass", "SUMMARY: 3/3 (PASS)", 0);
        assert_eq!(
            explain(&["explain".into(), "sid".into(), "r-pass".into()], false).unwrap(),
            0
        );

        // FIX: green run with a heal row → exit 2 (verdict code).
        let heal_dir = jdir.join("replays").join("r-fix");
        std::fs::create_dir_all(&heal_dir).unwrap();
        std::fs::write(
            heal_dir.join("audit.json"),
            r#"{"runId":"r-fix","summary":"SUMMARY: 3/3 (PASS)","exitCode":0}"#,
        )
        .unwrap();
        std::fs::write(
            heal_dir.join("heal.jsonl"),
            r#"{"stepId":"s1","mode":"locator-correction","strategy":"exact","from":"a","to":"b"}"#
                .to_string()
                + "\n",
        )
        .unwrap();
        assert_eq!(
            explain(&["explain".into(), "sid".into(), "r-fix".into()], false).unwrap(),
            2
        );

        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }
}
