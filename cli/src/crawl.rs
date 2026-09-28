//! `crawl <url>` — coverage scaffolding: open a page in the current session,
//! enumerate same-origin links + interactive elements, and write a DRAFT
//! scenario.json that visits each discovered route and asserts a shot claim
//! on each (instant visual-coverage skeleton a human or agent then refines
//! into real flows). Also writes `crawl-report.json` — the interactive-element
//! inventory — next to it, as authoring input; nothing consumes it.
//!
//! Deliberately deterministic: no LLM call, no clicking (clicks mutate/navigate
//! — a scaffold shouldn't do either). `--depth N` follows links BFS-style:
//! depth 1 (default) covers the entry page's links; each extra level opens the
//! discovered pages and merges their same-origin links, capped by `--max`.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::browser;
use crate::paths;

const MAX_DEFAULT: usize = 20;
const DEPTH_MAX: usize = 4;

/// `crawl <url> [--session <name>] [--out <dir>] [--max N] [--sid <name>]`
pub fn run(args: &[String]) -> Result<u8> {
    let mut url: Option<String> = None;
    let mut session = "default".to_string();
    let mut out_dir: Option<PathBuf> = None;
    let mut max_links = MAX_DEFAULT;
    let mut depth = 1usize;
    let mut sid_override: Option<String> = None;
    let mut console_checks = true;

    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa crawl — draft a coverage scenario from a live page\n\nUsage:\n  agent-qa crawl <url> [options]\n\nOptions:\n  --session <name>   Browser session to drive (default: default)\n  --out <dir>        Output dir for scenario.json + crawl-report.json\n                     (default: <scenarios_root>/crawl-<host>)\n  --max <N>          Max same-origin links to cover (default {MAX_DEFAULT})
  --depth <N>        BFS depth — 1 (default) covers entry-page links only;
                     each level deeper opens discovered pages and merges
                     their links (cap {DEPTH_MAX})\n  --sid <name>       Scenario id (default: crawl-<host>)\n  --no-console-checks\n                     Skip the per-page 'no console errors' claims\n                     (on by default — a page that crashes JS isn't green).\n\nThe draft is a goto + shot claim + console check per discovered route —\nrun `replay`, then `shot-accept` to mint baselines."
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
            "--depth" => {
                depth = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|n| (1..=DEPTH_MAX).contains(n))
                    .ok_or_else(|| {
                        anyhow::anyhow!("--depth expects an integer in 1..={DEPTH_MAX}")
                    })?;
            }
            "--no-console-checks" => console_checks = false,
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

    let page = enumerate_page(&session, &url)?;

    // BFS route discovery. `seen` holds every URL already queued or covered
    // (seeded with the entry URL so self-links are noise); `frontier` is the
    // current level's pages to open. Depth 1 = the historical behaviour.
    let mut seen: std::collections::BTreeSet<String> = [url.clone()].into_iter().collect();
    let mut links = page_links(&page, &mut seen);
    let mut frontier = links.clone();
    for level in 2..=depth {
        if frontier.is_empty() || links.len() >= max_links {
            break;
        }
        let mut next: Vec<String> = Vec::new();
        for link in &frontier {
            if links.len() >= max_links {
                break;
            }
            // A discovered page that 404s or hangs the eval shouldn't sink
            // the crawl — log and move on.
            let sub = match enumerate_page(&session, link) {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("crawl: level {level}: skipping {link} ({err:#})");
                    continue;
                }
            };
            for fresh in page_links(&sub, &mut seen) {
                links.push(fresh.clone());
                next.push(fresh);
            }
        }
        frontier = next;
    }
    links.sort();
    links.dedup();
    links.truncate(max_links);

    let mut steps: Vec<Value> = Vec::new();
    // One coverage unit: goto + shot claim (+ optional console-error check).
    let cover = |steps: &mut Vec<Value>,
                 i: &mut usize,
                 label: &str,
                 link: &str,
                 console_checks: bool| {
        *i += 1;
        steps.push(json!({"id":format!("s{}",*i),"intent":format!("open {link}"),"kind":"do","verb":"goto","value":{"from":"literal","literal":link}}));
        let n = *i;
        *i += 1;
        steps.push(json!({"id":format!("s{}",*i),"intent":format!("{label} renders"),"kind":"check","claim":{"subject":{"shot":format!("s{n}")},"predicate":"matches"}}));
        if console_checks {
            *i += 1;
            steps.push(json!({"id":format!("s{}",*i),"intent":format!("{label}: no console errors"),"kind":"check","claim":{"subject":{"console":{"type":"error"}},"predicate":"notExists"}}));
        }
    };

    let mut idx = 0usize;
    cover(&mut steps, &mut idx, "entry page", &url, console_checks);

    let mut covered = 0usize;
    for link in &links {
        // Skip non-navigable links (mailto/tel/javascript: are filtered
        // upstream by the starts-with-origin check, but fragments and files
        // like .pdf/.zip would still produce useless shots).
        let pathish = link.rsplit('/').next().unwrap_or("");
        if link.contains('#') || pathish.contains('.') && !pathish.ends_with(".html") {
            continue;
        }
        cover(&mut steps, &mut idx, link, link, console_checks);
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
            "depth": depth,
            "linksFound": links.len(),
            "routesCovered": covered,
            "interactive": page.get("interactive").cloned().unwrap_or(json!([])),
        }))?,
    )
    .with_context(|| format!("crawl: write {}", report_path.display()))?;

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
    println!("  next:     agent-qa replay {sid} && agent-qa shot-accept {sid}");
    Ok(0)
}

