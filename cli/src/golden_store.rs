//! `[baselines]` golden store — remote home for `<sid>/baselines/`.
//!
//! Default is `local`: baselines stay in the scenario dir and ride git
//! (today's behaviour — the store is a no-op). A remote store keeps binary
//! goldens out of the source repo's history:
//!
//! ```toml
//! [baselines]
//! store = "github"                       # or "turso"
//! repo = "org/agent-qa-goldens"          # github: target repository
//! branch = "main"                        # github: optional, default main
//! prefix = "baselines"                   # github: remote path prefix
//! token_env = "AGENT_QA_GH_TOKEN"        # env var holding the API token
//!   # github default lookup: AGENT_QA_GH_TOKEN, GITHUB_TOKEN, GH_TOKEN
//! url = "libsql://db-org.turso.io"       # turso: libsql or https URL
//! # turso default token_env: TURSO_AUTH_TOKEN
//! ```
//!
//! Sync model: replay `sync_in`s (pull) the sid's baselines into the local
//! dir before the step loop; `shot-accept`/`domshot-accept`/`replay
//! --update-baselines` `sync_out` (push) minted files after writing them.
//! Both directions track a `<sid>/baselines/.store.json` manifest
//! `{name: {local, remote}}` so syncs only move changed files — the remote
//! ref is opaque per backend (github blob sha, turso content sha256).
//!
//! Remote layout is `prefix/<sid>/<name>` — additive in both directions:
//! a file missing on one side is never deleted on the other.
//!
//! `agent-qa baselines pull|push|status [<sid> | --all] [--json]`.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};

use crate::paths;

const MANIFEST_NAME: &str = ".store.json";
/// Acceptable local file types — everything the mint verbs write.
const BASELINE_SUFFIXES: &[&str] = &[".png", ".snap.txt"];

// ---------- config ----------

/// The resolved store. `Local` covers both an absent `[baselines]` and
/// `store = "local"`.
#[derive(Debug)]
enum Backend {
    Github {
        api: String,
        repo: String,
        branch: String,
        prefix: String,
        token: String,
    },
    Turso {
        url: String,
        token: String,
    },
}

impl Backend {
    /// Short label for messages — no credentials.
    fn describe(&self) -> String {
        match self {
            Backend::Github { repo, branch, .. } => format!("github:{repo}@{branch}"),
            Backend::Turso { url, .. } => format!("turso:{url}"),
        }
    }
}

/// Resolve the configured backend. Ok(None) = local store — all syncs
/// no-op. Errors mean the config names a remote backend but is missing
/// required fields or credentials.
fn resolve() -> Result<Option<Backend>> {
    let Some((_cfg_path, t)) = paths::baselines_config() else {
        return Ok(None);
    };
    let store = t.store.as_deref().unwrap_or("local").trim();
    match store {
        "" | "local" => Ok(None),
        "github" => {
            let repo = t
                .repo
                .filter(|r| !r.trim().is_empty())
                .context("[baselines] store=github needs repo = \"owner/name\"")?;
            let token_env = t.token_env.clone();
            let token = github_token(token_env.as_deref())
                .context("[baselines] store=github needs a token — set AGENT_QA_GH_TOKEN (or GITHUB_TOKEN / GH_TOKEN), or name another var with token_env")?;
            Ok(Some(Backend::Github {
                api: std::env::var("AGENT_QA_GH_API")
                    .unwrap_or_else(|_| "https://api.github.com".into()),
                repo,
                branch: t.branch.unwrap_or_else(|| "main".into()),
                prefix: t.prefix.unwrap_or_else(|| "baselines".into()),
                token,
            }))
        }
        "turso" => {
            let url = t
                .url
                .filter(|u| !u.trim().is_empty())
                .context("[baselines] store=turso needs url = \"libsql://…\" or \"https://…\"")?;
            // libsql URLs speak the same hrana pipeline over https.
            let url = if let Some(rest) = url.strip_prefix("libsql://") {
                format!("https://{rest}")
            } else {
                url
            };
            let env_name = t.token_env.unwrap_or_else(|| "TURSO_AUTH_TOKEN".into());
            let token = std::env::var(&env_name)
                .ok()
                .filter(|v| !v.is_empty())
                .with_context(|| format!("[baselines] store=turso needs a token in ${env_name}"))?;
            Ok(Some(Backend::Turso { url, token }))
        }
        other => bail!("[baselines] store = {other:?} — expected local, github, or turso"),
    }
}

