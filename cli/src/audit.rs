//! `audit show <sid> <runId | latest>` — pretty-print one replay's audit.json.
//!
//! Useful when investigating a single failure without reaching for a JSON
//! viewer; complements `list <sid>` which only shows summary rows.

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::paths;

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
    let mut until_ms: Option<u64> = None;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            "--json" => json_out = true,
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
        other => bail!(
            "unknown audit subverb {other:?} (try: show | list | stats | stats-all | diff | summary | exit-code | field | count | duration)"
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
        .ok_or_else(|| anyhow!("usage: audit show <sid> <runId | latest>"))?;
    let run_ref = positionals
        .get(2)
        .ok_or_else(|| anyhow!("usage: audit show <sid> <runId | latest>"))?;
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
    let mut runs: Vec<std::path::PathBuf> = match fs::read_dir(&replays_dir) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    runs.sort();

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
    let mut runs: Vec<std::path::PathBuf> = match fs::read_dir(&replays_dir) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    runs.sort();

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
                last_pass = Some(run_id.clone());
            }
            Some(_) => {
                failures += 1;
                last_fail = Some(run_id.clone());
            }
            None => unknown += 1,
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
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(0);
    }

    println!("audit stats: {} ({total} run(s))", replays_dir.display());
    println!("  passes        : {passes}");
    println!("  failures      : {failures}");
    println!("  unknown       : {unknown}");
    println!("  pass rate     : {:.0}%", pass_rate * 100.0);
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
        .ok_or_else(|| anyhow!("usage: audit duration <sid> <runId | latest>"))?;
    let run_ref = positionals
        .get(2)
        .ok_or_else(|| anyhow!("usage: audit duration <sid> <runId | latest>"))?;
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
    let n = match fs::read_dir(&replays_dir) {
        Ok(it) => it.flatten().filter(|e| e.path().is_dir()).count(),
        Err(_) => 0,
    };
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
        .ok_or_else(|| anyhow!("usage: audit exit-code <sid> <runId | latest>"))?;
    let run_ref = positionals
        .get(2)
        .ok_or_else(|| anyhow!("usage: audit exit-code <sid> <runId | latest>"))?;
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

fn summary(positionals: &[String]) -> Result<u8> {
    let sid = positionals
        .get(1)
        .ok_or_else(|| anyhow!("usage: audit summary <sid> <runId | latest>"))?;
    let run_ref = positionals
        .get(2)
        .ok_or_else(|| anyhow!("usage: audit summary <sid> <runId | latest>"))?;
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
        let mut runs: Vec<std::path::PathBuf> = match fs::read_dir(&replays_dir) {
            Ok(it) => it
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect(),
            Err(_) => Vec::new(),
        };
        runs.sort();
        let (mut total, mut passes, mut failures, mut unknown) = (0u32, 0u32, 0u32, 0u32);
        let mut last_pass: Option<String> = None;
        let mut last_fail: Option<String> = None;
        let mut dur_ms_total: u64 = 0;
        let mut dur_ms_count: u64 = 0;
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
                    last_pass = Some(run_id.clone());
                }
                Some(_) => {
                    failures += 1;
                    last_fail = Some(run_id.clone());
                }
                None => unknown += 1,
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
    println!("{sid_h:<24}  {total_h:>5}  {pass_h:>5}  {fail_h:>5}  {rate_h:>5}");
    for r in &rows {
        let sid = &r.sid;
        let total = r.total;
        let pass = r.passes;
        let fail = r.failures;
        let rate = format!("{:.0}%", r.pass_rate * 100.0);
        println!("{sid:<24}  {total:>5}  {pass:>5}  {fail:>5}  {rate:>5}");
    }
    let rate = format!("{:.0}%", overall_pass_rate * 100.0);
    println!(
        "\nOVERALL: total={tot_total} pass={tot_passes} fail={tot_failures} unknown={tot_unknown} rate={rate} avg_duration={overall_avg_duration_secs:.3}s"
    );
    Ok(0)
}

fn resolve_run_id(scenario_dir: &std::path::Path, run_ref: &str) -> Result<String> {
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
    let mut entries: Vec<PathBuf> = match fs::read_dir(scenario_dir.join("replays")) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    entries.sort();
    let last = entries
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
                "agent-qa audit \u{2014} inspect a replay's audit.json\n\nUsage:\n  agent-qa audit show <sid> <runId | latest> [--json | --format text|json|github]\n  agent-qa audit list <sid>                    Table view: every run's\n                                               summary / exit / profile / tag\n  agent-qa audit list <sid> --json             Structured rows on stdout\n  agent-qa audit list <sid> [--passed | --failed] [--tag <pat>] [--profile <pat>] [--limit N] [--slow <secs>] [--sort duration|runId-desc] [--since <iso-ts>] [--until <iso-ts>] [--format text|json|github]\n                                               Filters: case-insensitive substring\n                                               --passed/--failed are exit-code partitions\n  agent-qa audit stats <sid> [--since <iso-ts>] [--until <iso-ts>]\n                                               Pass/fail/tag rollup for one scenario\n  agent-qa audit stats <sid> --json            Structured rollup on stdout\n  agent-qa audit stats-all                     Per-scenario + overall pass/fail rollup\n  agent-qa audit stats-all --json              Structured rollup on stdout\n  agent-qa audit stats-all [--since <iso-ts>] [--until <iso-ts>]\n                                               Constrain to a date window\n  agent-qa audit diff <sid> <runIdA> <runIdB>  Unified diff between two replays'\n                                               audit.json (canonicalised JSON;\n                                               'latest' accepted for either side;\n                                               exit 1 on difference)\n  agent-qa audit summary <sid> <runId | latest>\n                                               Print just the summary line (one line out)\n  agent-qa audit exit-code <sid> <runId | latest>\n                                               Print just the run's exitCode (-1 if missing)\n  agent-qa audit field <sid> <runId | latest> <fieldName>\n                                               Print any top-level audit field. String/\n                                               number/bool print verbatim; null prints\n                                               empty; object/array prints compact JSON.\n  agent-qa audit count <sid>                   Print the number of runs under <sid>\n  agent-qa audit duration <sid> <runId | latest>\n                                               Print the run's duration in seconds\n                                               (finishedAt - startedAt, 3 decimals)\n\n'latest' resolves to <sid>/replays/latest.txt if present, otherwise the\nhighest lex-sorted run directory (run_id is timestamp-prefixed)."
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
}
