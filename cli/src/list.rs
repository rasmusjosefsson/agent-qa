//! `list` verb — show a scenario directory's contents in a useful summary.
//!
//! Usage:
//!   agent-qa list                     # current scenarios root, all sids
//!   agent-qa list <sid|path>          # one scenario: steps + replay history
//!
//! Output is plain text optimised for terminal reading; `--json` emits a
//! structured object on stdout for automation.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::Value as Json;

use crate::paths;
use crate::scenario::Scenario;

pub fn run(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    let mut positional: Option<String> = None;
    let mut limit: Option<usize> = None;
    let mut filter: Option<String> = None;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json_out = true,
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            "--filter" => {
                filter = Some(
                    it.next()
                        .cloned()
                        .ok_or_else(|| anyhow!("--filter requires a substring"))?,
                )
            }
            s if s.starts_with("--filter=") => filter = Some(s["--filter=".len()..].to_string()),
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
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if positional.is_some() {
                    bail!("unexpected positional {other:?}; usage: list [<sid|path>]");
                }
                positional = Some(other.to_string());
            }
        }
    }

    let mut report = match positional {
        None => list_root()?,
        Some(target) => {
            let (file, dir) = resolve_target(&target)?;
            list_one(&file, &dir)?
        }
    };

    if let Some(f) = filter.as_deref() {
        apply_filter(&mut report, f);
    }
    if let Some(n) = limit {
        apply_limit(&mut report, n);
    }

    if json_out {
        let body = serde_json::to_string_pretty(&report)?;
        std::io::stdout().write_all(body.as_bytes())?;
        std::io::stdout().write_all(b"\n")?;
    } else {
        render_text(&report);
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa list — show scenario directory contents\n\nUsage:\n  agent-qa list                     All scenarios under the scenarios root\n  agent-qa list <sid | path>        One scenario: steps + replay history\n\nFlags:\n  --json                            Structured JSON on stdout\n  --filter <substr>                 Case-insensitive substring filter.\n                                    Root mode: matches sid + scenarioId +\n                                    intent. Single-scenario mode: matches\n                                    replay runId + summary + profile.\n  --limit <N>                       Cap output to N entries. In single-\n                                    scenario mode, keeps the most recent\n                                    N replays; in root mode, keeps the\n                                    first N scenarios (alphabetical).\n\nThe scenarios root is `<cwd>/tmp/agent-qa-scenarios` by default; override\nwith AGENT_QA_SCENARIOS_DIR."
    );
}

fn apply_filter(report: &mut Report, pattern: &str) {
    let p = pattern.to_ascii_lowercase();
    if let Some(view) = report.target.as_mut() {
        view.replays.retain(|r| {
            let run = r.run_id.to_ascii_lowercase();
            let summary = r.summary.as_deref().unwrap_or("").to_ascii_lowercase();
            let profile = r.profile.as_deref().unwrap_or("").to_ascii_lowercase();
            let tag = r.tag.as_deref().unwrap_or("").to_ascii_lowercase();
            run.contains(&p) || summary.contains(&p) || profile.contains(&p) || tag.contains(&p)
        });
    }
    if let Some(all) = report.all.as_mut() {
        all.retain(|v| {
            let sid = v.sid.to_ascii_lowercase();
            let scenario_id = v.scenario_id.as_deref().unwrap_or("").to_ascii_lowercase();
            let intent = v.intent.as_deref().unwrap_or("").to_ascii_lowercase();
            sid.contains(&p) || scenario_id.contains(&p) || intent.contains(&p)
        });
    }
}

fn apply_limit(report: &mut Report, n: usize) {
    if let Some(view) = report.target.as_mut() {
        // Replays are sorted by run_id (chronological). Keep the most
        // recent N — i.e. the tail.
        if view.replays.len() > n {
            let drop = view.replays.len() - n;
            view.replays.drain(0..drop);
        }
    }
    if let Some(all) = report.all.as_mut() {
        all.truncate(n);
    }
}

fn resolve_target(target: &str) -> Result<(PathBuf, PathBuf)> {
    let p = Path::new(target);
    if p.is_file() {
        let dir = p
            .parent()
            .ok_or_else(|| anyhow!("path {} has no parent", p.display()))?
            .to_path_buf();
        return Ok((p.to_path_buf(), dir));
    }
    // Treat as sid.
    let dir = paths::scenario_dir(target)?;
    let file = dir.join("scenario.json");
    if !file.is_file() {
        bail!("no scenario.json at {} (target={target:?})", file.display());
    }
    Ok((file, dir))
}

