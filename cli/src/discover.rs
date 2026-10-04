//! `discover <url> --changed <file>|--changed-git <ref>` — the PR-diff-scoped
//! cousin of `crawl`: map a set of changed paths to the routes they plausibly
//! render, then write a DRAFT scenario covering just those routes (goto +
//! shot claim + console check each). This is the deterministic slice of
//! "read the PR diff → cover the changed surface" — no model call, no
//! guessing: paths that don't map to a route are *reported* in
//! `discover-report.json`, never silently dropped.
//!
//! Route mapping is by convention, not config:
//!   `**/pages|app|routes|views/**` marks a routing root; the segments after
//!   it become the URL path. `page.*`/`route.*`/`layout.*` leaves mean
//!   dir-based routing (app router), plain file leaves mean file-based
//!   routing (pages router). `index` drops. Dynamic segments ([id], $id,
//!   _id) can't produce a concrete URL — reported as `dynamic`, not guessed.
//!
//! The emitted scenario carries `onlyWhen` = the changed paths that mapped,
//! so the draft is self-scoping: `replay --all --changed-git origin/main`
//! re-runs it exactly when those files change again.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::paths;

const MAX_DEFAULT: usize = 20;

/// Marker directories that can start a route path — order matters for
/// app-router `page.*`/`route.*` leaves (checked per marker).
const ROUTE_MARKERS: &[&str] = &["pages", "app", "routes", "views"];

/// Extensions that can render a page when they sit under a route marker.
const PAGE_EXTS: &[&str] = &[
    "tsx", "jsx", "ts", "js", "mjs", "vue", "svelte", "astro", "html", "mdx", "md",
];

