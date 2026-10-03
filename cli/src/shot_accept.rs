//! `shot-accept` verb — mint screenshot baselines for `{"shot"}` claims.
//!
//! Copies `<sid>/replays/<run>/screenshots/<stepId>.png` into
//! `<sid>/baselines/<stepId>.png` so subsequent replays' shot claims
//! compare against a known-good frame. Default run is `replays/
//! latest.txt`; default step set is every screenshot the run captured.
//!
//! CLI shape:
//!
//!   agent-qa shot-accept <sid> [--run <runId>] [--steps <csv>] [--json]
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
                println!("agent-qa shot-accept — mint screenshot baselines from a run\n\nUsage:\n  agent-qa shot-accept <sid> [--run <runId>] [--steps <csv>] [--json] [--dry-run]\n\nCopies <sid>/replays/<run>/screenshots/<stepId>.png to\n<sid>/baselines/<stepId>.png for {{\"shot\"}} claims. Defaults:\nlatest run, every captured screenshot. --dry-run previews:\nper step it reports new / identical / update (with the AA-filtered\npixel diff%) and writes nothing.");
                return Ok(0);
            }
            v if sid.is_none() => sid = Some(v.to_string()),
            v => bail!("shot-accept: unknown arg {v:?}"),
        }
        i += 1;
    }

    let sid = sid.context("shot-accept requires <sid>")?;
    let sdir = paths::scenarios_root().join(&sid);
    let rid = match run_id {
        Some(r) => r,
        None => fs::read_to_string(sdir.join("replays").join("latest.txt"))
            .context("no --run and replays/latest.txt is missing — replay first")?
            .trim()
            .to_string(),
    };
    let minted = mint_baselines(&sdir, &rid, steps, dry_run, json)?;
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
            "minted {} baseline(s) under {} from run {rid}: {}",
            minted.len(),
            sdir.join("baselines").display(),
            minted.join(", ")
        );
    }
    Ok(0)
}