// ---------- report types ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    scenarios_root: String,
    target: Option<ScenarioView>,
    all: Option<Vec<ScenarioView>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioView {
    sid: String,
    dir: String,
    scenario_id: Option<String>,
    intent: Option<String>,
    steps: Option<usize>,
    replays: Vec<ReplayView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayView {
    run_id: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    summary: Option<String>,
    exit_code: Option<i64>,
    profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
}

// ---------- single scenario ----------

fn list_one(scenario_file: &Path, scenario_dir: &Path) -> Result<Report> {
    let view = build_view(scenario_dir, scenario_file)?;
    Ok(Report {
        scenarios_root: paths::scenarios_root().display().to_string(),
        target: Some(view),
        all: None,
    })
}

fn build_view(scenario_dir: &Path, scenario_file: &Path) -> Result<ScenarioView> {
    let bytes =
        fs::read(scenario_file).with_context(|| format!("read {}", scenario_file.display()))?;
    let parsed: Option<Scenario> = serde_json::from_slice(&bytes).ok();

    let sid = scenario_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "?".into());

    let replays = collect_replays(scenario_dir);

    Ok(ScenarioView {
        sid,
        dir: scenario_dir.display().to_string(),
        scenario_id: parsed.as_ref().map(|j| j.id.clone()),
        intent: parsed.as_ref().map(|j| j.intent.clone()),
        steps: parsed.as_ref().map(|j| j.steps.len()),
        replays,
    })
}

fn collect_replays(scenario_dir: &Path) -> Vec<ReplayView> {
    let mut out = Vec::new();
    let replays_dir = scenario_dir.join("replays");
    let entries = match fs::read_dir(&replays_dir) {
        Ok(it) => it,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let run_id = entry.file_name().to_string_lossy().into_owned();
        let audit_path = entry.path().join("audit.json");
        let audit: Option<Json> = fs::read(&audit_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        out.push(ReplayView {
            run_id: run_id.clone(),
            started_at: audit
                .as_ref()
                .and_then(|a| a.get("startedAt")?.as_str().map(str::to_string)),
            finished_at: audit
                .as_ref()
                .and_then(|a| a.get("finishedAt")?.as_str().map(str::to_string)),
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
        });
    }
    // Sort by run_id (timestamp prefix → chronological).
    out.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    out
}

// ---------- all scenarios under the root ----------

fn list_root() -> Result<Report> {
    let root = paths::scenarios_root();
    let mut all: Vec<ScenarioView> = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let scenario_file = dir.join("scenario.json");
            // Tolerate dirs without a scenario.json — they may be in-flight.
            let view = if scenario_file.is_file() {
                build_view(&dir, &scenario_file)?
            } else {
                ScenarioView {
                    sid: dir
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "?".into()),
                    dir: dir.display().to_string(),
                    scenario_id: None,
                    intent: None,
                    steps: None,
                    replays: collect_replays(&dir),
                }
            };
            all.push(view);
        }
    }
    all.sort_by(|a, b| a.sid.cmp(&b.sid));
    Ok(Report {
        scenarios_root: root.display().to_string(),
        target: None,
        all: Some(all),
    })
}

// ---------- text rendering ----------

fn render_text(r: &Report) {
    println!("scenarios root: {}", r.scenarios_root);
    if let Some(t) = &r.target {
        render_scenario(t, true);
    }
    if let Some(all) = &r.all {
        if all.is_empty() {
            println!("(no scenarios)");
            return;
        }
        for v in all {
            render_scenario(v, false);
        }
    }
}

