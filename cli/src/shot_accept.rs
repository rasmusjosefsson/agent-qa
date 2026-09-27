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

use anyhow::{bail, Context, Result};

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let mut sid: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut steps: Option<Vec<String>> = None;
    let mut json = false;

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
            "--help" | "-h" => {
                println!("agent-qa shot-accept — mint screenshot baselines from a run\n\nUsage:\n  agent-qa shot-accept <sid> [--run <runId>] [--steps <csv>] [--json]\n\nCopies <sid>/replays/<run>/screenshots/<stepId>.png to\n<sid>/baselines/<stepId>.png for {{\"shot\"}} claims. Defaults:\nlatest run, every captured screenshot.");
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
    let shots_dir = sdir.join("replays").join(&rid).join("screenshots");
    if !shots_dir.is_dir() {
        bail!("no screenshots dir at {}", shots_dir.display());
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

    let base_dir = sdir.join("baselines");
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

    if json {
        println!("{}", serde_json::to_string(&minted)?);
    } else {
        println!(
            "minted {} baseline(s) under {} from run {rid}: {}",
            minted.len(),
            base_dir.display(),
            minted.join(", ")
        );
    }
    Ok(0)
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
