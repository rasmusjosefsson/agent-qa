//! `design` verb — the design-fidelity lane, kept separate from the
//! behavioural pass/fail lane.
//!
//! Replay answers "does it work". This answers "does it look like the
//! design we agreed on". They are different judgments: a failed claim is
//! always a bug, a design mismatch is sometimes a deliberate deviation.
//! Sharing one gate means either design noise blocks every PR or design
//! gets muted to keep CI green — both end with design ignored.
//!
//! Inputs are a convention, not a schema change:
//!
//!   <sid>/designs/<stepId>.png     reference export (png/jpg/jpeg/webp)
//!   <sid>/designs/<stepId>.md      optional notes for the reviewer
//!   <sid>/designs/verdicts.json    committed decisions, stepId -> verdict
//!
//! Verdicts are decisions, not results:
//!
//!   ok        matches the design
//!   accepted  deviates, and we decided that is fine (needs --reason)
//!   fail      real drift
//!   ask       reviewer is not confident — a human must answer
//!
//! Each verdict stamps the sha256 of the design file it judged. Re-export
//! the frame from Figma and the hash moves, which expires the verdict and
//! sends the step back to needs-review. That is what stops "we reviewed
//! that once in March" from counting forever.
//!
//! CLI shape:
//!
//!   agent-qa design review <sid> [--run <runId|latest>] [--json]
//!   agent-qa design verdict <sid> --step <id> (--ok | --accepted --reason <t>
//!                                              | --fail --reason <t> | --ask [--reason <t>])
//!
//! `review` exits 0 only when every design is `ok` or `accepted` against
//! the current design bytes; anything else exits 2. Screenshots come from
//! the replay run, so `review` never drives a browser.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::sidecar::{atomic_write_file, hash_scenario_bytes};

const DESIGN_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];

pub fn run(args: &[String]) -> Result<u8> {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    match sub {
        "review" => review(rest),
        "verdict" => verdict(rest),
        "-h" | "--help" | "help" | "" => {
            print_help();
            Ok(0)
        }
        other => bail!("unknown design sub-verb {other:?}; expected review | verdict"),
    }
}

fn print_help() {
    println!(
        "agent-qa design \u{2014} design-fidelity review, separate from the behavioural lane\n\nUsage:\n  agent-qa design review <sid> [--run <runId|latest>] [--json]\n  agent-qa design verdict <sid> --step <stepId>\n                                (--ok | --accepted --reason <text>\n                                 | --fail --reason <text> | --ask [--reason <text>])\n\nInputs (convention, no schema change):\n  <sid>/designs/<stepId>.png     reference export (png/jpg/jpeg/webp)\n  <sid>/designs/<stepId>.md      optional reviewer notes\n  <sid>/designs/verdicts.json    committed decisions\n\nVerdicts: ok | accepted (deliberate deviation) | fail | ask (human must answer).\nEach stamps the design file's sha256 \u{2014} re-exporting the frame expires it.\n\nreview pairs each design with <sid>/replays/<run>/screenshots/<stepId>.png,\nwrites design-review.md into the run dir, and exits 2 unless every design is\nok or accepted against current bytes. It never drives a browser."
    );
}

// ---------- verdict store ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub verdict: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub design_hash: String,
    pub recorded_at: String,
}

type Store = BTreeMap<String, Verdict>;

fn verdicts_path(scenario_dir: &Path) -> PathBuf {
    scenario_dir.join("designs").join("verdicts.json")
}

fn load_store(scenario_dir: &Path) -> Result<Store> {
    let p = verdicts_path(scenario_dir);
    match fs::read(&p) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("parse {}", p.display()))
            .map_err(|e| anyhow!("{e:#}")),
        Err(_) => Ok(Store::new()),
    }
}

fn save_store(scenario_dir: &Path, store: &Store) -> Result<PathBuf> {
    let p = verdicts_path(scenario_dir);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    let mut body = serde_json::to_string_pretty(store)?;
    body.push('\n');
    atomic_write_file(&p, body.as_bytes())?;
    Ok(p)
}

// ---------- pairing ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub step_id: String,
    /// needs-review | stale | ok | accepted | fail | ask | no-screenshot | unknown-step
    pub status: String,
    pub design: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Row {
    fn blocking(&self) -> bool {
        !matches!(self.status.as_str(), "ok" | "accepted")
    }
}