fn github_token(named: Option<&str>) -> Option<String> {
    if let Some(name) = named {
        return std::env::var(name).ok().filter(|v| !v.is_empty());
    }
    for name in ["AGENT_QA_GH_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(v) = std::env::var(name) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

// ---------- manifest ----------

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    #[serde(default)]
    files: BTreeMap<String, ManifestEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestEntry {
    /// sha256 of the local file bytes at last sync.
    local: String,
    /// Backend-opaque remote identity at last sync (github blob sha /
    /// turso stored sha).
    remote: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn baselines_dir(scenario_dir: &Path) -> PathBuf {
    scenario_dir.join("baselines")
}

fn read_manifest(bdir: &Path) -> Manifest {
    fs::read(bdir.join(MANIFEST_NAME))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_manifest(bdir: &Path, m: &Manifest) -> Result<()> {
    fs::create_dir_all(bdir)?;
    fs::write(bdir.join(MANIFEST_NAME), serde_json::to_vec_pretty(m)?)
        .context("write baselines/.store.json")
}

/// Local baseline files present under `bdir`, name → bytes.
fn local_files(bdir: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    if !bdir.is_dir() {
        return Ok(out);
    }
    for ent in fs::read_dir(bdir)?.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if BASELINE_SUFFIXES.iter().any(|s| name.ends_with(s)) {
            out.insert(name.clone(), fs::read(ent.path())?);
        }
    }
    Ok(out)
}

fn local_sha(bdir: &Path, name: &str) -> Option<String> {
    fs::read(bdir.join(name)).ok().map(|b| sha256(&b))
}

// ---------- backend ops ----------

/// Remote listing for a sid: name → opaque remote ref. An empty map means
/// the sid has nothing stored remotely (not an error).
fn remote_list(b: &Backend, sid: &str) -> Result<BTreeMap<String, String>> {
    match b {
        Backend::Github {
            api,
            repo,
            branch,
            prefix,
            token,
        } => {
            let url = format!("{api}/repos/{repo}/contents/{prefix}/{sid}?ref={branch}");
            let resp = gh_get(&url, token, "application/vnd.github+json")?;
            let Some(resp) = resp else {
                return Ok(BTreeMap::new());
            };
            let arr: Vec<Json> = serde_json::from_slice(&resp)
                .context("github contents listing: unexpected shape")?;
            let mut out = BTreeMap::new();
            for item in arr {
                if item.get("type").and_then(|v| v.as_str()) == Some("file") {
                    if let (Some(n), Some(s)) = (
                        item.get("name").and_then(|v| v.as_str()),
                        item.get("sha").and_then(|v| v.as_str()),
                    ) {
                        out.insert(n.to_string(), s.to_string());
                    }
                }
            }
            Ok(out)
        }
        Backend::Turso { url, token } => {
            let rows = turso_query(
                url,
                token,
                "SELECT name, sha FROM baselines WHERE sid = ?1",
                vec![turso_text(sid)],
            )?;
            let mut out = BTreeMap::new();
            for row in rows {
                if let [n, s] = &row[..] {
                    if let (Some(n), Some(s)) = (hrana_str(n), hrana_str(s)) {
                        out.insert(n.to_string(), s.to_string());
                    }
                }
            }
            Ok(out)
        }
    }
}

fn remote_get(b: &Backend, sid: &str, name: &str) -> Result<Vec<u8>> {
    match b {
        Backend::Github {
            api,
            repo,
            branch,
            prefix,
            token,
        } => {
            let url = format!("{api}/repos/{repo}/contents/{prefix}/{sid}/{name}?ref={branch}");
            gh_get(&url, token, "application/vnd.github.raw")?
                .with_context(|| format!("{name}: not in remote store"))
        }
        Backend::Turso { url, token } => {
            let rows = turso_query(
                url,
                token,
                "SELECT data FROM baselines WHERE sid = ?1 AND name = ?2",
                vec![turso_text(sid), turso_text(name)],
            )?;
            rows.first()
                .and_then(|r| r.first())
                .and_then(hrana_blob)
                .with_context(|| format!("{name}: not in remote store"))
        }
    }
}

/// Upload one file; `remote_ref` is the listing ref when the file already
/// exists remotely (github needs it as the `sha` for update PUTs). Returns
/// the new remote ref to record in the manifest.
fn remote_put(
    b: &Backend,
    sid: &str,
    name: &str,
    remote_ref: Option<&str>,
    data: &[u8],
) -> Result<String> {
    match b {
        Backend::Github {
            api,
            repo,
            branch,
            prefix,
            token,
        } => {
            let url = format!("{api}/repos/{repo}/contents/{prefix}/{sid}/{name}");
            let mut body = json!({
                "message": format!("agent-qa baselines: {sid}/{name}"),
                "content": B64.encode(data),
                "branch": branch,
            });
            if let Some(sha) = remote_ref {
                body["sha"] = json!(sha);
            }
            let resp = ureq::put(&url)
                .set("Authorization", &format!("Bearer {token}"))
                .set("Accept", "application/vnd.github+json")
                .set("User-Agent", "agent-qa")
                .send_json(body)
                .map_err(|e| anyhow::anyhow!("github put {name}: {e}"))?;
            let j: Json = resp.into_json().context("github put: bad json")?;
            j.pointer("/content/sha")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .context("github put: response had no content.sha")
        }
        Backend::Turso { url, token } => {
            let sha = sha256(data);
            // Version log: every pushed version appended per file so
            // `baselines revert` can roll back — the newest history row for a
            // (sid, name) mirrors the live `baselines` row.
            turso_exec(
                url,
                token,
                "INSERT INTO baseline_history (sid, name, seq, sha, data) VALUES (?1, ?2, \
                 (SELECT COALESCE(MAX(seq), 0) + 1 FROM baseline_history \
                  WHERE sid = ?1 AND name = ?2), ?3, ?4)",
                vec![
                    turso_text(sid),
                    turso_text(name),
                    turso_text(&sha),
                    turso_blob(data),
                ],
            )?;
            turso_exec(
                url,
                token,
                "INSERT INTO baselines (sid, name, sha, data) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (sid, name) DO UPDATE SET sha = excluded.sha, data = excluded.data",
                vec![
                    turso_text(sid),
                    turso_text(name),
                    turso_text(&sha),
                    turso_blob(data),
                ],
            )?;
            Ok(sha)
        }
    }
}

// ---------- public sync surface ----------

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncReport {
    pub store: String,
    pub sid: String,
    /// Files moved this sync (downloaded on pull, uploaded on push).
    pub moved: Vec<String>,
    /// Files already identical both sides.
    pub unchanged: usize,
}

/// Pull the sid's remote baselines into `<scenario_dir>/baselines/`.
/// Ok(None) = local store — nothing to do.
pub(crate) fn sync_in(scenario_dir: &Path) -> Result<Option<SyncReport>> {
    let Some(b) = resolve()? else { return Ok(None) };
    let sid = scenario_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .context("scenario dir has no name")?;
    let remote = remote_list(&b, &sid)?;
    let bdir = baselines_dir(scenario_dir);
    let mut manifest = read_manifest(&bdir);
    let mut moved = Vec::new();
    let mut unchanged = 0;
    for (name, ref_) in &remote {
        let fresh_local = manifest.files.get(name).is_some_and(|m| {
            m.remote == *ref_ && Some(&m.local) == local_sha(&bdir, name).as_ref()
        });
        if fresh_local {
            unchanged += 1;
            continue;
        }
        let data = remote_get(&b, &sid, name)?;
        fs::create_dir_all(&bdir)?;
        fs::write(bdir.join(name), &data).with_context(|| format!("write baselines/{name}"))?;
        manifest.files.insert(
            name.clone(),
            ManifestEntry {
                local: sha256(&data),
                remote: ref_.clone(),
            },
        );
        moved.push(name.clone());
    }
    write_manifest(&bdir, &manifest)?;
    Ok(Some(SyncReport {
        store: b.describe(),
        sid,
        moved,
        unchanged,
    }))
}

/// Push `<scenario_dir>/baselines/` files to the remote store.
/// Ok(None) = local store — nothing to do.
pub(crate) fn sync_out(scenario_dir: &Path) -> Result<Option<SyncReport>> {
    let Some(b) = resolve()? else { return Ok(None) };
    let sid = scenario_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .context("scenario dir has no name")?;
    let remote = remote_list(&b, &sid)?;
    let bdir = baselines_dir(scenario_dir);
    let local = local_files(&bdir)?;
    let mut manifest = read_manifest(&bdir);
    let mut moved = Vec::new();
    let mut unchanged = 0;
    for (name, data) in &local {
        let lsha = sha256(data);
        let remote_ref = remote.get(name).map(String::as_str);
        let already = manifest
            .files
            .get(name)
            .is_some_and(|m| m.local == lsha && remote_ref.is_some_and(|r| m.remote == r));
        if already {
            unchanged += 1;
            continue;
        }
        let new_ref = remote_put(&b, &sid, name, remote_ref, data)?;
        manifest.files.insert(
            name.clone(),
            ManifestEntry {
                local: lsha,
                remote: new_ref,
            },
        );
        moved.push(name.clone());
    }
    write_manifest(&bdir, &manifest)?;
    Ok(Some(SyncReport {
        store: b.describe(),
        sid,
        moved,
        unchanged,
    }))
}

// ---------- github plumbing ----------

/// GET with github headers. Ok(None) on 404 (sid absent remotely).
fn gh_get(url: &str, token: &str, accept: &str) -> Result<Option<Vec<u8>>> {
    match ureq::get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", accept)
        .set("User-Agent", "agent-qa")
        .call()
    {
        Ok(resp) => {
            let mut buf = Vec::new();
            resp.into_reader()
                .take(64 * 1024 * 1024)
                .read_to_end(&mut buf)
                .context("github: read body")?;
            Ok(Some(buf))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(e) => bail!("github {url}: {e}"),
    }
}

// ---------- turso plumbing ----------

fn turso_text(v: &str) -> Json {
    json!({"type": "text", "value": v})
}

fn turso_blob(data: &[u8]) -> Json {
    json!({"type": "blob", "base64": B64.encode(data)})
}

fn hrana_str(v: &Json) -> Option<&str> {
    v.get("value").and_then(|v| v.as_str())
}

fn hrana_blob(v: &Json) -> Option<Vec<u8>> {
    let b64 = v.get("base64").and_then(|v| v.as_str())?;
    // sqld/libsql-server omits '=' padding; the STANDARD engine requires it.
    let mut padded = b64.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    B64.decode(padded).ok()
}

fn turso_pipeline(url: &str, token: &str, sql: &str, args: Vec<Json>) -> Result<Vec<Vec<Json>>> {
    let resp = ureq::post(&format!("{url}/v2/pipeline"))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_json(json!({
            "requests": [
                {"type": "execute", "stmt": {"sql": sql, "args": args}},
                {"type": "close"}
            ]
        }))
        .map_err(|e| anyhow::anyhow!("turso {url}: {e}"))?;
    let j: Json = resp.into_json().context("turso: bad json")?;
    // results[0].response.result.rows — each row is an array of hrana values.
    if let Some(err) = j.pointer("/results/0/error") {
        bail!(
            "turso: {}",
            err.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
    }
    let rows = j
        .pointer("/results/0/response/result/rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(rows.iter().filter_map(|r| r.as_array().cloned()).collect())
}

fn turso_exec(url: &str, token: &str, sql: &str, args: Vec<Json>) -> Result<()> {
    turso_pipeline(url, token, sql, args).map(|_| ())
}

fn turso_query(url: &str, token: &str, sql: &str, args: Vec<Json>) -> Result<Vec<Vec<Json>>> {
    turso_exec(
        url,
        token,
        "CREATE TABLE IF NOT EXISTS baselines (\
            sid TEXT NOT NULL, name TEXT NOT NULL, sha TEXT NOT NULL, \
            data BLOB NOT NULL, PRIMARY KEY (sid, name))",
        vec![],
    )?;
    turso_exec(
        url,
        token,
        "CREATE TABLE IF NOT EXISTS baseline_history (\
            sid TEXT NOT NULL, name TEXT NOT NULL, seq INTEGER NOT NULL, \
            sha TEXT NOT NULL, data BLOB NOT NULL, \
            PRIMARY KEY (sid, name, seq))",
        vec![],
    )?;
    turso_pipeline(url, token, sql, args)
}

/// Roll every file of `sid` back one pushed version: drop the newest history
/// row per name, then set the live `baselines` row to whatever history row is
/// left (or delete it if the file's first push is being undone).
fn turso_revert(url: &str, token: &str, sid: &str) -> Result<usize> {
    let rows = turso_query(
        url,
        token,
        "SELECT name, seq FROM baseline_history WHERE sid = ?1 \
         ORDER BY name, seq DESC",
        vec![turso_text(sid)],
    )?;
    // Newest seq per name → the entry to roll back.
    let mut reverted: Vec<(String, i64)> = Vec::new();
    for row in rows {
        let name = hrana_str(&row[0]).unwrap_or_default().to_string();
        let seq = hrana_str(&row[1])
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        if reverted.iter().any(|(n, _)| n == &name) {
            continue;
        }
        reverted.push((name, seq));
    }
    for (name, seq) in &reverted {
        turso_exec(
            url,
            token,
            "DELETE FROM baseline_history WHERE sid = ?1 AND name = ?2 AND seq = ?3",
            vec![
                turso_text(sid),
                turso_text(name),
                json!({"type": "integer", "value": seq.to_string()}),
            ],
        )?;
        // Whatever is now the newest history row becomes live; none → file
        // didn't exist before the reverted push, so it stays deleted.
        turso_exec(
            url,
            token,
            "DELETE FROM baselines WHERE sid = ?1 AND name = ?2",
            vec![turso_text(sid), turso_text(name)],
        )?;
        turso_exec(
            url,
            token,
            "INSERT INTO baselines (sid, name, sha, data) \
             SELECT sid, name, sha, data FROM baseline_history \
             WHERE sid = ?1 AND name = ?2 \
             ORDER BY seq DESC LIMIT 1",
            vec![turso_text(sid), turso_text(name)],
        )?;
    }
    Ok(reverted.len())
}

// ---------- `baselines` verb ----------

pub fn cli(args: &[String]) -> Result<u8> {
    let mut sid: Option<String> = None;
    let mut all = false;
    let mut json_out = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--all" => all = true,
            "--json" => json_out = true,
            "--help" | "-h" => {
                print_help();
                return Ok(0);
            }
            v if sid.is_none() && !v.starts_with('-') => sid = Some(v.to_string()),
            v => bail!("baselines: unknown arg {v:?}"),
        }
        i += 1;
    }
    match args.first().map(String::as_str) {
        Some("status") => status(json_out),
        Some("pull") => sync_many(sid, all, Direction::Pull, json_out),
        Some("push") => sync_many(sid, all, Direction::Push, json_out),
        Some("revert") => revert(sid, all, json_out),
        Some("-h" | "--help" | "help") | None => {
            print_help();
            Ok(0)
        }
        Some(other) => bail!("baselines: unknown subverb {other:?} — pull, push, revert, status"),
    }
}

enum Direction {
    Pull,
    Push,
}

fn print_help() {
    println!(
        "agent-qa baselines — sync `<sid>/baselines/` with a remote golden store\n\n\
         Usage:\n\
         \x20 agent-qa baselines status [--json]        Show configured store + per-sid counts\n\
         \x20 agent-qa baselines pull [<sid> | --all]   Download remote baselines into scenario dirs\n\
         \x20 agent-qa baselines push [<sid> | --all]   Upload local baselines to the store\n\
         \x20 agent-qa baselines revert [<sid> | --all] Roll remote goldens back one pushed version\n\n\
         Configure in agent-qa.toml — [baselines] store = \"local\" | \"github\" | \"turso\".\n\
         Replay pulls before the step loop (--no-baseline-sync opts out); shot-accept,\n\
         domshot-accept and replay --update-baselines push after minting.\n\
         Revert needs the turso store (every pushed version is logged); for the\n\
         github/local stores roll the goldens repo back with git instead."
    );
}

/// Sids under the scenarios root (dir names only).
fn all_sids() -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(paths::scenarios_root())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    (e.file_type().is_ok_and(|t| t.is_dir())
                        && e.path().join("scenario.json").is_file())
                    .then_some(n)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn sync_many(sid: Option<String>, all: bool, dir: Direction, json_out: bool) -> Result<u8> {
    let sids = match (sid, all) {
        (Some(s), _) => vec![s],
        (None, true) => all_sids(),
        (None, false) => bail!("baselines: pass a sid or --all"),
    };
    let mut reports = Vec::new();
    for sid in &sids {
        let sdir = paths::scenario_dir(sid)?;
        let r = match dir {
            Direction::Pull => sync_in(&sdir),
            Direction::Push => sync_out(&sdir),
        }
        .with_context(|| format!("baselines {sid}"))?;
        if let Some(r) = r {
            reports.push(r);
        }
    }
    if json_out {
        println!("{}", serde_json::to_string_pretty(&reports)?);
        return Ok(0);
    }
    if reports.is_empty() {
        println!("baselines: local store — nothing to sync");
    }
    for r in &reports {
        let verb = match dir {
            Direction::Pull => "pulled",
            Direction::Push => "pushed",
        };
        println!(
            "baselines {}: {verb} {} file(s), {} unchanged ({})",
            r.sid,
            r.moved.len(),
            r.unchanged,
            r.store
        );
    }
    Ok(0)
}

/// Roll remote goldens back one pushed version per file, then pull so the
/// local `<sid>/baselines/` and manifest reflect the restored state.
fn revert(sid: Option<String>, all: bool, json_out: bool) -> Result<u8> {
    let Some(b) = resolve()? else {
        bail!("baselines revert: local store — roll the goldens dir back with git instead");
    };
    let Backend::Turso { ref url, ref token } = b else {
        bail!(
            "baselines revert: '{}' store — roll the goldens repo back with git instead",
            b.describe()
        );
    };
    let sids = match (sid, all) {
        (Some(s), _) => vec![s],
        (None, true) => all_sids(),
        (None, false) => bail!("baselines revert: pass a sid or --all"),
    };
    let mut rows = Vec::new();
    for sid in &sids {
        let n = turso_revert(url, token, sid).with_context(|| format!("baselines revert {sid}"))?;
        let sdir = paths::scenario_dir(sid)?;
        let _ = sync_in(&sdir).with_context(|| format!("baselines pull {sid}"))?;
        rows.push(json!({"sid": sid, "revertedFiles": n}));
        if !json_out {
            println!("baselines {sid}: reverted {n} file(s) — pulled restored state");
        }
    }
    if json_out {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    }
    Ok(0)
}

fn status(json_out: bool) -> Result<u8> {
    match resolve()? {
        None => {
            if json_out {
                println!("{}", serde_json::json!({"store": "local"}));
            } else {
                println!("baselines: local store — goldens live in <sid>/baselines/ (committed with the repo)");
            }
        }
        Some(b) => {
            let store = b.describe();
            let mut rows = Vec::new();
            for sid in all_sids() {
                let sdir = paths::scenario_dir(&sid)?;
                let remote = remote_list(&b, &sid)?;
                let local = local_files(&baselines_dir(&sdir))?;
                rows.push(json!({
                    "sid": sid,
                    "remoteFiles": remote.len(),
                    "localFiles": local.len(),
                }));
            }
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({"store": store, "scenarios": rows}))?
                );
            } else {
                println!("baselines: {store}");
                for r in &rows {
                    println!(
                        "  {} — {} remote / {} local",
                        r["sid"].as_str().unwrap_or("?"),
                        r["remoteFiles"],
                        r["localFiles"]
                    );
                }
            }
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// Minimal HTTP stub: records requests, serves canned responses in order.
    /// Covers github contents + turso pipeline shapes without real network.
    struct Stub {
        url: String,
        #[allow(dead_code)]
        requests: std::sync::Arc<std::sync::Mutex<Vec<(String, String, String)>>>,
    }

    impl Stub {
        fn start() -> (Self, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let bodies = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
            let reqs = requests.clone();
            let bods = bodies.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut s) = stream else { continue };
                    let mut reader = BufReader::new(s.try_clone().unwrap());
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line.is_empty() {
                        continue;
                    }
                    let mut parts = line.split_whitespace();
                    let method = parts.next().unwrap_or("").to_string();
                    let path = parts.next().unwrap_or("").to_string();
                    // Headers until blank line; capture content-length.
                    let mut len = 0usize;
                    loop {
                        let mut h = String::new();
                        if reader.read_line(&mut h).is_err() {
                            break;
                        }
                        let h = h.trim();
                        if h.is_empty() {
                            break;
                        }
                        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                            len = v.trim().parse().unwrap_or(0);
                        }
                    }
                    let mut body = vec![0u8; len];
                    let _ = reader.read_exact(&mut body);
                    reqs.lock().unwrap().push((
                        method.clone(),
                        path.clone(),
                        String::from_utf8_lossy(&body).into_owned(),
                    ));
                    let canned = { bods.lock().unwrap().remove(0) };
                    let (status, ctype, payload) = if canned == "__404__" {
                        ("404 Not Found", "application/json", "{}".to_string())
                    } else if let Some(raw) = canned.strip_prefix("__RAW__") {
                        ("200 OK", "application/octet-stream", raw.to_string())
                    } else {
                        ("200 OK", "application/json", canned)
                    };
                    let resp = format!(
                        "HTTP/1.1 {status}\r\ncontent-type: {ctype}\r\ncontent-length: {}\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = s.write_all(resp.as_bytes());
                }
            });
            (
                Stub {
                    url: format!("http://127.0.0.1:{port}"),
                    requests,
                },
                bodies,
            )
        }
    }

    /// A turso pipeline reply wrapping `rows` of hrana values.
    fn turso_reply(rows: Vec<Vec<Json>>) -> String {
        serde_json::json!({
            "results": [{
                "type": "ok",
                "response": {
                    "type": "execute",
                    "result": {"rows": rows}
                }
            }, {"type": "ok"}]
        })
        .to_string()
    }

    fn write_cfg(dir: &Path, body: &str) {
        fs::write(dir.join("agent-qa.toml"), body).unwrap();
    }

    fn sid_dir(root: &Path, sid: &str) -> PathBuf {
        root.join("tmp/agent-qa-scenarios").join(sid)
    }

    #[test]
    fn local_store_is_a_noop() {
        let _g = crate::test_util::lock_env();
        let root = tempfile::tempdir().unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        let r = sync_in(&root.path().join("x")).unwrap();
        std::env::set_current_dir(cwd).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn github_push_then_pull_roundtrip() {
        let _g = crate::test_util::lock_env();
        let root = tempfile::tempdir().unwrap();
        let sdir = sid_dir(root.path(), "s-gold");
        let bdir = sdir.join("baselines");
        fs::create_dir_all(&bdir).unwrap();
        fs::write(bdir.join("s1.png"), b"PNG-A").unwrap();
        fs::write(sdir.join("scenario.json"), "{}").unwrap();

        let (stub, bodies) = Stub::start();
        // push: listing 404 (nothing remote) → then PUT per file
        bodies.lock().unwrap().push("__404__".into());
        bodies
            .lock()
            .unwrap()
            .push(r#"{"content":{"sha":"r1"}}"#.into());
        write_cfg(
            root.path(),
            "[baselines]\nstore = \"github\"\nrepo = \"o/goldens\"\ntoken_env = \"T_X\"\n",
        );
        // github api base override
        std::env::set_var("AGENT_QA_GH_API", &stub.url);
        std::env::set_var("T_X", "tok");
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        let push = sync_out(&sdir).unwrap().unwrap();
        assert_eq!(push.moved, vec!["s1.png"]);
        // second push: listing returns remote sha → manifest matches → skipped
        bodies
            .lock()
            .unwrap()
            .push(serde_json::json!([{"type":"file","name":"s1.png","sha":"r1"}]).to_string());
        let again = sync_out(&sdir).unwrap().unwrap();
        assert!(again.moved.is_empty());
        assert_eq!(again.unchanged, 1);
        // pull with remote sha changed → downloads
        bodies
            .lock()
            .unwrap()
            .push(serde_json::json!([{"type":"file","name":"s1.png","sha":"r2"}]).to_string());
        bodies.lock().unwrap().push("__RAW__PNG-B".into());
        let pull = sync_in(&sdir).unwrap().unwrap();
        assert_eq!(pull.moved, vec!["s1.png"]);
        assert_eq!(fs::read(bdir.join("s1.png")).unwrap(), b"PNG-B");
        std::env::set_current_dir(cwd).unwrap();
        std::env::remove_var("AGENT_QA_GH_API");
        std::env::remove_var("T_X");
    }

    #[test]
    fn turso_pull_downloads_changed() {
        let _g = crate::test_util::lock_env();
        let root = tempfile::tempdir().unwrap();
        let sdir = sid_dir(root.path(), "s-tur");
        let bdir = sdir.join("baselines");
        fs::create_dir_all(&bdir).unwrap();
        fs::write(sdir.join("scenario.json"), "{}").unwrap();

        let (stub, bodies) = Stub::start();
        let sha = sha256(b"blob-data!");
        // pull: 2 schema execs → list rows, then 2 schema execs → data row
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![vec![
            json!({"type":"text","value":"s1.png"}),
            json!({"type":"text","value":sha}),
        ]]));
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![vec![
            // sqld serves blob base64 without padding — exercise that path.
            json!({"type":"blob","base64": B64.encode(b"blob-data!").trim_end_matches('=')}),
        ]]));
        write_cfg(
            root.path(),
            &format!("[baselines]\nstore = \"turso\"\nurl = \"{}\"\n", stub.url),
        );
        std::env::set_var("TURSO_AUTH_TOKEN", "tok");
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root.path()).unwrap();
        let pull = sync_in(&sdir).unwrap().unwrap();
        std::env::set_current_dir(cwd).unwrap();
        std::env::remove_var("TURSO_AUTH_TOKEN");
        assert_eq!(pull.moved, vec!["s1.png"]);
        assert_eq!(fs::read(bdir.join("s1.png")).unwrap(), b"blob-data!");
    }

    #[test]
    fn turso_revert_drops_newest_version() {
        let _g = crate::test_util::lock_env();
        let (stub, bodies) = Stub::start();
        // SELECT name,seq: 2 schema execs + the query (seq DESC → 2 is newest)
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![
            vec![
                json!({"type":"text","value":"s1.png"}),
                json!({"type":"integer","value":"2"}),
            ],
            vec![
                json!({"type":"text","value":"s1.png"}),
                json!({"type":"integer","value":"1"}),
            ],
        ]));
        // per file: delete history seq2, delete baselines, insert-select
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![]));
        bodies.lock().unwrap().push(turso_reply(vec![]));

        let n = turso_revert(&stub.url, "tok", "s-tur").unwrap();
        assert_eq!(n, 1);
        let reqs = stub.requests.lock().unwrap();
        let sql: Vec<&str> = reqs.iter().map(|(_, _, b)| b.as_str()).collect();
        assert!(sql
            .iter()
            .any(|b| b.contains("DELETE FROM baseline_history") && b.contains("\"2\"")));
        assert!(sql.iter().any(|b| b.contains("DELETE FROM baselines")));
        assert!(sql
            .iter()
            .any(|b| b.contains("INSERT INTO baselines") && b.contains("FROM baseline_history")));
    }
}