/// Copy `<scenario_dir>/replays/<rid>/screenshots/<step>.png` to
/// `<scenario_dir>/baselines/<step>.png`. Shared with `replay
/// --update-baselines` — both mint from a finished run's capture set.
pub(crate) fn mint_baselines(
    scenario_dir: &Path,
    rid: &str,
    steps: Option<Vec<String>>,
    dry_run: bool,
    json: bool,
) -> Result<Vec<String>> {
    let shots_dir = scenario_dir.join("replays").join(rid).join("screenshots");
    if !shots_dir.is_dir() {
        bail!("no screenshots dir at {}", shots_dir.display());
    }

    // Stale-run guard: the audit records the scenario's content hash; if
    // scenario.json changed since this run, the captures describe a different
    // scenario — warn (re-minting still proceeds; --dry-run keeps it advisory).
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

    let step_ids: Vec<String> = match steps {
        Some(v) => v,
        None => {
            let mut ids: Vec<String> = fs::read_dir(&shots_dir)?
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension().is_some_and(|x| x == "png"))
                        .then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
                })
                .collect();
            ids.sort();
            ids
        }
    };
    if step_ids.is_empty() {
        bail!("no screenshots to mint in {rid}");
    }

    let base_dir = scenario_dir.join("baselines");

    if dry_run {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Plan {
            step: String,
            action: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            diff_pct: Option<f64>,
        }
        let mut plans: Vec<Plan> = Vec::new();
        for id in &step_ids {
            let src = shots_dir.join(format!("{id}.png"));
            if !src.is_file() {
                bail!("run {rid} has no screenshot for step '{id}'");
            }
            let dst = base_dir.join(format!("{id}.png"));
            let plan = if !dst.is_file() {
                Plan {
                    step: id.clone(),
                    action: "new",
                    diff_pct: None,
                }
            } else {
                let a = crate::compare::screenshots::decode_png(&dst)
                    .with_context(|| format!("decode {}", dst.display()))?;
                let b = crate::compare::screenshots::decode_png(&src)
                    .with_context(|| format!("decode {}", src.display()))?;
                if a.dimensions() != b.dimensions() {
                    Plan {
                        step: id.clone(),
                        action: "update",
                        diff_pct: Some(100.0),
                    }
                } else {
                    let (frac, _) = crate::compare::screenshots::pixel_diff(&a, &b);
                    if frac == 0.0 {
                        Plan {
                            step: id.clone(),
                            action: "identical",
                            diff_pct: None,
                        }
                    } else {
                        Plan {
                            step: id.clone(),
                            action: "update",
                            diff_pct: Some((frac * 100.0 * 100.0).round() / 100.0),
                        }
                    }
                }
            };
            plans.push(plan);
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&plans)?);
        } else {
            for p in &plans {
                match p.diff_pct {
                    Some(d) => println!("  {:<9} {} ({}% differs)", p.action, p.step, d),
                    None => println!("  {:<9} {}", p.action, p.step),
                }
            }
            println!(
                "dry-run: {} baseline(s) would change under {} from run {rid}",
                plans.iter().filter(|p| p.action != "identical").count(),
                base_dir.display()
            );
        }
        return Ok(Vec::new());
    }

    fs::create_dir_all(&base_dir)?;
    let mut minted: Vec<String> = Vec::new();
    for id in &step_ids {
        let src = shots_dir.join(format!("{id}.png"));
        if !src.is_file() {
            bail!("run {rid} has no screenshot for step '{id}'");
        }
        fs::copy(&src, base_dir.join(format!("{id}.png")))?;
        minted.push(id.clone());
    }
    Ok(minted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// `scenarios_root()` defaults to `<cwd>/tmp/agent-qa-scenarios` — build
    /// fixtures there after cd-ing into the tempdir (under the env lock so
    /// the cwd mutation doesn't race other tests).
    fn sroot(root: &std::path::Path, sid: &str) -> PathBuf {
        root.join("tmp/agent-qa-scenarios").join(sid)
    }

    #[test]
    fn accepts_latest_run_and_all_shots_by_default() {
        let _g = crate::test_util::lock_env();
        let root = tempfile::tempdir().unwrap();
        let sid = "s-shot";
        let run_dir = sroot(root.path(), sid)
            .join("replays")
            .join("r1")
            .join("screenshots");
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(run_dir.join("a.png"), b"pngA").unwrap();
        fs::write(run_dir.join("b.png"), b"pngB").unwrap();
        fs::write(
            sroot(root.path(), sid).join("replays").join("latest.txt"),
            "r1",
        )
        .unwrap();

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        let rc = run(&[sid.to_string(), "--json".to_string()]).unwrap();
        std::env::set_current_dir(cwd).unwrap();

        assert_eq!(rc, 0);
        let base = sroot(root.path(), sid).join("baselines");
        assert_eq!(fs::read(base.join("a.png")).unwrap(), b"pngA");
        assert_eq!(fs::read(base.join("b.png")).unwrap(), b"pngB");
    }

    #[test]
    fn stale_scenario_still_mints_but_warns() {
        let _g = crate::test_util::lock_env();
        let root = tempfile::tempdir().unwrap();
        let sid = "s-shot-stale";
        let sdir = sroot(root.path(), sid);
        let run_dir = sdir.join("replays").join("r1").join("screenshots");
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(run_dir.join("a.png"), b"pngA").unwrap();
        fs::write(sdir.join("replays").join("latest.txt"), "r1").unwrap();
        // audit recorded a DIFFERENT scenario hash than the current file
        fs::write(
            sdir.join("replays").join("r1").join("audit.json"),
            r#"{"schema":"scenario-replay-audit/v1","scenarioContentHash":"stale"}"#,
        )
        .unwrap();
        fs::write(sdir.join("scenario.json"), br#"{"id":"x"}"#).unwrap();

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        let rc = run(&[sid.to_string()]).unwrap();
        std::env::set_current_dir(cwd).unwrap();
        assert_eq!(rc, 0); // warn-only — the mint still lands
        assert_eq!(
            fs::read(sdir.join("baselines").join("a.png")).unwrap(),
            b"pngA"
        );
    }

    #[test]
    fn dry_run_plans_new_identical_and_update() {
        let _g = crate::test_util::lock_env();
        let root = tempfile::tempdir().unwrap();
        let sid = "s-shot-dry";
        let run_dir = sroot(root.path(), sid)
            .join("replays")
            .join("r1")
            .join("screenshots");
        fs::create_dir_all(&run_dir).unwrap();
        let baselines = sroot(root.path(), sid).join("baselines");
        fs::create_dir_all(&baselines).unwrap();

        let white = || {
            let mut img = image::RgbaImage::new(4, 4);
            for p in img.pixels_mut() {
                *p = image::Rgba([255, 255, 255, 255]);
            }
            img
        };
        // a: baseline identical to run shot
        white().save(run_dir.join("a.png")).unwrap();
        white().save(baselines.join("a.png")).unwrap();
        // b: run shot differs from baseline everywhere
        white().save(run_dir.join("b.png")).unwrap();
        let mut dark = image::RgbaImage::new(4, 4);
        for p in dark.pixels_mut() {
            *p = image::Rgba([0, 0, 0, 255]);
        }
        dark.save(baselines.join("b.png")).unwrap();
        // c: no baseline yet
        white().save(run_dir.join("c.png")).unwrap();

        fs::write(
            sroot(root.path(), sid).join("replays").join("latest.txt"),
            "r1",
        )
        .unwrap();

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        let rc = run(&[sid.to_string(), "--dry-run".to_string()]).unwrap();
        std::env::set_current_dir(cwd).unwrap();
        assert_eq!(rc, 0);
        // dry-run wrote nothing: c.png must still be absent from baselines
        assert!(!baselines.join("c.png").exists());
        // b.png must still be the old dark baseline
        let b = crate::compare::screenshots::decode_png(&baselines.join("b.png")).unwrap();
        assert_eq!(*b.get_pixel(0, 0), image::Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn respects_steps_filter_and_missing_shot_bails() {
        let _g = crate::test_util::lock_env();
        let root = tempfile::tempdir().unwrap();
        let sid = "s-shot2";
        let run_dir = sroot(root.path(), sid)
            .join("replays")
            .join("r9")
            .join("screenshots");
        fs::create_dir_all(&run_dir).unwrap();
        fs::write(run_dir.join("a.png"), b"pngA").unwrap();

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        let ok = run(&[
            sid.to_string(),
            "--run".into(),
            "r9".into(),
            "--steps".into(),
            "a".into(),
        ])
        .unwrap();
        let err = run(&[
            sid.to_string(),
            "--run".into(),
            "r9".into(),
            "--steps".into(),
            "nope".into(),
        ]);
        std::env::set_current_dir(cwd).unwrap();

        assert_eq!(ok, 0);
        assert!(err.is_err());
        assert_eq!(
            fs::read(sroot(root.path(), sid).join("baselines").join("a.png")).unwrap(),
            b"pngA"
        );
    }
}
