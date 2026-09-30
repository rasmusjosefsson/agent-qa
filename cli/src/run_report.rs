//! `run-report` verb — render one replay run as a self-contained HTML file.
//!
//! ```
//! agent-qa run-report <sid> [--run <runId>] [--out <path>]
//! ```
//!
//! Reads `<sid>/replays/<run>/events.jsonl` (+ `audit.json`/`status.json`
//! when present) and writes `<run>/report.html` (or `--out`): a verdict
//! header, a per-step table (status, duration, thumbnail, error), and an
//! artifacts index linking every other file in the run dir by relative
//! path — so the report stays correct when the whole run dir is zipped
//! or uploaded as a CI artifact.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let scenario_dir = paths::scenario_dir(&opts.sid)?;
    // `latest` (or no flag) resolves like every audit subverb: replays/
    // latest.txt first, else the highest lex-sorted run dir — so the
    // report still works when latest.txt is absent.
    let run_id =
        crate::audit::resolve_run_id(&scenario_dir, opts.run.as_deref().unwrap_or("latest"))?;
    let run_dir = scenario_dir.join("replays").join(&run_id);
    if !run_dir.is_dir() {
        bail!("run {run_id:?} not found under {}", run_dir.display());
    }
    let out = match &opts.out {
        Some(p) => PathBuf::from(p),
        None => run_dir.join("report.html"),
    };
    let html = render_report(&run_dir, &opts.sid, &run_id)?;
    fs::write(&out, html).with_context(|| format!("write {}", out.display()))?;
    println!("report → {}", out.display());
    Ok(0)
}

#[derive(Debug)]
struct Opts {
    sid: String,
    run: Option<String>,
    out: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        println!(
            "agent-qa run-report — render a replay run as a self-contained HTML file\n\nUsage:\n  agent-qa run-report <sid> [--run <runId>] [--out <path>]\n\nWrites <run>/report.html by default: verdict header, per-step table\n(status / ms / screenshot thumbnail / error), and an index of every\nartifact in the run dir (network.json, console.json, heal.jsonl,\ndiffs/, shots-diff/, video, junit.xml) by relative path."
        );
        std::process::exit(0);
    }
    let mut sid: Option<String> = None;
    let mut run_id: Option<String> = None;
    let mut out: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--run" => {
                run_id = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--run expects a run id"))?
                        .to_string(),
                )
            }
            s if s.starts_with("--run=") => run_id = Some(s["--run=".len()..].to_string()),
            "--out" => {
                out = Some(
                    it.next()
                        .ok_or_else(|| anyhow!("--out expects a path"))?
                        .to_string(),
                )
            }
            s if s.starts_with("--out=") => out = Some(s["--out=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if sid.is_some() {
                    bail!("unexpected positional {other:?}; usage: run-report <sid>");
                }
                sid = Some(other.to_string());
            }
        }
    }
    Ok(Opts {
        sid: sid.ok_or_else(|| anyhow!("usage: run-report <sid> [--run <id>] [--out <p>]"))?,
        run: run_id,
        out,
    })
}

struct StepRow {
    idx: u64,
    id: String,
    intent: String,
    kind: String,
    status: String,
    ms: u64,
    screenshot: Option<String>,
    snapshot: Option<String>,
    error: Option<String>,
}

/// Last terminal row (pass/fail) per step wins; a lone `running` row
/// survives so interrupted runs still show where they stopped.
fn collect_steps(events: &[Json]) -> Vec<StepRow> {
    let mut by_id: BTreeMap<String, StepRow> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for row in events {
        let id = match row.get("id").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let status = row
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let entry = StepRow {
            idx: row.get("idx").and_then(|v| v.as_u64()).unwrap_or(0),
            intent: row
                .get("intent")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            kind: row
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            ms: row.get("ms").and_then(|v| v.as_u64()).unwrap_or(0),
            screenshot: row
                .get("screenshot")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            snapshot: row
                .get("snapshot")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            error: row
                .get("error")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            status: status.clone(),
            id: id.clone(),
        };
        match by_id.get(&id) {
            None => {
                order.push(id.clone());
                by_id.insert(id, entry);
            }
            Some(existing)
                if existing.status == "running" || status == "pass" || status == "fail" =>
            {
                by_id.insert(id, entry);
            }
            _ => {}
        }
    }
    order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect()
}