/// Every `<stepId>.<ext>` under `designs/`, sorted by step id.
/// `verdicts.json` and note files are not designs.
fn collect_designs(designs_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let Ok(entries) = fs::read_dir(designs_dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !DESIGN_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        out.push((stem, path));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Step ids declared by the scenario, used to flag a design whose
/// filename matches no step (the usual cause is a typo'd export name).
/// An unreadable scenario.json yields None — we degrade to not checking
/// rather than failing the whole review.
fn scenario_step_ids(scenario_dir: &Path) -> Option<Vec<String>> {
    let bytes = fs::read(scenario_dir.join("scenario.json")).ok()?;
    let doc: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let steps = doc.get("steps")?.as_array()?;
    Some(
        steps
            .iter()
            .filter_map(|s| s.get("id")?.as_str().map(str::to_string))
            .collect(),
    )
}

pub fn build_rows(scenario_dir: &Path, shots_dir: &Path) -> Result<Vec<Row>> {
    let designs_dir = scenario_dir.join("designs");
    let store = load_store(scenario_dir)?;
    let step_ids = scenario_step_ids(scenario_dir);
    let mut rows = Vec::new();

    for (step_id, design_path) in collect_designs(&designs_dir) {
        let bytes =
            fs::read(&design_path).with_context(|| format!("read {}", design_path.display()))?;
        let hash = hash_scenario_bytes(&bytes);

        let notes_path = designs_dir.join(format!("{step_id}.md"));
        let notes = notes_path
            .is_file()
            .then(|| notes_path.display().to_string());

        let shot_path = shots_dir.join(format!("{step_id}.png"));
        let screenshot = shot_path.is_file().then(|| shot_path.display().to_string());

        let recorded = store.get(&step_id);
        let status = if step_ids
            .as_ref()
            .is_some_and(|ids| !ids.iter().any(|i| i == &step_id))
        {
            "unknown-step".to_string()
        } else if screenshot.is_none() {
            "no-screenshot".to_string()
        } else {
            match recorded {
                None => "needs-review".to_string(),
                Some(v) if v.design_hash != hash => "stale".to_string(),
                Some(v) => v.verdict.clone(),
            }
        };

        rows.push(Row {
            step_id,
            status,
            design: design_path.display().to_string(),
            screenshot,
            notes,
            reason: recorded.and_then(|v| v.reason.clone()),
        });
    }
    Ok(rows)
}

// ---------- review ----------

fn review(args: &[String]) -> Result<u8> {
    let mut sid: Option<String> = None;
    let mut run_ref = "latest".to_string();
    let mut json_out = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            "--json" => json_out = true,
            "--run" => {
                run_ref = it
                    .next()
                    .cloned()
                    .ok_or_else(|| anyhow!("--run requires a value"))?
            }
            s if s.starts_with("--run=") => run_ref = s["--run=".len()..].to_string(),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!(
                        "unexpected positional {other:?}; usage: design review <sid> [--run <id>]"
                    );
                }
                sid = Some(other.to_string());
            }
        }
    }
    let sid = sid.ok_or_else(|| anyhow!("usage: agent-qa design review <sid> [--run <runId>]"))?;
    let scenario_dir = paths::scenario_dir(&sid)?;
    if !scenario_dir.is_dir() {
        bail!("no scenario at {}", scenario_dir.display());
    }
    let run_id = crate::audit::resolve_run_id(&scenario_dir, &run_ref)?;
    let run_dir = scenario_dir.join("replays").join(&run_id);
    let rows = build_rows(&scenario_dir, &run_dir.join("screenshots"))?;

    let total_steps = scenario_step_ids(&scenario_dir).map(|v| v.len());
    let report = render_markdown(&sid, &run_id, &rows, total_steps);
    if run_dir.is_dir() {
        atomic_write_file(&run_dir.join("design-review.md"), report.as_bytes())?;
    }

    let blocking = rows.iter().filter(|r| r.blocking()).count();
    if json_out {
        let body = serde_json::json!({
            "sid": sid,
            "runId": run_id,
            "designs": rows.len(),
            "steps": total_steps,
            "blocking": blocking,
            "rows": rows,
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        print!("{report}");
    }
    Ok(if blocking == 0 { 0 } else { 2 })
}

fn render_markdown(sid: &str, run_id: &str, rows: &[Row], total_steps: Option<usize>) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let coverage = match total_steps {
        Some(t) => format!("{} of {t} steps have a design", rows.len()),
        None => format!("{} designs", rows.len()),
    };
    let _ = writeln!(s, "# Design review — {sid} / {run_id}\n");
    let _ = writeln!(
        s,
        "Coverage: {coverage}. A step without a design is uncovered, not passing.\n"
    );
    if rows.is_empty() {
        let _ = writeln!(
            s,
            "No designs found. Drop reference exports at <sid>/designs/<stepId>.png.\n"
        );
        return s;
    }
    let _ = writeln!(s, "| step | status | reason |");
    let _ = writeln!(s, "| --- | --- | --- |");
    for r in rows {
        let _ = writeln!(
            s,
            "| {} | {} | {} |",
            r.step_id,
            r.status,
            r.reason.as_deref().unwrap_or("")
        );
    }
    let open: Vec<&Row> = rows.iter().filter(|r| r.blocking()).collect();
    if open.is_empty() {
        let _ = writeln!(s, "\nAll designs are ok or accepted.\n");
        return s;
    }
    let _ = writeln!(s, "\n## Open — compare these pairs\n");
    for r in open {
        let _ = writeln!(s, "### {} ({})\n", r.step_id, r.status);
        let _ = writeln!(s, "- design: {}", r.design);
        match &r.screenshot {
            Some(p) => {
                let _ = writeln!(s, "- built:  {p}");
            }
            None => {
                let _ = writeln!(
                    s,
                    "- built:  MISSING — the step produced no screenshot in this run"
                );
            }
        }
        if let Some(n) = &r.notes {
            let _ = writeln!(s, "- notes:  {n}");
        }
        let _ = writeln!(s);
    }
    let _ = writeln!(
        s,
        "Judge layout, spacing, hierarchy, component choice and state — NOT text content.\
         \nMock copy and real data differ by design; different words are never a `fail`.\
         \nRecord each decision:\n\n```\nagent-qa design verdict <sid> --step <id> --ok\nagent-qa design verdict <sid> --step <id> --accepted --reason '<why the deviation is fine>'\nagent-qa design verdict <sid> --step <id> --fail --reason '<what drifted>'\nagent-qa design verdict <sid> --step <id> --ask --reason '<what you need the human to decide>'\n```\n"
    );
    s
}