fn render_scenario(v: &ScenarioView, verbose: bool) {
    let id_str = v.scenario_id.as_deref().unwrap_or("?");
    let steps = v.steps.map(|n| n.to_string()).unwrap_or_else(|| "?".into());
    let intent = v.intent.as_deref().unwrap_or("");
    println!();
    println!("• {}  id={id_str}  steps={steps}", v.sid);
    if verbose {
        println!("  dir:   {}", v.dir);
        if !intent.is_empty() {
            println!("  intent: {intent}");
        }
    }
    if v.replays.is_empty() {
        println!("  replays: (none)");
        return;
    }
    println!("  replays ({}):", v.replays.len());
    for r in &v.replays {
        let summary = r.summary.as_deref().unwrap_or("(in flight)");
        let exit = r
            .exit_code
            .map(|c| format!(" exit={c}"))
            .unwrap_or_default();
        let prof = r
            .profile
            .as_deref()
            .map(|p| format!(" profile={p}"))
            .unwrap_or_default();
        let tag = r
            .tag
            .as_deref()
            .map(|t| format!(" tag={t}"))
            .unwrap_or_default();
        println!("    - {summary}{exit}{prof}{tag}");
        println!("      run: {}", r.run_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_scenario(dir: &Path, id: &str, intent: &str, steps: usize) {
        let mut steps_json = String::from("[");
        for i in 0..steps {
            if i > 0 {
                steps_json.push(',');
            }
            steps_json.push_str(&format!(
                r#"{{"id":"s{i}","intent":"step {i}","kind":"do","verb":"reload"}}"#
            ));
        }
        steps_json.push(']');
        let body = format!(
            r#"{{"schema":"scenario/2","id":{id_json},"intent":{intent_json},"steps":{steps_json}}}"#,
            id_json = serde_json::to_string(id).unwrap(),
            intent_json = serde_json::to_string(intent).unwrap()
        );
        fs::write(dir.join("scenario.json"), body).unwrap();
    }

    fn write_audit(dir: &Path, run_id: &str, summary: &str, exit: i64) {
        let run_dir = dir.join("replays").join(run_id);
        fs::create_dir_all(&run_dir).unwrap();
        let body = format!(
            r#"{{"schema":"scenario-replay-audit/v1","runId":"{run_id}","scenarioId":"j","startedAt":"2026-01-01T00:00:00.000Z","finishedAt":"2026-01-01T00:00:01.000Z","summary":"{summary}","exitCode":{exit},"scenarioContentHash":"deadbeef"}}"#
        );
        fs::write(run_dir.join("audit.json"), body).unwrap();
    }

    #[test]
    fn list_one_returns_steps_and_replays() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("mysid");
        fs::create_dir_all(&jdir).unwrap();
        write_scenario(&jdir, "j1", "smoke", 3);
        write_audit(
            &jdir,
            "2026-01-01T00-00-00-000Z__deadbeef",
            "SUMMARY: 3/3 (PASS)",
            0,
        );
        write_audit(
            &jdir,
            "2026-01-01T01-00-00-000Z__feedface",
            "SUMMARY: 2/3 (FAIL)",
            1,
        );

        let report = list_one(&jdir.join("scenario.json"), &jdir).unwrap();
        let v = report.target.unwrap();
        assert_eq!(v.scenario_id.as_deref(), Some("j1"));
        assert_eq!(v.intent.as_deref(), Some("smoke"));
        assert_eq!(v.steps, Some(3));
        assert_eq!(v.replays.len(), 2);
        assert_eq!(v.replays[0].run_id, "2026-01-01T00-00-00-000Z__deadbeef");
        assert_eq!(v.replays[0].exit_code, Some(0));
        assert_eq!(v.replays[1].exit_code, Some(1));
        assert!(v.replays[1].summary.as_deref().unwrap().contains("FAIL"));
    }

    #[test]
    fn list_one_tolerates_no_replays() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        write_scenario(&jdir, "j1", "x", 1);
        let report = list_one(&jdir.join("scenario.json"), &jdir).unwrap();
        assert!(report.target.unwrap().replays.is_empty());
    }

    #[test]
    fn list_one_tolerates_malformed_scenario() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        fs::write(jdir.join("scenario.json"), "not json").unwrap();
        // We still produce a view; just no parsed fields.
        let report = list_one(&jdir.join("scenario.json"), &jdir).unwrap();
        let v = report.target.unwrap();
        assert!(v.scenario_id.is_none());
        assert!(v.steps.is_none());
    }

    #[test]
    fn apply_limit_keeps_most_recent_replays_in_single_mode() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        fs::create_dir_all(&jdir).unwrap();
        write_scenario(&jdir, "j1", "x", 1);
        for i in 0..5 {
            write_audit(
                &jdir,
                &format!("2026-01-0{i}T00-00-00-000Z__hash{i}"),
                "SUMMARY: 1/1 (PASS)",
                0,
            );
        }
        let mut report = list_one(&jdir.join("scenario.json"), &jdir).unwrap();
        assert_eq!(report.target.as_ref().unwrap().replays.len(), 5);
        apply_limit(&mut report, 2);
        let view = report.target.unwrap();
        assert_eq!(view.replays.len(), 2);
        // The two retained should be the most recent (3 and 4 in the prefix).
        assert!(view.replays[0].run_id.contains("2026-01-03"));
        assert!(view.replays[1].run_id.contains("2026-01-04"));
    }

    #[test]
    fn apply_limit_truncates_root_list() {
        let mut report = Report {
            scenarios_root: "/tmp".into(),
            target: None,
            all: Some(vec![
                ScenarioView {
                    sid: "a".into(),
                    dir: "".into(),
                    scenario_id: None,
                    intent: None,
                    steps: None,
                    replays: vec![],
                },
                ScenarioView {
                    sid: "b".into(),
                    dir: "".into(),
                    scenario_id: None,
                    intent: None,
                    steps: None,
                    replays: vec![],
                },
                ScenarioView {
                    sid: "c".into(),
                    dir: "".into(),
                    scenario_id: None,
                    intent: None,
                    steps: None,
                    replays: vec![],
                },
            ]),
        };
        apply_limit(&mut report, 2);
        let all = report.all.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].sid, "a");
        assert_eq!(all[1].sid, "b");
    }

    #[test]
    fn apply_filter_keeps_matching_sids_in_root_mode() {
        let mut report = Report {
            scenarios_root: "/tmp".into(),
            target: None,
            all: Some(vec![
                ScenarioView {
                    sid: "alpha".into(),
                    dir: "".into(),
                    scenario_id: Some("alpha-id".into()),
                    intent: Some("login".into()),
                    steps: None,
                    replays: vec![],
                },
                ScenarioView {
                    sid: "beta".into(),
                    dir: "".into(),
                    scenario_id: Some("beta-id".into()),
                    intent: Some("checkout flow".into()),
                    steps: None,
                    replays: vec![],
                },
            ]),
        };
        // Case-insensitive substring against sid + scenarioId + intent.
        apply_filter(&mut report, "CHECKOUT");
        let all = report.all.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].sid, "beta");
    }

    #[test]
    fn apply_filter_keeps_matching_replays_in_single_mode() {
        let mut report = Report {
            scenarios_root: "/tmp".into(),
            target: Some(ScenarioView {
                sid: "j".into(),
                dir: "".into(),
                scenario_id: None,
                intent: None,
                steps: None,
                replays: vec![
                    ReplayView {
                        run_id: "2026-01-01__abc".into(),
                        started_at: None,
                        finished_at: None,
                        summary: Some("SUMMARY: 3/3 (PASS)".into()),
                        exit_code: Some(0),
                        profile: Some("default".into()),
                        tag: None,
                    },
                    ReplayView {
                        run_id: "2026-01-02__def".into(),
                        started_at: None,
                        finished_at: None,
                        summary: Some("SUMMARY: 2/3 (FAIL)".into()),
                        exit_code: Some(1),
                        profile: Some("corp".into()),
                        tag: None,
                    },
                ],
            }),
            all: None,
        };
        apply_filter(&mut report, "fail");
        let view = report.target.unwrap();
        assert_eq!(view.replays.len(), 1);
        assert_eq!(view.replays[0].run_id, "2026-01-02__def");
    }

    #[test]
    fn apply_filter_also_matches_tag() {
        let mut report = Report {
            scenarios_root: "/tmp".into(),
            target: Some(ScenarioView {
                sid: "j".into(),
                dir: "".into(),
                scenario_id: None,
                intent: None,
                steps: None,
                replays: vec![
                    ReplayView {
                        run_id: "r1".into(),
                        started_at: None,
                        finished_at: None,
                        summary: None,
                        exit_code: None,
                        profile: None,
                        tag: Some("nightly".into()),
                    },
                    ReplayView {
                        run_id: "r2".into(),
                        started_at: None,
                        finished_at: None,
                        summary: None,
                        exit_code: None,
                        profile: None,
                        tag: Some("pre-deploy".into()),
                    },
                ],
            }),
            all: None,
        };
        apply_filter(&mut report, "NIGHT");
        let view = report.target.unwrap();
        assert_eq!(view.replays.len(), 1);
        assert_eq!(view.replays[0].run_id, "r1");
    }
}