/// `discover <url> [--routes r1,r2] (--changed <file> | --changed-git <ref>)`
pub fn run(args: &[String]) -> Result<u8> {
    let mut url: Option<String> = None;
    let mut changed_file: Option<PathBuf> = None;
    let mut changed_git: Option<String> = None;
    let mut extra_routes: Vec<String> = Vec::new();
    let mut out_dir: Option<PathBuf> = None;
    let mut sid_override: Option<String> = None;
    let mut max = MAX_DEFAULT;

    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa discover — draft a scenario for the routes a change set touches\n\nUsage:\n  agent-qa discover <base-url> --changed <paths-file>\n  agent-qa discover <base-url> --changed-git <ref>\n  agent-qa discover <base-url> --routes /a,/b\n\nOptions:\n  --changed <file>   Newline-separated changed paths (e.g. from\n                     `git diff --name-only`)\n  --changed-git <ref>\n                     Diff cwd's git repo against <ref> for the path list\n  --routes a,b       Extra literal routes to cover regardless of mapping\n  --out <dir>        Output dir (default: <scenarios_root>/discover-<host>)\n  --sid <name>       Scenario id (default: discover-<host>)\n  --max <N>          Max routes to cover (default {MAX_DEFAULT})\n\nWrites scenario.json + discover-report.json (mapped/unmapped paths).\nThe scenario's onlyWhen is seeded with the changed paths — replay with\n`--all --changed-git <ref>` re-runs it when those files change again."
                );
                return Ok(0);
            }
            "--changed" => {
                changed_file =
                    Some(PathBuf::from(it.next().ok_or_else(|| {
                        anyhow::anyhow!("--changed requires a file")
                    })?));
            }
            "--changed-git" => {
                changed_git = Some(
                    it.next()
                        .ok_or_else(|| anyhow::anyhow!("--changed-git requires a ref"))?
                        .clone(),
                );
            }
            "--routes" => {
                extra_routes = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--routes requires a value"))?
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "--out" => {
                out_dir = Some(PathBuf::from(
                    it.next()
                        .ok_or_else(|| anyhow::anyhow!("--out requires a value"))?,
                ));
            }
            "--sid" => {
                sid_override = Some(
                    it.next()
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("--sid requires a value"))?,
                );
            }
            "--max" => {
                max = it
                    .next()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| anyhow::anyhow!("--max expects a positive integer"))?;
            }
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => url = Some(other.to_string()),
        }
    }

    let url = url.ok_or_else(|| {
        anyhow::anyhow!(
            "usage: agent-qa discover <base-url> --changed <file>|--changed-git <ref>|--routes a,b"
        )
    })?;

    let changed = if let Some(f) = &changed_file {
        fs::read_to_string(f)
            .with_context(|| format!("--changed: read {}", f.display()))?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    } else if let Some(r) = &changed_git {
        crate::runner::changed_paths_from_git(r, &std::env::current_dir()?)?
    } else {
        Vec::new()
    };
    if changed.is_empty() && extra_routes.is_empty() {
        bail!("nothing to discover — pass --changed <file>, --changed-git <ref>, or --routes a,b");
    }

    // Map paths → routes; keep an honest report of what didn't map.
    let mut mapped: Vec<Json> = Vec::new();
    let mut unmapped: Vec<Json> = Vec::new();
    let mut routes: Vec<String> = Vec::new();
    for p in &changed {
        match path_to_route(p) {
            RouteHit::Route(r) => {
                mapped.push(json!({"path": p, "route": r}));
                if !routes.contains(&r) {
                    routes.push(r);
                }
            }
            RouteHit::Dynamic(r) => {
                unmapped.push(
                    json!({"path": p, "reason": format!("dynamic route {r:?} needs a concrete segment — pass it via --routes")}),
                );
            }
            RouteHit::None => {
                unmapped.push(json!({"path": p, "reason": "not a route file"}));
            }
        }
    }
    for r in &extra_routes {
        let r = if r.starts_with('/') {
            r.clone()
        } else {
            format!("/{r}")
        };
        if !routes.contains(&r) {
            routes.push(r);
        }
    }
    routes.sort();
    routes.truncate(max);

    if routes.is_empty() {
        bail!(
            "no routes mapped from {} changed paths — see unmapped reasons (use --routes to name routes directly)",
            changed.len()
        );
    }

    let host = crate::crawl::url::host_of(&url);
    let sid = sid_override.unwrap_or_else(|| format!("discover-{}", sanitize(&host)));
    let dir = out_dir.unwrap_or_else(|| paths::scenarios_root().join(&sid));
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;

    // Draft: goto + shot + console check per route — same coverage unit
    // crawl emits, so a discovered draft mints and replays identically.
    let mut steps: Vec<Json> = Vec::new();
    let mut idx = 0usize;
    for route in &routes {
        let target = format!("{}{}", url.trim_end_matches('/'), route);
        idx += 1;
        let goto_id = format!("s{idx}");
        steps.push(json!({"id":goto_id,"intent":format!("open {target}"),"kind":"do","verb":"goto","value":{"from":"literal","literal":target}}));
        idx += 1;
        steps.push(json!({"id":format!("s{idx}"),"intent":format!("{route} renders"),"kind":"check","claim":{"subject":{"shot":goto_id},"predicate":"matches"}}));
        idx += 1;
        steps.push(json!({"id":format!("s{idx}"),"intent":format!("{route}: no console errors"),"kind":"check","claim":{"subject":{"console":{"type":"error"}},"predicate":"notExists"}}));
    }

    let mut scenario = json!({
        "schema": "scenario/2",
        "id": sid,
        "intent": format!("changed-surface coverage of {host}"),
        "steps": steps,
    });
    // Self-scoping: the paths that produced the covered routes double as
    // onlyWhen globs — the draft re-runs when its own sources change, not
    // when an unrelated docs file does.
    let scoped: Vec<String> = mapped
        .iter()
        .filter_map(|m| m["path"].as_str().map(str::to_string))
        .collect();
    if !scoped.is_empty() {
        scenario["onlyWhen"] = json!(scoped);
    }
    let scenario_path = dir.join("scenario.json");
    fs::write(&scenario_path, serde_json::to_string_pretty(&scenario)?)
        .with_context(|| format!("write {}", scenario_path.display()))?;

    let report = json!({
        "baseUrl": url,
        "changed": changed.len(),
        "mapped": mapped,
        "unmapped": unmapped,
        "routes": routes,
        "sid": sid,
    });
    fs::write(
        dir.join("discover-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;

    // plan.md — the reviewable half of the draft: what will be covered,
    // what was skipped, and where to add deeper checks before minting.
    let mut plan = String::new();
    plan.push_str(&format!("# Test plan — {sid}\n\n"));
    plan.push_str(&format!("Base: {url}\n\n"));
    plan.push_str("## Coverage\n\n");
    for route in &routes {
        let target = format!("{}{}", url.trim_end_matches('/'), route);
        plan.push_str(&format!("### `{route}`\n\n"));
        plan.push_str(&format!("- open {target}\n"));
        plan.push_str(&format!("- `{route}` matches golden screenshot\n"));
        plan.push_str(&format!("- `{route}` logs no console errors\n\n"));
    }
    if !unmapped.is_empty() {
        plan.push_str("## Not covered\n\n");
        for u in &unmapped {
            let path = u["path"].as_str().unwrap_or("?");
            let reason = u["reason"].as_str().unwrap_or("unmapped");
            plan.push_str(&format!("- `{path}` — {reason}\n"));
        }
        plan.push('\n');
    }
    plan.push_str("## Next\n\n");
    plan.push_str(&format!(
        "1. `agent-qa replay {sid}` — verify the draft runs\n"
    ));
    plan.push_str(&format!(
        "2. `agent-qa shot-accept {sid}` — mint baselines\n"
    ));
    plan.push_str("3. Extend: add interaction steps (click/fill/check) per route where a\n   golden alone isn't enough\n");
    fs::write(dir.join("plan.md"), plan).with_context(|| "write plan.md")?;

    let skipped = report["unmapped"].as_array().map(|u| u.len()).unwrap_or(0);
    println!(
        "wrote {} — {} routes ({} paths unmapped, see discover-report.json + plan.md)",
        scenario_path.display(),
        routes.len(),
        skipped
    );
    println!("next: replay {sid} && shot-accept {sid}");
    Ok(0)
}

pub(crate) enum RouteHit {
    Route(String),
    /// A route-shaped path with a dynamic segment we can't concretize.
    Dynamic(String),
    None,
}

/// Map a repo-relative path to a URL route by front-end convention.
/// `src/pages/a/b.tsx` → `/a/b`; `app/(group)/users/[id]/page.tsx` →
/// Dynamic("/users/[id]"). Route groups `(x)` are dropped (they don't
/// appear in the URL); `index` leaves drop; non-page extensions and
/// paths outside a route marker return None.
pub(crate) fn path_to_route(path: &str) -> RouteHit {
    let p = path.trim_matches('/');
    let segments: Vec<&str> = p.split('/').collect();
    // First marker wins — `app/` under `src/`, `pages/` at root, etc.
    let Some(mpos) = segments.iter().position(|s| ROUTE_MARKERS.contains(s)) else {
        return RouteHit::None;
    };
    let rest = &segments[mpos + 1..];
    if rest.is_empty() {
        return RouteHit::Route("/".into());
    }
    let (leaf, dirs) = rest.split_last().unwrap();
    let leaf = *leaf;
    let stem = leaf.rsplit_once('.').map(|(s, _)| s).unwrap_or(leaf);
    let ext = leaf.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    // app router leaves name the convention, not the route.
    let leaf_is_router_entry =
        ["page", "route", "layout", "loading", "error", "template"].contains(&stem);
    let leaf_is_page = PAGE_EXTS.contains(&ext);
    let mut segs: Vec<String> = dirs
        .iter()
        .filter(|s| !s.starts_with('(')) // route groups
        .filter(|s| !s.starts_with('@')) // parallel routes
        .map(|s| (*s).to_string())
        .collect();
    if !leaf_is_router_entry {
        if !leaf_is_page {
            return RouteHit::None;
        }
        if stem != "index" {
            segs.push(stem.to_string());
        }
    }
    let route = format!("/{}", segs.join("/"));
    let route = route.trim_end_matches('/').to_string();
    let route = if route.is_empty() { "/".into() } else { route };
    if segs
        .iter()
        .any(|s| s.starts_with('[') || s.starts_with('$') || s.starts_with('_'))
    {
        return RouteHit::Dynamic(route);
    }
    RouteHit::Route(route)
}

fn sanitize(host: &str) -> String {
    host.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(p: &str) -> Option<String> {
        match path_to_route(p) {
            RouteHit::Route(r) => Some(r),
            _ => None,
        }
    }
    fn dynamic(p: &str) -> Option<String> {
        match path_to_route(p) {
            RouteHit::Dynamic(r) => Some(r),
            _ => None,
        }
    }

    #[test]
    fn pages_router_files_map() {
        assert_eq!(route("src/pages/index.tsx"), Some("/".into()));
        assert_eq!(route("pages/about.tsx"), Some("/about".into()));
        assert_eq!(
            route("src/pages/users/list.tsx"),
            Some("/users/list".into())
        );
        assert_eq!(route("pages/a.vue"), Some("/a".into()));
        assert_eq!(route("views/home.svelte"), Some("/home".into()));
    }

    #[test]
    fn app_router_leaves_and_groups_map() {
        assert_eq!(route("src/app/page.tsx"), Some("/".into()));
        assert_eq!(route("src/app/users/page.tsx"), Some("/users".into()));
        assert_eq!(route("app/(shop)/cart/page.tsx"), Some("/cart".into()));
        assert_eq!(route("src/app/blog/layout.tsx"), Some("/blog".into()));
        assert_eq!(route("routes/api/route.ts"), Some("/api".into()));
    }

    #[test]
    fn dynamic_segments_are_reported_not_guessed() {
        assert_eq!(
            dynamic("src/app/users/[id]/page.tsx"),
            Some("/users/[id]".into())
        );
        assert_eq!(dynamic("pages/$slug.tsx"), Some("/$slug".into()));
    }

    #[test]
    fn non_route_paths_do_not_map() {
        assert_eq!(route("src/components/Button.tsx"), None);
        assert_eq!(route("docs/readme.md"), None);
        assert_eq!(route(".github/workflows/ci.yml"), None);
        assert_eq!(route("src/pages/data.json"), None);
        assert_eq!(route("src/pages/styles.css"), None);
    }
}