/// Open `url` in `session` and enumerate its same-origin links + interactive
/// elements (title/links/interactive JSON inventory).
fn enumerate_page(session: &str, url: &str) -> Result<Value> {
    browser::open(session, url).with_context(|| format!("crawl: open {url}"))?;
    browser::wait_for_load_capped(session, "networkidle", 5000).ok();
    // SPAs mount their nav links after networkidle — an inventory with zero
    // links on first paint is a render race, not a link-less page. Re-poll a
    // few times before accepting an empty set.
    let mut last: Option<Value> = None;
    for attempt in 0..5 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        let raw = browser::eval_expression(
            session,
            "(() => { const links=[...document.querySelectorAll('a[href]')].map(a=>a.href).filter(h=>h.startsWith(location.origin)); const els=[...document.querySelectorAll('button,[role=button],input,select,textarea,[onclick]')].map(e=>({tag:e.tagName.toLowerCase(),role:e.getAttribute('role'),text:(e.innerText||e.value||'').trim().slice(0,60),testid:e.getAttribute('data-testid')})).slice(0,50); return JSON.stringify({title:document.title,links:[...new Set(links)],interactive:els}); })()",
        )
        .context("crawl: enumerate page")?;
        let page = parse_inventory(&raw)?;
        let has_links = page
            .get("links")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if has_links || attempt == 4 {
            return Ok(page);
        }
        last = Some(page);
    }
    last.context("crawl: enumerate page")
}

/// agent-browser returns the in-page value JSON-encoded, so a
/// JSON.stringify'd result arrives as a *string literal* holding the object —
/// parse once, then unwrap one string layer if the result is a Value::String.
fn parse_inventory(raw: &str) -> Result<Value> {
    let first: Value = serde_json::from_str(raw.trim()).context("crawl: parse inventory")?;
    match first.as_str() {
        Some(inner) => serde_json::from_str(inner).context("crawl: parse inventory"),
        None => Ok(first),
    }
}

/// Same-origin links from an inventory, minus anything in `seen` (which this
/// mutates). Discovery order — callers re-sort the final list.
fn page_links(page: &Value, seen: &mut std::collections::BTreeSet<String>) -> Vec<String> {
    page.get("links")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .filter(|l| seen.insert(l.to_string()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
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
    fn page_links_dedups_against_seen() {
        let mut seen: std::collections::BTreeSet<String> =
            ["https://a/".to_string()].into_iter().collect();
        let page = json!({"links": ["https://a/", "https://a/x", "https://a/x", "https://a/y"]});
        let got = page_links(&page, &mut seen);
        assert_eq!(got, vec!["https://a/x", "https://a/y"]);
        // A second page re-offering /x contributes only the fresh link.
        let sub = json!({"links": ["https://a/x", "https://a/z"]});
        assert_eq!(page_links(&sub, &mut seen), vec!["https://a/z"]);
        // Cross-origin links never leave the host (enumerator filters, but
        // the set guard is defence in depth).
        assert!(seen.iter().all(|l| l.starts_with("https://a")));
    }

    #[test]
    fn parse_inventory_unwraps_agent_browser_string_encoding() {
        // `eval` JSON-encodes the return value: a JSON.stringify'd object
        // arrives as `"{\"title\":...}"`. The first parse yields a
        // Value::String — the inventory lives one layer down.
        let inner = json!({"title":"app","links":["https://a/x"],"interactive":[]});
        let raw = serde_json::to_string(&inner.to_string()).unwrap();
        let page = parse_inventory(&raw).unwrap();
        assert_eq!(page["links"][0], "https://a/x");
        // A bare-object stdout (no encoding layer) parses directly.
        let plain = parse_inventory(&inner.to_string()).unwrap();
        assert_eq!(plain["title"], "app");
        parse_inventory("not json").unwrap_err();
    }

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
