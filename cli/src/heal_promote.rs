//! `heal-promote` verb — apply replay-side suggested locator patches back
//! into `scenario.json`.
//!
//! Reads externally supplied
//! `<sid>/replays/<runId>/diffs/<stepId>.patch.json` files and rewrites the
//! matching step's `on` field. Core replay does not yet produce these files.
//! Default mode is dry-run; `--apply` atomically rewrites the contract.
//!
//! Drift guard: each patch file may carry a `scenarioContentHash` field
//! recorded at the time the replay produced the patch. When a patch's
//! hash doesn't match the live scenario's current hash, the verb refuses
//! to apply (exit 3) so a hand-edited contract isn't overwritten.
//!
//! Patch file shape (the producer side lands later; this verb is the
//! consumer):
//!
//! ```json
//! {
//!   "schema": "heal-patch/v1",
//!   "stepId": "s2",
//!   "scenarioContentHash": "<sha256-of-scenario.json-bytes>",
//!   "newLocator": { "role": "button", "name": "Save changes" },
//!   "rationale": "name drifted: 'Save' → 'Save changes'"
//! }
//! ```

use std::collections::BTreeMap;
use std::fs;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::paths;
use crate::sidecar::{atomic_write_file, hash_scenario_bytes};

const EXIT_REBASE_GUARD: u8 = 3;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let plan = build_plan(&opts)?;
    if plan.patches.is_empty() {
        println!("(no heal patches found under {})", plan.diffs_dir.display());
        return Ok(0);
    }
    print_plan(&plan);

    if !opts.apply {
        println!();
        println!("dry-run; pass --apply to rewrite scenario.json");
        return Ok(0);
    }

    if let Some(stale) = plan.patches.iter().find(|p| p.hash_mismatch) {
        eprintln!(
            "FAIL refusing to apply: patch for {} carries scenarioContentHash {} but live scenario is {}",
            stale.step_id,
            stale.recorded_hash.as_deref().unwrap_or("(missing)"),
            plan.live_hash
        );
        return Ok(EXIT_REBASE_GUARD);
    }
    apply_plan(&plan)?;
    println!(
        "\napplied {} patch(es) to {}",
        plan.patches.len(),
        plan.scenario_file.display()
    );
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa heal-promote \u{2014} apply replay-side suggested locator patches\n\nUsage:\n  agent-qa heal-promote <sid> [--run <runId>] [--steps <s1,s2,\u{2026}>]\n                              [--apply]\n\n--run defaults to <sid>/replays/latest.txt.\n--steps filters which patches to consider (comma-separated stepIds).\n--apply atomically rewrites scenario.json; without it, dry-run.\n\nExit codes:\n  0  ok (or no patches found)\n  1  any other error\n  3  rebase guard \u{2014} a patch's recorded scenarioContentHash doesn't match\n     the live contract; refusing to overwrite hand edits"
    );
}

#[derive(Debug, Clone)]
struct Opts {
    sid: String,
    run_id: Option<String>,
    step_filter: Option<Vec<String>>,
    apply: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchFile {
    #[serde(default)]
    schema: Option<String>,
    step_id: String,
    #[serde(default)]
    scenario_content_hash: Option<String>,
    new_locator: Json,
    #[serde(default)]
    rationale: Option<String>,
}

#[derive(Debug)]
struct Patch {
    step_id: String,
    new_locator: Json,
    rationale: Option<String>,
    recorded_hash: Option<String>,
    hash_mismatch: bool,
}

#[derive(Debug)]
struct Plan {
    scenario_file: std::path::PathBuf,
    scenario: Json,
    live_hash: String,
    diffs_dir: std::path::PathBuf,
    run_id: String,
    patches: Vec<Patch>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut sid: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut step_filter: Option<Vec<String>> = None;
    let mut apply = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--run" => run_id = it.next().cloned(),
            s if s.starts_with("--run=") => run_id = Some(s["--run=".len()..].to_string()),
            "--steps" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow!("--steps requires a comma-separated list"))?;
                step_filter = Some(
                    v.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            s if s.starts_with("--steps=") => {
                let v = &s["--steps=".len()..];
                step_filter = Some(
                    v.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            "--apply" => apply = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!("unexpected positional {other:?}; usage: heal-promote <sid> [flags]");
                }
                sid = Some(other.to_string());
            }
        }
    }
    let sid = sid.ok_or_else(|| anyhow!("usage: heal-promote <sid> [flags]"))?;
    Ok(Opts {
        sid,
        run_id,
        step_filter,
        apply,
    })
}

