//! `domshot-accept` verb — mint ARIA-snapshot baselines for `{"domshot"}`
//! claims.
//!
//! Copies `<sid>/replays/<run>/snapshots/<stepId>.txt` into
//! `<sid>/baselines/<stepId>.snap.txt` so subsequent replays' domshot
//! claims compare against a known-good structural snapshot. Default run
//! is `replays/latest.txt`; default step set is every snapshot the run
//! captured.
//!
//! CLI shape:
//!
//!   agent-qa domshot-accept <sid> [--run <runId>] [--steps <csv>] [--json]
//!
//! Idempotent — re-running after a legitimate UI change re-mints the
//! baseline. `--json` emits the minted step list for automation.

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let mut sid: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut steps: Option<Vec<String>> = None;
    let mut json = false;
    let mut dry_run = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--run" => {
                i += 1;
                run_id = Some(args.get(i).context("--run needs a value")?.clone());
            }
            "--steps" => {
                i += 1;
                let raw = args.get(i).context("--steps needs a value")?;
                steps = Some(
                    raw.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            "--json" => json = true,
            "--dry-run" | "-n" => dry_run = true,
            "--help" | "-h" => {
                println!("agent-qa domshot-accept — mint ARIA-snapshot baselines from a run\n\nUsage:\n  agent-qa domshot-accept <sid> [--run <runId>] [--steps <csv>] [--json] [--dry-run]\n\nCopies <sid>/replays/<run>/snapshots/<stepId>.txt to\n<sid>/baselines/<stepId>.snap.txt for {{\"domshot\"}} claims.\nDefaults: latest run, every step a domshot claim references. --dry-run previews:\nper step it reports new / identical / update (with the changed-line\ncount) and writes nothing.");
                return Ok(0);
            }
            v if sid.is_none() => sid = Some(v.to_string()),
            v => bail!("domshot-accept: unknown arg {v:?}"),
        }
        i += 1;
    }

    let sid = sid.context("domshot-accept requires <sid>")?;
    let sdir = paths::scenarios_root().join(&sid);
    let rid = match run_id {
        Some(r) => r,
        None => fs::read_to_string(sdir.join("replays").join("latest.txt"))
            .context("no --run and replays/latest.txt is missing — replay first")?
            .trim()
            .to_string(),
    };
    let minted = mint_domshot_baselines(&sdir, &rid, steps, dry_run, json)?;
    if dry_run {
        return Ok(0);
    }
    // Push minted goldens to the configured [baselines] remote store —
    // warn-only so a remote outage never fails a successful mint.
    match crate::golden_store::sync_out(&sdir) {
        Ok(Some(r)) if !r.moved.is_empty() => {
            eprintln!("baselines: pushed {} file(s) to {}", r.moved.len(), r.store)
        }
        Ok(_) => {}
        Err(e) => eprintln!("baselines: push skipped ({e:#})"),
    }

    if json {
        println!("{}", serde_json::to_string(&minted)?);
    } else {
        println!(
            "minted {} domshot baseline(s) under {} from run {rid}: {}",
            minted.len(),
            sdir.join("baselines").display(),
            minted.join(", ")
        );
    }
    Ok(0)
}