/// Files/dirs surfaced in the artifacts index when present in the run dir.
const ARTIFACT_FILES: &[&str] = &[
    "events.jsonl",
    "audit.json",
    "status.json",
    "heal.jsonl",
    "network.json",
    "console.json",
    "junit.xml",
    "run.webm",
    "junit.html",
];
const ARTIFACT_DIRS: &[&str] = &[
    "screenshots",
    "snapshots",
    "diffs",
    "shots-diff",
    "domshots-diff",
    "heal-responses",
];

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_report(run_dir: &Path, sid: &str, run_id: &str) -> Result<String> {
    let body = fs::read_to_string(run_dir.join("events.jsonl"))
        .with_context(|| format!("read {}", run_dir.join("events.jsonl").display()))?;
    let events: Vec<Json> = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let steps = collect_steps(&events);

    let audit = fs::read_to_string(run_dir.join("audit.json"))
        .ok()
        .and_then(|b| serde_json::from_str::<Json>(&b).ok());

    let total = steps.iter().filter(|s| s.status != "running").count();
    let passed = steps.iter().filter(|s| s.status == "pass").count();
    let failed = steps.iter().filter(|s| s.status == "fail").count();
    let running = steps.iter().filter(|s| s.status == "running").count();
    let verdict = if failed > 0 {
        "FAIL"
    } else if running > 0 {
        "INTERRUPTED"
    } else {
        "PASS"
    };
    let summary = audit
        .as_ref()
        .and_then(|a| a.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let started = audit
        .as_ref()
        .and_then(|a| a.get("startedAt"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let finished = audit
        .as_ref()
        .and_then(|a| a.get("finishedAt"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut rows = String::new();
    for s in &steps {
        let cls = match s.status.as_str() {
            "pass" => "pass",
            "fail" => "fail",
            _ => "running",
        };
        let shot = match &s.screenshot {
            Some(p) => format!(
                "<a href=\"{}\"><img class=\"thumb\" src=\"{}\" loading=\"lazy\" alt=\"{}\"></a>",
                esc(p),
                esc(p),
                esc(&s.id)
            ),
            None => String::new(),
        };
        let err = match &s.error {
            Some(e) => format!("<div class=\"err\">{}</div>", esc(e)),
            None => String::new(),
        };
        let snap = match &s.snapshot {
            Some(p) => format!(" <a href=\"{}\">snapshot</a>", esc(p)),
            None => String::new(),
        };
        rows.push_str(&format!(
            "<tr class=\"{cls}\"><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td>\
             <td><span class=\"pill {cls}\">{}</span></td><td>{}</td><td>{}{}</td><td>{}</td></tr>\n",
            s.idx,
            esc(&s.id),
            esc(&s.kind),
            esc(&s.intent),
            esc(&s.status),
            s.ms,
            shot,
            err,
            snap
        ));
    }

    let mut artifacts = String::new();
    let mut push_link = |name: &str, href: &str, extra: &str| {
        artifacts.push_str(&format!(
            "<li><a href=\"{}\"><code>{}</code></a>{}</li>\n",
            esc(href),
            esc(name),
            extra
        ));
    };
    for f in ARTIFACT_FILES {
        if run_dir.join(f).is_file() {
            push_link(f, f, "");
        }
    }
    for d in ARTIFACT_DIRS {
        let dir = run_dir.join(d);
        if dir.is_dir() {
            let n = fs::read_dir(&dir).map(|it| it.count()).unwrap_or(0);
            push_link(&format!("{d}/"), d, &format!(" — {n} file(s)"));
        }
    }
    let artifacts_html = if artifacts.is_empty() {
        "<p class=\"dim\">(no side artifacts)</p>".to_string()
    } else {
        format!("<ul class=\"artifacts\">{artifacts}</ul>")
    };

    let meta = if !started.is_empty() || !finished.is_empty() {
        format!(
            "<p class=\"dim\">started {} · finished {}</p>",
            esc(started),
            esc(finished)
        )
    } else {
        String::new()
    };
    let summary_line = if summary.is_empty() {
        String::new()
    } else {
        format!(
            "<p class=\"summary-line\"><code>{}</code></p>",
            esc(&summary)
        )
    };
    let verdict_cls = match verdict {
        "PASS" => "pass",
        "FAIL" => "fail",
        _ => "running",
    };

    Ok(format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<title>agent-qa run report — {sid} / {run_id}</title>
<style>
body {{ font: 14px/1.45 system-ui, sans-serif; margin: 2rem; color: #1a1a1a; }}
code {{ font-family: ui-monospace, monospace; font-size: .92em; }}
h1 {{ font-size: 1.3rem; margin: 0 0 .25rem; }}
.verdict {{ display: inline-block; font-weight: 700; font-size: 1.05rem;
  padding: .15rem .7rem; border-radius: .4rem; margin: .4rem 0; }}
.verdict.pass {{ background: #dcfce7; color: #166534; }}
.verdict.fail {{ background: #fee2e2; color: #991b1b; }}
.verdict.running {{ background: #fef9c3; color: #854d0e; }}
.summary-line code {{ background: #f3f4f6; padding: .15rem .4rem; border-radius: .3rem; }}
.dim {{ color: #6b7280; font-size: .85rem; }}
table {{ border-collapse: collapse; width: 100%; margin-top: 1rem; }}
th, td {{ text-align: left; padding: .4rem .6rem; border-bottom: 1px solid #e5e7eb;
  vertical-align: top; }}
th {{ font-size: .8rem; text-transform: uppercase; letter-spacing: .04em; color: #6b7280; }}
tr.fail td {{ background: #fff5f5; }}
.pill {{ padding: .1rem .5rem; border-radius: 999px; font-size: .78rem; font-weight: 600; }}
.pill.pass {{ background: #dcfce7; color: #166534; }}
.pill.fail {{ background: #fee2e2; color: #991b1b; }}
.pill.running {{ background: #fef9c3; color: #854d0e; }}
.err {{ color: #991b1b; font-size: .82rem; max-width: 42rem; white-space: pre-wrap; }}
.thumb {{ max-width: 160px; max-height: 100px; border: 1px solid #e5e7eb;
  border-radius: .3rem; }}
.artifacts {{ padding-left: 1.2rem; }}
.artifacts li {{ margin: .15rem 0; }}
h2 {{ font-size: 1rem; margin-top: 1.6rem; }}
</style></head><body>
<h1>agent-qa run report</h1>
<p><code>{sid}</code> · run <code>{run_id}</code></p>
{meta}
<span class="verdict {verdict_cls}">{verdict}</span>
<p class="dim">{passed}/{total} passed · {failed} failed · {running} interrupted</p>
{summary_line}
<h2>Steps</h2>
<table><thead><tr><th>#</th><th>id</th><th>kind</th><th>intent</th><th>status</th>
<th>ms</th><th>screenshot / error</th><th>snapshot</th></tr></thead>
<tbody>
{rows}</tbody></table>
<h2>Artifacts</h2>
{artifacts_html}
</body></html>
"#,
        sid = esc(sid),
        run_id = esc(run_id),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp);
    }
    fn teardown() {
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
    }

    fn make_run(tmp: &Path, sid: &str, run_id: &str) -> PathBuf {
        let dir = tmp.join(sid).join("replays").join(run_id);
        fs::create_dir_all(dir.join("screenshots")).unwrap();
        fs::create_dir_all(dir.join("snapshots")).unwrap();
        fs::write(dir.join("screenshots").join("s0.png"), b"png").unwrap();
        fs::write(dir.join("snapshots").join("s0.txt"), b"snap").unwrap();
        let rows = [
            json!({"idx":1,"total":2,"id":"s0","intent":"open","kind":"do:goto","status":"running"}),
            json!({"idx":1,"total":2,"id":"s0","intent":"open","kind":"do:goto","status":"pass","ms":100,"screenshot":"screenshots/s0.png","snapshot":"snapshots/s0.txt"}),
            json!({"idx":2,"total":2,"id":"s1","intent":"boom <script>","kind":"do:click","status":"running"}),
            json!({"idx":2,"total":2,"id":"s1","intent":"boom <script>","kind":"do:click","status":"fail","ms":50,"error":"step 's1' <bad> & \"quoted\""}),
        ];
        let body: String = rows
            .iter()
            .map(|r| serde_json::to_string(r).unwrap() + "\n")
            .collect();
        fs::write(dir.join("events.jsonl"), body).unwrap();
        fs::write(
            dir.join("audit.json"),
            r#"{"runId":"R","scenarioId":"S","startedAt":"t0","finishedAt":"t1","exitCode":1,"summary":"SUMMARY: 1/2 (FAIL)"}"#,
        )
        .unwrap();
        fs::write(dir.join("heal.jsonl"), b"{}\n").unwrap();
        dir
    }

    #[test]
    fn report_renders_verdict_last_row_wins_and_escapes() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let dir = make_run(tmp.path(), "j1", "rA");
        let html = render_report(&dir, "j1", "rA").unwrap();
        assert!(html.contains(">FAIL<"));
        assert!(html.contains("SUMMARY: 1/2 (FAIL)"));
        // running rows were replaced by their terminal rows
        assert!(!html.contains(">running</span>") || html.matches("running").count() <= 2);
        assert!(html.contains("boom &lt;script&gt;"));
        assert!(html.contains("&lt;bad&gt; &amp; &quot;quoted&quot;"));
        assert!(html.contains("screenshots/s0.png"));
        assert!(html.contains("heal.jsonl"));
        assert!(html.contains("screenshots/"));
        teardown();
    }

    #[test]
    fn run_report_writes_into_run_dir_by_default() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        make_run(tmp.path(), "j1", "rA");
        fs::write(tmp.path().join("j1/replays/latest.txt"), "rA").unwrap();
        run(&["j1".into()]).unwrap();
        let p = tmp.path().join("j1/replays/rA/report.html");
        assert!(p.is_file());
        assert!(fs::read_to_string(p)
            .unwrap()
            .contains("agent-qa run report"));
        teardown();
    }

    #[test]
    fn run_report_run_latest_resolves_without_latest_txt() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        make_run(tmp.path(), "j1", "rA");
        make_run(tmp.path(), "j1", "rB");
        // No replays/latest.txt — 'latest' falls back to the lex-max dir.
        run(&["j1".into(), "--run".into(), "latest".into()]).unwrap();
        assert!(tmp.path().join("j1/replays/rB/report.html").is_file());
        teardown();
    }

    #[test]
    fn run_report_out_flag_overrides_destination() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        make_run(tmp.path(), "j1", "rA");
        let out = tmp.path().join("elsewhere.html");
        run(&[
            "j1".into(),
            "--run".into(),
            "rA".into(),
            format!("--out={}", out.display()),
        ])
        .unwrap();
        assert!(out.is_file());
        teardown();
    }
}