fn build_plan(opts: &Opts) -> Result<Plan> {
    let scenario_dir = paths::scenario_dir(&opts.sid)?;
    let scenario_file = scenario_dir.join("scenario.json");
    let bytes =
        fs::read(&scenario_file).with_context(|| format!("read {}", scenario_file.display()))?;
    let scenario: Json = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse {}", scenario_file.display()))?;
    let live_hash = hash_scenario_bytes(&bytes);

    let run_id = match &opts.run_id {
        Some(r) => r.clone(),
        None => {
            let latest = scenario_dir.join("replays").join("latest.txt");
            fs::read_to_string(&latest)
                .map(|s| s.trim().to_string())
                .map_err(|_| anyhow!("--run not given and no {} pointer found", latest.display()))?
        }
    };
    if !is_safe_segment(&run_id) {
        bail!("unsafe runId: {run_id:?}");
    }

    let diffs_dir = scenario_dir.join("replays").join(&run_id).join("diffs");
    let mut patches: Vec<Patch> = Vec::new();
    if diffs_dir.is_dir() {
        for entry in fs::read_dir(&diffs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let body = match fs::read(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let patch: PatchFile = serde_json::from_slice(&body)
                .with_context(|| format!("parse {}", path.display()))?;
            if let Some(filter) = &opts.step_filter {
                if !filter.iter().any(|s| s == &patch.step_id) {
                    continue;
                }
            }
            let mismatch = patch
                .scenario_content_hash
                .as_ref()
                .map(|h| h != &live_hash)
                .unwrap_or(false);
            patches.push(Patch {
                step_id: patch.step_id,
                new_locator: patch.new_locator,
                rationale: patch.rationale,
                recorded_hash: patch.scenario_content_hash,
                hash_mismatch: mismatch,
            });
        }
    }
    patches.sort_by(|a, b| a.step_id.cmp(&b.step_id));

    Ok(Plan {
        scenario_file,
        scenario,
        live_hash,
        diffs_dir,
        run_id,
        patches,
    })
}

fn print_plan(plan: &Plan) {
    println!("scenario: {}", plan.scenario_file.display());
    println!("run:     {}", plan.run_id);
    println!("found {} patch(es):", plan.patches.len());
    for p in &plan.patches {
        let mismatch = if p.hash_mismatch {
            " [HASH MISMATCH]"
        } else {
            ""
        };
        let why = p.rationale.as_deref().unwrap_or("(no rationale)");
        println!("  {}{}", p.step_id, mismatch);
        println!("    why:      {why}");
        let loc = serde_json::to_string(&p.new_locator).unwrap_or_else(|_| "?".into());
        println!("    locator:  {loc}");
    }
}

fn apply_plan(plan: &Plan) -> Result<()> {
    let mut scenario = plan.scenario.clone();
    let steps = scenario
        .get_mut("steps")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("scenario.json has no steps[] array"))?;
    let mut by_id: BTreeMap<String, &Patch> = plan
        .patches
        .iter()
        .map(|p| (p.step_id.clone(), p))
        .collect();
    for step in steps.iter_mut() {
        if let Some(id) = step.get("id").and_then(|v| v.as_str()).map(str::to_string) {
            if let Some(patch) = by_id.remove(&id) {
                if let Some(obj) = step.as_object_mut() {
                    obj.insert("on".into(), patch.new_locator.clone());
                }
            }
        }
    }
    if !by_id.is_empty() {
        let unmatched: Vec<&String> = by_id.keys().collect();
        bail!("patches reference unknown step ids: {unmatched:?}");
    }
    let mut bytes = serde_json::to_vec_pretty(&scenario)?;
    bytes.push(b'\n');
    atomic_write_file(&plan.scenario_file, &bytes)?;
    Ok(())
}