/// Copy `<scenario_dir>/replays/<rid>/snapshots/<step>.txt` to
/// `<scenario_dir>/baselines/<step>.snap.txt`.
pub(crate) fn mint_domshot_baselines(
    scenario_dir: &Path,
    rid: &str,
    steps: Option<Vec<String>>,
    dry_run: bool,
    json: bool,
) -> Result<Vec<String>> {
    let snaps_dir = scenario_dir.join("replays").join(rid).join("snapshots");
    if !snaps_dir.is_dir() {
        bail!("no snapshots dir at {}", snaps_dir.display());
    }

    // Stale-run guard: the audit records the scenario's content hash; if
    // scenario.json changed since this run, the captures describe a
    // different scenario — warn (re-minting still proceeds).
    let run_dir = scenario_dir.join("replays").join(rid);
    if let Ok(audit_bytes) = fs::read(run_dir.join("audit.json")) {
        if let Ok(audit) = serde_json::from_slice::<serde_json::Value>(&audit_bytes) {
            if let Some(recorded) = audit.get("scenarioContentHash").and_then(|v| v.as_str()) {
                if let Ok(current) = fs::read(scenario_dir.join("scenario.json")) {
                    let now = crate::sidecar::hash_scenario_bytes(&current);
                    if recorded != now {
                        eprintln!(
                            "warning: scenario.json changed since run {rid} — baselines minted from it may not match what you replay next"
                        );
                    }
                }
            }
        }
    }

    // Default step set: the ids domshot claims reference. Snapshot
    // sidecars exist for EVERY step — including check steps — so minting
    // the whole dir would create orphan baselines (lint flags them).
    // --steps overrides for minting something the claims don't cover.
    let step_ids: Vec<String> = match steps {
        Some(v) => v,
        None => claimed_domshot_ids(scenario_dir),
    };
    if step_ids.is_empty() {
        bail!(
            "no domshot claims in {} — pass --steps to mint anyway",
            scenario_dir.join("scenario.json").display()
        );
    }

    let base_dir = scenario_dir.join("baselines");

    if dry_run {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Plan {
            step: String,
            action: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            changed_lines: Option<usize>,
        }
        let mut plans: Vec<Plan> = Vec::new();
        for id in &step_ids {
            let src = snaps_dir.join(format!("{id}.txt"));
            if !src.is_file() {
                bail!("run {rid} has no snapshot for step '{id}'");
            }
            let dst = base_dir.join(format!("{id}.snap.txt"));
            let plan = if !dst.is_file() {
                Plan {
                    step: id.clone(),
                    action: "new",
                    changed_lines: None,
                }
            } else if fs::read(&dst)? == fs::read(&src)? {
                Plan {
                    step: id.clone(),
                    action: "identical",
                    changed_lines: None,
                }
            } else {
                let a = fs::read_to_string(&dst)?;
                let b = fs::read_to_string(&src)?;
                let diff = similar::TextDiff::from_lines(&a, &b)
                    .unified_diff()
                    .to_string();
                let changed = diff
                    .lines()
                    .filter(|l| l.starts_with('+') || l.starts_with('-'))
                    .filter(|l| !l.starts_with("+++") && !l.starts_with("---"))
                    .count();
                Plan {
                    step: id.clone(),
                    action: "update",
                    changed_lines: Some(changed),
                }
            };
            plans.push(plan);
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&plans)?);
        } else {
            for p in &plans {
                match p.changed_lines {
                    Some(d) => println!("  {:<9} {} ({} changed lines)", p.action, p.step, d),
                    None => println!("  {:<9} {}", p.action, p.step),
                }
            }
            println!(
                "dry-run: {} domshot baseline(s) would change under {} from run {rid}",
                plans.iter().filter(|p| p.action != "identical").count(),
                base_dir.display()
            );
        }
        return Ok(Vec::new());
    }

    fs::create_dir_all(&base_dir)?;
    let mut minted: Vec<String> = Vec::new();
    for id in &step_ids {
        let src = snaps_dir.join(format!("{id}.txt"));
        if !src.is_file() {
            bail!("run {rid} has no snapshot for step '{id}'");
        }
        fs::copy(&src, base_dir.join(format!("{id}.snap.txt")))?;
        minted.push(id.clone());
    }
    Ok(minted)
}

/// Collect the step ids `{"domshot": "<id>"}` claims reference — the
/// mintable set. Walks raw JSON (nested group/loop steps count) without
/// depending on the typed Scenario shape.
fn claimed_domshot_ids(scenario_dir: &Path) -> Vec<String> {
    let Ok(bytes) = fs::read(scenario_dir.join("scenario.json")) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(map) => {
                if let Some(subject) = map.get("claim").and_then(|c| c.get("subject")) {
                    if let Some(sid) = subject.get("domshot").and_then(|s| s.as_str()) {
                        out.push(sid.to_string());
                    }
                }
                for item in map.values() {
                    walk(item, out);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(&v, &mut out);
    out.sort();
    out.dedup();
    out
}