// ---------- verdict ----------

fn verdict(args: &[String]) -> Result<u8> {
    let mut sid: Option<String> = None;
    let mut step: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut chosen: Option<&str> = None;
    let mut it = args.iter();
    let set = |c: &'static str, chosen: &mut Option<&str>| -> Result<()> {
        if let Some(prev) = chosen {
            bail!("conflicting verdicts: --{prev} and --{c}");
        }
        *chosen = Some(c);
        Ok(())
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            "--ok" => set("ok", &mut chosen)?,
            "--accepted" => set("accepted", &mut chosen)?,
            "--fail" => set("fail", &mut chosen)?,
            "--ask" => set("ask", &mut chosen)?,
            "--step" => step = it.next().cloned(),
            s if s.starts_with("--step=") => step = Some(s["--step=".len()..].to_string()),
            "--reason" => reason = it.next().cloned(),
            s if s.starts_with("--reason=") => reason = Some(s["--reason=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!("unexpected positional {other:?}; usage: design verdict <sid> --step <id> --ok|--accepted|--fail|--ask");
                }
                sid = Some(other.to_string());
            }
        }
    }
    let sid = sid.ok_or_else(|| anyhow!("usage: agent-qa design verdict <sid> --step <stepId> (--ok | --accepted --reason <t> | --fail --reason <t> | --ask)"))?;
    let step = step.ok_or_else(|| anyhow!("design verdict requires --step <stepId>"))?;
    let kind = chosen.ok_or_else(|| anyhow!("pick one of --ok / --accepted / --fail / --ask"))?;
    // A deviation nobody justified is indistinguishable from drift nobody
    // noticed, so the two verdicts that record a divergence need a reason.
    if matches!(kind, "accepted" | "fail") && reason.as_deref().unwrap_or("").trim().is_empty() {
        bail!("--{kind} requires --reason <text>");
    }

    let scenario_dir = paths::scenario_dir(&sid)?;
    let design_path = find_design(&scenario_dir.join("designs"), &step).ok_or_else(|| {
        anyhow!(
            "no design for step {step:?} under {}/designs (expected {step}.png)",
            scenario_dir.display()
        )
    })?;
    let bytes =
        fs::read(&design_path).with_context(|| format!("read {}", design_path.display()))?;

    let mut store = load_store(&scenario_dir)?;
    store.insert(
        step.clone(),
        Verdict {
            verdict: kind.to_string(),
            reason: reason.filter(|r| !r.trim().is_empty()),
            design_hash: hash_scenario_bytes(&bytes),
            recorded_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
        },
    );
    let path = save_store(&scenario_dir, &store)?;
    println!("design verdict {step}: {kind}");
    println!("file: {}", path.display());
    if kind == "ask" {
        println!(
            "`design review` stays red until a human turns this into --ok, --accepted or --fail."
        );
    }
    Ok(0)
}