fn is_safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
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

    fn write_minimal_scenario(jdir: &Path, sid: &str) -> Vec<u8> {
        fs::create_dir_all(jdir).unwrap();
        let body = format!(
            r#"{{
              "schema":"scenario/2","id":{id_q},"intent":"smoke",
              "steps":[
                {{"id":"s0","intent":"go","kind":"do","verb":"goto",
                  "value":{{"from":"literal","literal":"https://x"}}}},
                {{"id":"s1","intent":"click","kind":"do","verb":"click",
                  "on":{{"role":"button","name":"Save"}}}}
              ]
            }}"#,
            id_q = serde_json::to_string(sid).unwrap()
        );
        fs::write(jdir.join("scenario.json"), &body).unwrap();
        body.into_bytes()
    }

    fn write_patch(
        jdir: &Path,
        run: &str,
        step_id: &str,
        hash: Option<&str>,
        locator: serde_json::Value,
    ) {
        let dir = jdir.join("replays").join(run).join("diffs");
        fs::create_dir_all(&dir).unwrap();
        let mut obj = serde_json::Map::new();
        obj.insert("schema".into(), json!("heal-patch/v1"));
        obj.insert("stepId".into(), json!(step_id));
        if let Some(h) = hash {
            obj.insert("scenarioContentHash".into(), json!(h));
        }
        obj.insert("newLocator".into(), locator);
        obj.insert("rationale".into(), json!("test"));
        fs::write(
            dir.join(format!("{step_id}.patch.json")),
            serde_json::to_string(&Json::Object(obj)).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn dry_run_lists_patches_without_writing() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let pre = write_minimal_scenario(&jdir, "j1");
        write_patch(
            &jdir,
            "rA",
            "s1",
            Some(&hash_scenario_bytes(&pre)),
            json!({"role":"button","name":"Save changes"}),
        );

        let plan = build_plan(&Opts {
            sid: "j1".into(),
            run_id: Some("rA".into()),
            step_filter: None,
            apply: false,
        })
        .unwrap();
        assert_eq!(plan.patches.len(), 1);
        assert_eq!(plan.patches[0].step_id, "s1");
        assert!(!plan.patches[0].hash_mismatch);

        // Scenario untouched.
        let post = fs::read(jdir.join("scenario.json")).unwrap();
        assert_eq!(post, pre);
        teardown();
    }

    #[test]
    fn apply_rewrites_locator_atomically() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let pre = write_minimal_scenario(&jdir, "j1");
        write_patch(
            &jdir,
            "rA",
            "s1",
            Some(&hash_scenario_bytes(&pre)),
            json!({"role":"button","name":"Save changes"}),
        );
        let plan = build_plan(&Opts {
            sid: "j1".into(),
            run_id: Some("rA".into()),
            step_filter: None,
            apply: true,
        })
        .unwrap();
        apply_plan(&plan).unwrap();
        let post: Json =
            serde_json::from_slice(&fs::read(jdir.join("scenario.json")).unwrap()).unwrap();
        assert_eq!(post["steps"][1]["on"]["name"], "Save changes");
        teardown();
    }

    #[test]
    fn rebase_guard_blocks_apply_on_hash_mismatch() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_minimal_scenario(&jdir, "j1");
        // Patch carries an unrelated hash.
        write_patch(
            &jdir,
            "rA",
            "s1",
            Some("not-the-real-hash"),
            json!({"role":"button","name":"Save changes"}),
        );
        let plan = build_plan(&Opts {
            sid: "j1".into(),
            run_id: Some("rA".into()),
            step_filter: None,
            apply: true,
        })
        .unwrap();
        assert!(plan.patches.iter().any(|p| p.hash_mismatch));
        // apply_plan() doesn't enforce the guard — `run` does. Verify the
        // mismatch flag is set so `run` would short-circuit.
        teardown();
    }

    #[test]
    fn step_filter_narrows_patches() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let pre = write_minimal_scenario(&jdir, "j1");
        let h = hash_scenario_bytes(&pre);
        write_patch(
            &jdir,
            "rA",
            "s0",
            Some(&h),
            json!({"role":"link","name":"home"}),
        );
        write_patch(
            &jdir,
            "rA",
            "s1",
            Some(&h),
            json!({"role":"button","name":"x"}),
        );
        let plan = build_plan(&Opts {
            sid: "j1".into(),
            run_id: Some("rA".into()),
            step_filter: Some(vec!["s1".into()]),
            apply: false,
        })
        .unwrap();
        assert_eq!(plan.patches.len(), 1);
        assert_eq!(plan.patches[0].step_id, "s1");
        teardown();
    }

    #[test]
    fn no_patches_directory_returns_empty_plan() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        write_minimal_scenario(&jdir, "j1");
        let plan = build_plan(&Opts {
            sid: "j1".into(),
            run_id: Some("rA".into()),
            step_filter: None,
            apply: false,
        })
        .unwrap();
        assert!(plan.patches.is_empty());
        teardown();
    }

    #[test]
    fn apply_errors_on_unknown_step_id() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let jdir = tmp.path().join("j1");
        let pre = write_minimal_scenario(&jdir, "j1");
        write_patch(
            &jdir,
            "rA",
            "sNope",
            Some(&hash_scenario_bytes(&pre)),
            json!({"role":"button","name":"x"}),
        );
        let plan = build_plan(&Opts {
            sid: "j1".into(),
            run_id: Some("rA".into()),
            step_filter: None,
            apply: true,
        })
        .unwrap();
        let err = apply_plan(&plan).unwrap_err().to_string();
        assert!(err.contains("unknown step ids"));
        teardown();
    }
}
