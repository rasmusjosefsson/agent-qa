//! `crawl <url>` — coverage scaffolding: open a page in the current session,
//! enumerate same-origin links + interactive elements, and write a DRAFT
//! scenario.json that visits each discovered route and asserts a shot claim
//! on each (instant visual-coverage skeleton a human or agent then refines
//! into real flows). Also writes `crawl-report.json` — the interactive-element
//! inventory — next to it, as authoring input; nothing consumes it.
//!
//! Deliberately deterministic: no LLM call, no clicking (clicks mutate/navigate
//! — a scaffold shouldn't do either). Route discovery is one level deep; run
//! again from a discovered route for a deeper crawl.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::browser;
use crate::paths;

const MAX_DEFAULT: usize = 20;

/// `crawl <url> [--session <name>] [--out <dir>] [--max N] [--sid <name>]`
pub fn run(args: &[String]) -> Result<u8> {
    let mut url: Option<String> = None;
    let mut session = "default".to_string();
    let mut out_dir: Option<PathBuf> = None;
    let mut max_links = MAX_DEFAULT;
    let mut sid_override: Option<String> = None;
    let mut console_checks = true;
    let mut mint_baselines = false;

    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa crawl — draft a coverage scenario from a live page\n\nUsage:\n  agent-qa crawl <url> [options]\n\nOptions:\n  --session <name>   Browser session to drive (default: default)\n  --out <dir>        Output dir for scenario.json + crawl-report.json\n                     (default: <scenarios_root>/crawl-<host>)\n  --max <N>          Max same-origin links to cover (default {MAX_DEFAULT})\n  --sid <name>       Scenario id (default: crawl-<host>)\n  --no-console-checks\n                     Skip the per-page 'no console errors' claims\n                     (on by default — a page that crashes JS isn't green).\n\nThe draft is a goto + shot claim + console check per discovered route —\nrun `replay`, then `shot-accept` to mint baselines.\n\n  --mint-baselines\n                     After writing the draft, visit every covered route\n                     and capture baselines/<gotoStepId>.png — the draft\n                     replays green immediately, no shot-accept pass."
                );
                return Ok(0);
            }
            "--session" => {
                session = it
                    .next()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("--session requires a value"))?;
            }
            "--out" => {
                out_dir = Some(PathBuf::from(
                    it.next()
                        .ok_or_else(|| anyhow::anyhow!("--out requires a value"))?,
                ));
            }
            "--max" => {
                max_links = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow::anyhow!("--max expects a positive integer"))?;
            }
            "--sid" => {
                sid_override = Some(
                    it.next()
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("--sid requires a value"))?,
                );
            }
            "--no-console-checks" => console_checks = false,
            "--mint-baselines" => mint_baselines = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => url = Some(other.to_string()),
        }
    }

    let url = url.ok_or_else(|| {
        anyhow::anyhow!(
            "usage: agent-qa crawl <url> [--session <name>] [--out <dir>] [--max N] [--sid <name>]"
        )
    })?;
    let host = url::host_of(&url);
    let sid = sid_override.unwrap_or_else(|| format!("crawl-{}", sanitize(&host)));
    let dir = out_dir.unwrap_or_else(|| paths::scenarios_root().join(&sid));

    browser::open(&session, &url).with_context(|| format!("crawl: open {url}"))?;
    browser::wait_for_load_capped(&session, "networkidle", 5000).ok();

    let raw = browser::eval_expression(
        &session,
        "(() => { const links=[...document.querySelectorAll('a[href]')].map(a=>a.href).filter(h=>h.startsWith(location.origin)); const els=[...document.querySelectorAll('button,[role=button],input,select,textarea,[onclick]')].map(e=>({tag:e.tagName.toLowerCase(),role:e.getAttribute('role'),text:(e.innerText||e.value||'').trim().slice(0,60),testid:e.getAttribute('data-testid')})).slice(0,50); return JSON.stringify({title:document.title,links:[...new Set(links)],interactive:els}); })()",
    )
    .context("crawl: enumerate page")?;
    let peeled = raw.trim().trim_matches('"').replace("\\\"", "\"");
    let page: Value = serde_json::from_str(raw.trim())
        .or_else(|_| serde_json::from_str(&peeled))
        .context("crawl: parse inventory")?;

    let mut links: Vec<String> = page
        .get("links")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    // Stable order, self-link first is noise — drop the bare root URL.
    links.retain(|l| l != &url);
    links.sort();
    links.dedup();
    links.truncate(max_links);

    let mut steps: Vec<Value> = Vec::new();
    // (link, shot-subject step id) per covered route — the shot claim on
    // the goto step is where `baselines/<id>.png` must land.
    let mut shot_targets: Vec<(String, String)> = Vec::new();
    // One coverage unit: goto + shot claim (+ optional console-error check).
    let cover = |steps: &mut Vec<Value>,
                 i: &mut usize,
                 label: &str,
                 link: &str,
                 console_checks: bool,
                 shot_targets: &mut Vec<(String, String)>| {
        *i += 1;
        let goto_id = format!("s{}", *i);
        steps.push(json!({"id":goto_id,"intent":format!("open {link}"),"kind":"do","verb":"goto","value":{"from":"literal","literal":link}}));
        let n = *i;
        shot_targets.push((link.to_string(), format!("s{n}")));
        *i += 1;
        steps.push(json!({"id":format!("s{}",*i),"intent":format!("{label} renders"),"kind":"check","claim":{"subject":{"shot":format!("s{n}")},"predicate":"matches"}}));
        if console_checks {
            *i += 1;
            steps.push(json!({"id":format!("s{}",*i),"intent":format!("{label}: no console errors"),"kind":"check","claim":{"subject":{"console":{"type":"error"}},"predicate":"notExists"}}));
        }
    };

    let mut idx = 0usize;
    cover(
        &mut steps,
        &mut idx,
        "entry page",
        &url,
        console_checks,
        &mut shot_targets,
    );

    let mut covered = 0usize;
    for link in &links {
        // Skip non-navigable links (mailto/tel/javascript: are filtered
        // upstream by the starts-with-origin check, but fragments and files
        // like .pdf/.zip would still produce useless shots).
        let pathish = link.rsplit('/').next().unwrap_or("");
        if link.contains('#') || pathish.contains('.') && !pathish.ends_with(".html") {
            continue;
        }
        cover(
            &mut steps,
            &mut idx,
            link,
            link,
            console_checks,
            &mut shot_targets,
        );
        covered += 1;
    }

    let title = page.get("title").and_then(|v| v.as_str()).unwrap_or(&host);
    let doc = json!({
        "schema": "scenario/2",
        "id": sid,
        "intent": format!("coverage crawl of {url} — {title}"),
        "steps": steps,
    });

    fs::create_dir_all(&dir).with_context(|| format!("crawl: mkdir {}", dir.display()))?;
    let scenario_path = dir.join("scenario.json");
    fs::write(&scenario_path, serde_json::to_string_pretty(&doc)?)
        .with_context(|| format!("crawl: write {}", scenario_path.display()))?;

    let report_path = dir.join("crawl-report.json");
    fs::write(
        &report_path,
        serde_json::to_string_pretty(&json!({
            "url": url,
            "title": title,
            "linksFound": links.len(),
            "routesCovered": covered,
            "interactive": page.get("interactive").cloned().unwrap_or(json!([])),
        }))?,
    )
    .with_context(|| format!("crawl: write {}", report_path.display()))?;

    if mint_baselines {
        let baselines = dir.join("baselines");
        fs::create_dir_all(&baselines).ok();
        let mut minted = 0usize;
        for (link, shot_id) in &shot_targets {
            if browser::open(&session, link).is_err() {
                eprintln!("crawl: mint {shot_id} skipped — {link} failed to open");
                continue;
            }
            browser::wait_for_load_capped(&session, "networkidle", 5000).ok();
            let dest = baselines.join(format!("{shot_id}.png"));
            match browser::screenshot(&session, &dest, true, Some(10_000)) {
                Ok(true) => minted += 1,
                _ => eprintln!("crawl: mint {shot_id} skipped — screenshot failed"),
            }
        }
        println!("crawl: minted {minted}/{} baseline(s)", shot_targets.len());
    }

    println!(
        "crawl: {sid} — {covered} route(s) + entry, {} shot claim(s){}",
        steps.len() / if console_checks { 3 } else { 2 },
        if console_checks {
            " + console-error checks"
        } else {
            ""
        }
    );
    println!("  scenario: {}", scenario_path.display());
    println!(
        "  report:   {} ({} interactive elements)",
        report_path.display(),
        page.get("interactive")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );
    if mint_baselines {
        println!("  next:     agent-qa replay {sid}   (baselines already minted)");
    } else {
        println!("  next:     agent-qa replay {sid} && agent-qa shot-accept {sid}");
    }
    Ok(0)
}

fn sanitize(host: &str) -> String {
    host.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// URL helpers kept local — the crate doesn't take a url dep for one host parse.
mod url {
    pub fn host_of(url: &str) -> String {
        let after_scheme = url.split("://").nth(1).unwrap_or(url);
        after_scheme
            .split(['/', '?', '#', ':'])
            .next()
            .unwrap_or("")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_parse_and_sanitize() {
        assert_eq!(
            url::host_of("https://app.example.com:8443/x"),
            "app.example.com"
        );
        assert_eq!(url::host_of("http://localhost:3000"), "localhost");
        assert_eq!(url::host_of("example.com/path"), "example.com");
        assert_eq!(sanitize("app.example.com"), "app-example-com");
        assert_eq!(sanitize("localhost"), "localhost");
    }
}