fn find_design(designs_dir: &Path, step_id: &str) -> Option<PathBuf> {
    DESIGN_EXTS
        .iter()
        .map(|ext| designs_dir.join(format!("{step_id}.{ext}")))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn scaffold(steps: &[&str]) -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("designs")).unwrap();
        fs::create_dir_all(root.join("replays/r1/screenshots")).unwrap();
        let steps_json: Vec<serde_json::Value> = steps
            .iter()
            .map(|id| serde_json::json!({ "id": id, "intent": "x", "kind": "do", "verb": "click" }))
            .collect();
        fs::write(
            root.join("scenario.json"),
            serde_json::to_vec(&serde_json::json!({ "id": "s", "steps": steps_json })).unwrap(),
        )
        .unwrap();
        tmp
    }

    fn put(root: &Path, rel: &str, body: &[u8]) {
        fs::write(root.join(rel), body).unwrap();
    }

    fn rows_for(root: &Path) -> Vec<Row> {
        build_rows(root, &root.join("replays/r1/screenshots")).unwrap()
    }

    #[test]
    fn design_without_verdict_needs_review_and_blocks() {
        let tmp = scaffold(&["s1"]);
        let root = tmp.path();
        put(root, "designs/s1.png", b"design-bytes");
        put(root, "replays/r1/screenshots/s1.png", b"shot");
        let rows = rows_for(root);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "needs-review");
        assert!(rows[0].blocking());
    }

    #[test]
    fn ok_and_accepted_pass_fail_and_ask_block() {
        let tmp = scaffold(&["s1", "s2", "s3", "s4"]);
        let root = tmp.path();
        let mut store = Store::new();
        for (id, kind) in [
            ("s1", "ok"),
            ("s2", "accepted"),
            ("s3", "fail"),
            ("s4", "ask"),
        ] {
            put(root, &format!("designs/{id}.png"), b"design-bytes");
            put(root, &format!("replays/r1/screenshots/{id}.png"), b"shot");
            store.insert(
                id.to_string(),
                Verdict {
                    verdict: kind.to_string(),
                    reason: None,
                    design_hash: hash_scenario_bytes(b"design-bytes"),
                    recorded_at: "now".into(),
                },
            );
        }
        save_store(root, &store).unwrap();
        let rows = rows_for(root);
        let blocking: Vec<&str> = rows
            .iter()
            .filter(|r| r.blocking())
            .map(|r| r.step_id.as_str())
            .collect();
        assert_eq!(blocking, vec!["s3", "s4"]);
    }

    #[test]
    fn re_exported_design_expires_the_verdict() {
        let tmp = scaffold(&["s1"]);
        let root = tmp.path();
        put(root, "designs/s1.png", b"v1-bytes");
        put(root, "replays/r1/screenshots/s1.png", b"shot");
        let mut store = Store::new();
        store.insert(
            "s1".into(),
            Verdict {
                verdict: "ok".into(),
                reason: None,
                design_hash: hash_scenario_bytes(b"v1-bytes"),
                recorded_at: "now".into(),
            },
        );
        save_store(root, &store).unwrap();
        assert_eq!(rows_for(root)[0].status, "ok");

        // Same step, freshly exported frame from Figma.
        put(root, "designs/s1.png", b"v2-bytes");
        let rows = rows_for(root);
        assert_eq!(rows[0].status, "stale");
        assert!(rows[0].blocking());
    }

    #[test]
    fn missing_screenshot_and_unknown_step_are_flagged_not_passed() {
        let tmp = scaffold(&["s1"]);
        let root = tmp.path();
        put(root, "designs/s1.png", b"d");
        put(root, "designs/typo-step.png", b"d");
        let rows = rows_for(root);
        let by: BTreeMap<&str, &str> = rows
            .iter()
            .map(|r| (r.step_id.as_str(), r.status.as_str()))
            .collect();
        assert_eq!(by["s1"], "no-screenshot");
        assert_eq!(by["typo-step"], "unknown-step");
        assert!(rows.iter().all(|r| r.blocking()));
    }

    #[test]
    fn notes_and_verdicts_json_are_not_treated_as_designs() {
        let tmp = scaffold(&["s1"]);
        let root = tmp.path();
        put(root, "designs/s1.png", b"d");
        put(root, "designs/s1.md", b"8px gap, primary right");
        put(root, "replays/r1/screenshots/s1.png", b"shot");
        save_store(root, &Store::new()).unwrap();
        let rows = rows_for(root);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].notes.is_some());
    }

    #[test]
    fn markdown_tells_the_reviewer_to_ignore_text_content() {
        let tmp = scaffold(&["s1"]);
        let root = tmp.path();
        put(root, "designs/s1.png", b"d");
        put(root, "replays/r1/screenshots/s1.png", b"shot");
        let md = render_markdown("sid", "r1", &rows_for(root), Some(1));
        assert!(md.contains("NOT text content"), "got: {md}");
        assert!(md.contains("1 of 1 steps have a design"), "got: {md}");
    }
}
