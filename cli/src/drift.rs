//! Known-drift ledger — suppress a previously triaged visual drift.
//!
//! When a `shot` claim misses, the diff's region signature is hashed into
//! a fingerprint (`drift_fingerprint` in compare). If the claim opts in
//! (`tolerance.knownDrift: true`) and the fingerprint is listed in the
//! scenario's `known-drift.json`, the miss is downgraded to a warning —
//! the same drift a human already reviewed — while any NEW fingerprint
//! still fails. This is what stops a known-flaky visual element (a clock,
//! an avatar with subpixel AA churn) from re-failing every run without
//! hiding a real regression elsewhere in the same screenshot.
//!
//! Ledger file: `<scenario-dir>/known-drift.json`
//! ```json
//! { "drift": { "<fp16>": { "note": "avatar AA", "run": "r-…", "at": "…" } } }
//! ```
//! Minted by `agent-qa known-drift accept <sid> <runId> <stepId>`.

use anyhow::{Context, Result};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn ledger_path(scenario_dir: &Path) -> PathBuf {
    scenario_dir.join("known-drift.json")
}

/// Read the ledger's fingerprint map (`fp → {note, run, at}`). Missing /
/// malformed file = empty map — a typo'd ledger never breaks a run.
pub fn fingerprints(scenario_dir: &Path) -> BTreeMap<String, Json> {
    let path = ledger_path(scenario_dir);
    let Ok(body) = fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    serde_json::from_str::<Json>(&body)
        .ok()
        .and_then(|v| v.get("drift")?.as_object().cloned())
        .map(|m| m.into_iter().collect())
        .unwrap_or_default()
}

/// `tolerance.knownDrift` opt-in flag on a shot claim.
pub fn opt_in(tolerance: Option<&BTreeMap<String, Json>>) -> bool {
    tolerance
        .and_then(|t| t.get("knownDrift"))
        .map(|v| v.as_bool().unwrap_or(false))
        .unwrap_or(false)
}

/// `known-drift accept <sid> <runId> <stepId>` — record this run's diff
/// fingerprint for `<stepId>` in the ledger, so future runs that opt in
/// (`tolerance.knownDrift`) treat the identical drift as triaged.
/// Recomputes the fingerprint from the run's own diff PNG, so it can
/// only ever suppress what a run actually produced.
pub fn run(args: &[String]) -> Result<u8> {
    // accept <sid> <runId> <stepId> | list <sid>
    let sub = args.first().map(String::as_str).unwrap_or("");
    match sub {
        "accept" => {
            let sid = args
                .get(1)
                .context("usage: known-drift accept <sid> <runId> <stepId>")?;
            let run_id = args.get(2).context("missing runId")?;
            let step = args.get(3).context("missing stepId")?;
            let scenario_dir = crate::paths::scenario_dir(sid)?;
            let (run_dir, resolved_id) = crate::audit::resolve_run_id(&scenario_dir, run_id)
                .map(|id| (scenario_dir.join("replays").join(&id), id))
                .or_else(|_| {
                    // runId may already be a dir name under replays/
                    let d = scenario_dir.join("replays").join(run_id);
                    if d.is_dir() {
                        Ok((d, run_id.clone()))
                    } else {
                        Err(anyhow::anyhow!("run {run_id:?} not found"))
                    }
                })?;
            let diff_path = run_dir.join("shots-diff").join(format!("{step}.diff.png"));
            if !diff_path.is_file() {
                anyhow::bail!(
                    "no diff for step {step:?} in run {run_id:?} ({})",
                    diff_path.display()
                );
            }
            let diff = crate::compare::screenshots::decode_png(&diff_path)?;
            let regions = crate::compare::screenshots::diff_regions(&diff);
            let fp = crate::compare::screenshots::drift_fingerprint(
                &regions,
                diff.width(),
                diff.height(),
            );
            let path = ledger_path(&scenario_dir);
            let mut root: Json = fs::read_to_string(&path)
                .ok()
                .and_then(|b| serde_json::from_str(&b).ok())
                .unwrap_or_else(|| json!({ "drift": {} }));
            let drift = root
                .as_object_mut()
                .context("known-drift.json is not an object")?
                .entry("drift")
                .or_insert_with(|| json!({}));
            let map = drift.as_object_mut().context("'drift' is not an object")?;
            let existed = map.contains_key(&fp);
            map.insert(
                fp.clone(),
                json!({
                    "run": resolved_id,
                    "step": step,
                    "at": chrono::Utc::now().to_rfc3339(),
                }),
            );
            fs::write(&path, serde_json::to_string_pretty(&root)? + "\n")?;
            println!(
                "{} fingerprint {fp} for {sid}/{step} — {}/{} regions hashed",
                if existed { "re-accepted" } else { "accepted" },
                step,
                regions.len()
            );
            Ok(0)
        }
        "list" => {
            let sid = args.get(1).context("usage: known-drift list <sid>")?;
            let scenario_dir = crate::paths::scenario_dir(sid)?;
            let fps = fingerprints(&scenario_dir);
            if fps.is_empty() {
                println!("{sid}: no known drift");
                return Ok(0);
            }
            for (fp, meta) in &fps {
                let run = meta.get("run").and_then(|v| v.as_str()).unwrap_or("?");
                let step = meta.get("step").and_then(|v| v.as_str()).unwrap_or("?");
                let at = meta.get("at").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {fp}  step={step}  run={run}  at={at}");
            }
            Ok(0)
        }
        _ => {
            eprintln!("usage: known-drift accept <sid> <runId> <stepId> | list <sid>");
            Ok(2)
        }
    }
}
