//! `ps` and `cleanup` — visibility and reaping for agent-browser residue.
//!
//! Every `agent-browser` session leaves a footprint in its socket dir
//! (`$AGENT_BROWSER_SOCKET_DIR`, default `~/.agent-browser`):
//!
//!   live daemon   `<name>.sock` + `<name>.pid` (+ .engine/.version/.stream)
//!   residue       `<name>.config` / `<name>.target` — always left behind,
//!                 even by a clean `close`. Never reaped upstream, so the
//!                 dir grows one pair per run, forever.
//!   orphan        `.pid` exists but the process is gone — the daemon was
//!                 killed (SIGKILL, OOM, laptop reboot) without a close.
//!                 Its Chrome children survive, reparented to init, still
//!                 holding a `/tmp/agent-browser-chrome-<uuid>` profile.
//!
//! `agent-qa ps` correlates the registry, the process table, and the
//! temp profile dirs into one table; `agent-qa cleanup` reaps the
//! residue/orphans (and, with `--all`, closes live sessions too).
//!
//! Process-table reads are a single `ps`/`/proc` pass, not a spawn per
//! session — `ps` stays cheap even with a hundred stale entries.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::json;

// ---------- data model ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Daemon pid alive and is an agent-browser process.
    Live,
    /// Pid file exists but the process is dead or not agent-browser —
    /// the daemon died without closing; chrome children may be reparented.
    Orphan,
    /// Only residue (.config/.target); closed cleanly or crashed pre-daemon.
    Stale,
}

impl SessionState {
    fn as_str(self) -> &'static str {
        match self {
            SessionState::Live => "live",
            SessionState::Orphan => "orphan",
            SessionState::Stale => "stale",
        }
    }
}

#[derive(Debug)]
pub struct SessionInfo {
    pub name: String,
    pub state: SessionState,
    pub pid: Option<u32>,
    /// Age of the oldest registry file for this session.
    pub age_secs: u64,
    /// RSS (kB) of the daemon + its process tree, when live.
    pub rss_kb: Option<u64>,
    /// Last target URL from `.target`, if any.
    pub url: Option<String>,
    /// Every registry file belonging to this session.
    pub files: Vec<PathBuf>,
}

#[derive(Debug)]
struct ProcRow {
    pid: u32,
    ppid: u32,
    etimes: u64,
    rss_kb: u64,
    args: String,
}

impl ProcRow {
    fn is_agent_browser(&self) -> bool {
        self.args.contains("agent-browser")
    }

    /// Session daemon: the bare binary with no subcommand (daemonized
    /// `agent-browser-linux-x64`). CLI invocations always carry a verb
    /// (close/eval/…), so a bare-args proc is a daemon.
    fn is_daemon(&self) -> bool {
        self.is_agent_browser() && self.args.split_whitespace().count() == 1
    }

    fn chrome_profile_dir(&self) -> Option<String> {
        let marker = "--user-data-dir=";
        let idx = self.args.find(marker)?;
        let rest = &self.args[idx + marker.len()..];
        let dir = rest.split_whitespace().next()?;
        dir.contains("agent-browser-chrome-")
            .then(|| dir.trim_matches('"').to_string())
    }
}

#[derive(Debug)]
pub struct Inventory {
    pub socket_dir: PathBuf,
    pub sessions: Vec<SessionInfo>,
    /// Chrome processes not owned by any live daemon.
    pub orphan_chromes: Vec<OrphanChrome>,
    /// Bare agent-browser daemon procs no live session's pid points at —
    /// their registry files are gone (or were never written).
    pub orphan_daemons: Vec<OrphanDaemon>,
    /// `/tmp/agent-browser-chrome-*` dirs no chrome process uses.
    pub stray_dirs: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct OrphanDaemon {
    pub pid: u32,
    /// Seconds since process start.
    pub age_secs: u64,
    pub rss_kb: u64,
}

#[derive(Debug)]
pub struct OrphanChrome {
    pub pid: u32,
    pub rss_kb: u64,
    pub profile_dir: String,
}

// ---------- inventory ----------

/// The directory agent-browser keeps session sockets/registry in.
/// `$AGENT_BROWSER_SOCKET_DIR` wins; otherwise `~/.agent-browser`
/// (agent-browser's documented default state dir).
pub fn socket_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("AGENT_BROWSER_SOCKET_DIR") {
        let p = PathBuf::from(d);
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let p = PathBuf::from(home).join(".agent-browser");
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

/// Snapshot the whole process table in one pass. `/proc` first (Linux),
/// `ps` as the portable fallback; `None` where neither exists.
fn proc_table() -> Option<Vec<ProcRow>> {
    proc_table_proc().or_else(proc_table_ps)
}

#[cfg(target_os = "linux")]
fn proc_table_proc() -> Option<Vec<ProcRow>> {
    let clk_tck = 100_u64; // USER_HZ on every supported Linux arch we ship.
    let boot_secs = fs::read_to_string("/proc/stat").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("btime "))
            .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
    })?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let mut rows = Vec::new();
    for ent in fs::read_dir("/proc").ok()?.flatten() {
        let name = ent.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let dir = ent.path();
        let Ok(stat) = fs::read_to_string(dir.join("stat")) else {
            continue;
        };
        // comm may contain spaces/parens — fields after the last ')'.
        let Some(rparen) = stat.rfind(')') else {
            continue;
        };
        let f: Vec<&str> = stat[rparen + 1..].split_whitespace().collect();
        // f[0]=state, f[1]=ppid, f[19]=starttime(ticks), f[21]=rss(pages)
        let (Some(ppid), Some(start), Some(rss)) = (
            f.get(1).and_then(|s| s.parse::<u32>().ok()),
            f.get(19).and_then(|s| s.parse::<u64>().ok()),
            f.get(21).and_then(|s| s.parse::<i64>().ok()),
        ) else {
            continue;
        };
        let start_secs = start / clk_tck;
        let etimes = now.saturating_sub(boot_secs + start_secs);
        let args = fs::read_to_string(dir.join("cmdline"))
            .map(|s| s.replace('\0', " "))
            .unwrap_or_default();
        rows.push(ProcRow {
            pid,
            ppid,
            etimes,
            rss_kb: (rss.max(0) as u64) * 4,
            args,
        });
    }
    Some(rows)
}

#[cfg(not(target_os = "linux"))]
fn proc_table_proc() -> Option<Vec<ProcRow>> {
    None
}

fn proc_table_ps() -> Option<Vec<ProcRow>> {
    let out = std::process::Command::new("ps")
        .args(["-eo", "pid=,ppid=,etimes=,rss=,args="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let row = (
            it.next().and_then(|s| s.parse::<u32>().ok()),
            it.next().and_then(|s| s.parse::<u32>().ok()),
            it.next().and_then(|s| s.parse::<u64>().ok()),
            it.next().and_then(|s| s.parse::<u64>().ok()),
        );
        if let (Some(pid), Some(ppid), Some(etimes), Some(rss_kb)) = row {
            rows.push(ProcRow {
                pid,
                ppid,
                etimes,
                rss_kb,
                args: it.collect::<Vec<_>>().join(" "),
            });
        }
    }
    Some(rows)
}

/// Group socket-dir files into sessions by `<name>.<ext>` prefix.
fn scan_socket_dir(dir: &Path) -> Vec<(String, Vec<PathBuf>, u64)> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut groups: BTreeMap<String, (Vec<PathBuf>, u64)> = BTreeMap::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    for ent in rd.flatten() {
        let path = ent.path();
        // Skip directories; sockets and regular files both count.
        if ent.file_type().map(|t| t.is_dir()).unwrap_or(true) {
            continue;
        }
        let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some((name, _ext)) = fname.rsplit_once('.') else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let mtime = ent
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(now);
        let g = groups
            .entry(name.to_string())
            .or_insert_with(|| (Vec::new(), u64::MAX));
        g.0.push(path);
        g.1 = g.1.min(mtime);
    }
    groups
        .into_iter()
        .map(|(name, (files, oldest))| (name, files, now.saturating_sub(oldest)))
        .collect()
}

fn read_pid_file(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_target_url(path: &Path) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    v.get("url")?.as_str().map(str::to_string)
}

/// RSS of a process plus all transitive children (by ppid walking).
fn tree_rss(root: u32, rows: &[ProcRow]) -> u64 {
    let mut total = 0;
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        for r in rows {
            if r.pid == pid {
                total += r.rss_kb;
            } else if r.ppid == pid {
                stack.push(r.pid);
            }
        }
    }
    total
}

/// Is `pid` an ancestor of the chrome process (daemon parenthood)?
/// Chrome's direct ppid is the daemon; after daemon death chrome is
/// reparented to init — ppid == 1 means nobody owns it.
fn owned_by(chrome: &ProcRow, live_pids: &[u32], by_pid: &BTreeMap<u32, &ProcRow>) -> bool {
    // Walk the ppid chain: chrome's parent is the daemon (main chrome)
    // or another chrome process (zygotes, renderers, utilities). A dead
    // daemon leaves the chain ending at init — ppid <= 1 means unowned.
    let mut ppid = chrome.ppid;
    for _ in 0..32 {
        if ppid <= 1 {
            return false;
        }
        if live_pids.contains(&ppid) {
            return true;
        }
        match by_pid.get(&ppid) {
            Some(r) => ppid = r.ppid,
            None => return false,
        }
    }
    false
}

pub fn collect() -> Option<Inventory> {
    let dir = socket_dir()?;
    let procs = proc_table().unwrap_or_default();
    let by_pid: BTreeMap<u32, &ProcRow> = procs.iter().map(|r| (r.pid, r)).collect();
    let mut sessions = Vec::new();
    for (name, files, age_secs) in scan_socket_dir(&dir) {
        let has_sock = files
            .iter()
            .any(|p| p.extension().and_then(|e| e.to_str()) == Some("sock"));
        let pid = files
            .iter()
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("pid"))
            .and_then(|p| read_pid_file(p));
        let url = files
            .iter()
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("target"))
            .and_then(|p| read_target_url(p));
        let (state, rss_kb) = match pid {
            Some(pid) if by_pid.get(&pid).is_some_and(|r| r.is_agent_browser()) => {
                (SessionState::Live, Some(tree_rss(pid, &procs)))
            }
            // No process table (Windows): trust the socket file.
            Some(_) if procs.is_empty() && has_sock => (SessionState::Live, None),
            Some(_) => (SessionState::Orphan, None),
            None if has_sock => (SessionState::Live, None),
            None => (SessionState::Stale, None),
        };
        // A live session's age is the daemon's own uptime, not the
        // oldest registry file's mtime.
        let age_secs = pid
            .and_then(|p| by_pid.get(&p))
            .filter(|_| state == SessionState::Live)
            .map(|r| r.etimes)
            .unwrap_or(age_secs);
        sessions.push(SessionInfo {
            name,
            state,
            pid,
            age_secs,
            rss_kb,
            url,
            files,
        });
    }
    sessions.sort_by(|a, b| b.age_secs.cmp(&a.age_secs).then(a.name.cmp(&b.name)));

    let live_pids: Vec<u32> = sessions
        .iter()
        .filter_map(|s| (s.state == SessionState::Live).then_some(s.pid).flatten())
        .collect();
    let mut orphan_chromes = Vec::new();
    // Daemon procs not registered by any live session — their registry
    // files vanished under them (killed replay, manual rm), so nothing
    // else in the inventory sees them.
    let mut orphan_daemons: Vec<OrphanDaemon> = Vec::new();
    for r in &procs {
        if r.is_daemon() && !live_pids.contains(&r.pid) {
            orphan_daemons.push(OrphanDaemon {
                pid: r.pid,
                age_secs: r.etimes,
                rss_kb: r.rss_kb,
            });
        }
    }
    // Dirs referenced by ANY chrome process (owned or orphan) — a stray dir
    // is one no running chrome uses at all.
    let mut chrome_dirs = Vec::new();
    for r in &procs {
        let Some(dir) = r.chrome_profile_dir() else {
            continue;
        };
        chrome_dirs.push(dir.clone());
        if !owned_by(r, &live_pids, &by_pid) {
            orphan_chromes.push(OrphanChrome {
                pid: r.pid,
                rss_kb: r.rss_kb,
                profile_dir: dir,
            });
        }
    }
    let mut stray_dirs = Vec::new();
    if let Ok(rd) = fs::read_dir(std::env::temp_dir()) {
        for ent in rd.flatten() {
            let path = ent.path();
            let is_profile = path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with("agent-browser-chrome-"));
            if is_profile && !chrome_dirs.iter().any(|d| Path::new(d) == path) {
                stray_dirs.push(path);
            }
        }
    }
    Some(Inventory {
        socket_dir: dir,
        sessions,
        orphan_chromes,
        orphan_daemons,
        stray_dirs,
    })
}

// ---------- ps verb ----------

fn human_age(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86400),
    }
}

fn human_kb(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1} GB", kb as f64 / 1_048_576.0)
    } else if kb >= 1024 {
        format!("{} MB", kb / 1024)
    } else {
        format!("{kb} kB")
    }
}

pub fn ps(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    for a in args {
        match a.as_str() {
            "--json" => json_out = true,
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa ps \u{2014} live agent-browser sessions + zombie residue\n\n\
                     Usage:\n  agent-qa ps          Table of live / orphaned / stale sessions\n  \
                     agent-qa ps --json   Structured inventory on stdout\n\n\
                     Session states:\n  live    daemon running (pid + socket alive)\n  \
                     orphan  pid file but process dead — killed daemon, chrome may survive\n  \
                     stale   residue files only (.config/.target), nothing running\n\n\
                     Reap with `agent-qa cleanup` (add --all to close live sessions too)."
                );
                return Ok(0);
            }
            other => bail!("agent-qa ps: unknown flag {other:?}"),
        }
    }
    let Some(inv) = collect() else {
        if json_out {
            println!(
                "{{\"socketDir\":null,\"sessions\":[],\"orphanChromes\":[],\"orphanDaemons\":[],\"strayDirs\":[]}}"
            );
        } else {
            println!("No agent-browser state dir found — nothing running, nothing to track.");
        }
        return Ok(0);
    };
    if json_out {
        let body = json!({
            "socketDir": inv.socket_dir,
            "sessions": inv.sessions.iter().map(|s| json!({
                "name": s.name,
                "state": s.state.as_str(),
                "pid": s.pid,
                "ageSecs": s.age_secs,
                "rssKb": s.rss_kb,
                "url": s.url,
                "files": s.files,
            })).collect::<Vec<_>>(),
            "orphanChromes": inv.orphan_chromes.iter().map(|c| json!({
                "pid": c.pid, "rssKb": c.rss_kb, "profileDir": c.profile_dir,
            })).collect::<Vec<_>>(),
            "orphanDaemons": inv.orphan_daemons.iter().map(|d| json!({
                "pid": d.pid, "ageSecs": d.age_secs, "rssKb": d.rss_kb,
            })).collect::<Vec<_>>(),
            "strayDirs": inv.stray_dirs,
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
        return Ok(0);
    }

    println!(
        "agent-browser sessions  (state dir: {})",
        inv.socket_dir.display()
    );
    if inv.sessions.is_empty()
        && inv.orphan_chromes.is_empty()
        && inv.orphan_daemons.is_empty()
        && inv.stray_dirs.is_empty()
    {
        println!("  all clean — no sessions, no residue.");
        return Ok(0);
    }
    println!();
    if !inv.sessions.is_empty() {
        println!(
            "  {:<34} {:<7} {:<7} {:<6} {:<9} URL",
            "SESSION", "STATE", "PID", "AGE", "RSS"
        );
        for s in &inv.sessions {
            println!(
                "  {:<34} {:<7} {:<7} {:<6} {:<9} {}",
                truncate(&s.name, 34),
                s.state.as_str(),
                s.pid.map(|p| p.to_string()).unwrap_or_else(|| "—".into()),
                human_age(s.age_secs),
                s.rss_kb.map(human_kb).unwrap_or_else(|| "—".into()),
                s.url.as_deref().unwrap_or("—"),
            );
        }
    }
    if !inv.orphan_chromes.is_empty() {
        let total: u64 = inv.orphan_chromes.iter().map(|c| c.rss_kb).sum();
        println!();
        println!(
            "  unowned Chrome processes: {} ({} total)",
            inv.orphan_chromes.len(),
            human_kb(total)
        );
        for c in &inv.orphan_chromes {
            println!(
                "    pid {}  {}  {}",
                c.pid,
                human_kb(c.rss_kb),
                c.profile_dir
            );
        }
    }
    if !inv.orphan_daemons.is_empty() {
        let total: u64 = inv.orphan_daemons.iter().map(|d| d.rss_kb).sum();
        println!();
        println!(
            "  unowned agent-browser daemons: {} ({} total)",
            inv.orphan_daemons.len(),
            human_kb(total)
        );
        for d in &inv.orphan_daemons {
            println!(
                "    pid {}  {}  {}",
                d.pid,
                human_age(d.age_secs),
                human_kb(d.rss_kb)
            );
        }
    }
    if !inv.stray_dirs.is_empty() {
        println!();
        println!("  stray profile dirs (no live chrome):");
        for d in &inv.stray_dirs {
            println!("    {}", d.display());
        }
    }
    println!();
    println!("  cleanup: `agent-qa cleanup` reaps stale+orphan residue; `cleanup --all` also closes live sessions.");
    Ok(0)
}

fn truncate(s: &str, w: usize) -> String {
    if s.len() <= w {
        s.to_string()
    } else {
        format!("{}…", &s[..w - 1])
    }
}

// ---------- cleanup verb ----------

fn parse_duration(s: &str) -> Option<u64> {
    let (num, mult) = match s.chars().last() {
        Some('s') => (&s[..s.len() - 1], 1),
        Some('m') => (&s[..s.len() - 1], 60),
        Some('h') => (&s[..s.len() - 1], 3600),
        Some('d') => (&s[..s.len() - 1], 86400),
        _ => (s, 1),
    };
    Some(num.parse::<u64>().ok()? * mult)
}

#[cfg(unix)]
fn term_then_kill(pid: u32) {
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_millis(800));
    unsafe {
        libc::kill(pid as i32, 0); // probe
                                   // SIGKILL is harmless if already dead.
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn term_then_kill(_pid: u32) {
    // Windows cleanup is conservative: orphan chromes are reported but not
    // killed (no libc kill); the user closes them from the session list.
}

struct CleanupPlan {
    /// Live sessions to close via `agent-browser close --session`.
    close: Vec<String>,
    /// Session names whose registry files get deleted.
    remove_files: Vec<String>,
    /// Orphan chrome pids to terminate.
    kill_chromes: Vec<u32>,
    /// Unowned agent-browser daemon pids to terminate.
    kill_daemons: Vec<u32>,
    /// Stray profile dirs to delete.
    remove_dirs: Vec<PathBuf>,
}

fn plan_cleanup(
    inv: &Inventory,
    close_live: bool,
    only_session: Option<&str>,
    older_than: Option<u64>,
) -> CleanupPlan {
    let mut plan = CleanupPlan {
        close: Vec::new(),
        remove_files: Vec::new(),
        kill_chromes: Vec::new(),
        kill_daemons: Vec::new(),
        remove_dirs: Vec::new(),
    };
    let in_scope = |s: &&SessionInfo| {
        if let Some(only) = only_session {
            return s.name == only;
        }
        older_than.map_or(true, |min| s.age_secs >= min)
    };
    for s in inv.sessions.iter().filter(in_scope) {
        match s.state {
            SessionState::Live => {
                if close_live || only_session.is_some() {
                    plan.close.push(s.name.clone());
                    // `close` removes the live files; residue (.config/
                    // .target) still needs the sweep below.
                    plan.remove_files.push(s.name.clone());
                }
            }
            SessionState::Orphan | SessionState::Stale => {
                plan.remove_files.push(s.name.clone());
            }
        }
    }
    // Orphan chromes are definitionally unowned — always reap them.
    // A `--session` filter still reaps them when the target is itself
    // orphan/stale (its chrome is among the unowned set); a live-session
    // target leaves unrelated zombies alone.
    let reap_chromes = match only_session {
        None => true,
        // Only a live-session target suppresses the chrome sweep — a
        // dead/missing session name means the caller wants the zombies.
        Some(only) => !inv
            .sessions
            .iter()
            .any(|s| s.name == only && s.state == SessionState::Live),
    };
    if reap_chromes {
        plan.kill_chromes = inv.orphan_chromes.iter().map(|c| c.pid).collect();
        plan.kill_daemons = inv.orphan_daemons.iter().map(|d| d.pid).collect();
        // Stray dirs plus the profile dirs of the orphans being killed —
        // the dir is trash once its chrome is dead.
        let mut dirs = inv.stray_dirs.clone();
        for c in &inv.orphan_chromes {
            let p = PathBuf::from(&c.profile_dir);
            if !dirs.contains(&p) {
                dirs.push(p);
            }
        }
        plan.remove_dirs = dirs;
    }
    plan
}

pub fn cleanup(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    let mut dry_run = false;
    let mut all = false;
    let mut only_session: Option<String> = None;
    let mut older_than: Option<u64> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_out = true,
            "--dry-run" => dry_run = true,
            "--all" => all = true,
            "--session" => {
                i += 1;
                match args.get(i) {
                    Some(v) => only_session = Some(v.clone()),
                    None => bail!("agent-qa cleanup: --session needs a <name>"),
                }
            }
            "--older-than" => {
                i += 1;
                match args.get(i).and_then(|v| parse_duration(v)) {
                    Some(d) => older_than = Some(d),
                    None => bail!("agent-qa cleanup: --older-than needs a duration like 30m/2h/1d"),
                }
            }
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa cleanup \u{2014} reap agent-browser zombies\n\n\
                     Usage:\n  agent-qa cleanup                  Remove stale files, kill orphaned Chrome\n  \
                     agent-qa cleanup --all          Also close every live session gracefully\n  \
                     agent-qa cleanup --session <n>  Close/reap only session <n>\n  \
                     agent-qa cleanup --older-than 1h   Only sessions older than the duration\n  \
                     agent-qa cleanup --dry-run      Show the plan without changing anything\n  \
                     agent-qa cleanup --json         Machine-readable plan + result"
                );
                return Ok(0);
            }
            other => bail!("agent-qa cleanup: unknown flag {other:?}"),
        }
        i += 1;
    }

    let Some(inv) = collect() else {
        if json_out {
            println!(
                "{{\"closed\":[],\"removedFiles\":[],\"killedChromes\":[],\"removedDirs\":[]}}"
            );
        } else {
            println!("No agent-browser state dir found — nothing to clean.");
        }
        return Ok(0);
    };
    let plan = plan_cleanup(&inv, all, only_session.as_deref(), older_than);

    if dry_run {
        let body = json!({
            "dryRun": true,
            "close": plan.close,
            "removeSessionFiles": plan.remove_files,
            "killChromes": plan.kill_chromes,
            "removeDirs": plan.remove_dirs,
        });
        if json_out {
            println!("{}", serde_json::to_string_pretty(&body)?);
        } else {
            println!("cleanup plan (dry-run):");
            for n in &plan.close {
                println!("  close live session {n}");
            }
            for n in &plan.remove_files {
                println!("  remove registry files for {n}");
            }
            for p in &plan.kill_chromes {
                println!("  kill orphan chrome pid {p}");
            }
            for p in &plan.kill_daemons {
                println!("  kill orphan daemon pid {p}");
            }
            for d in &plan.remove_dirs {
                println!("  remove {}", d.display());
            }
            if plan.close.is_empty()
                && plan.remove_files.is_empty()
                && plan.kill_chromes.is_empty()
                && plan.kill_daemons.is_empty()
                && plan.remove_dirs.is_empty()
            {
                println!("  nothing to do — already clean.");
            }
        }
        return Ok(0);
    }

    let mut closed = Vec::new();
    for name in &plan.close {
        if crate::browser::close_session(name) {
            closed.push(name.clone());
        } else {
            eprintln!(
                "agent-qa cleanup: close --session {name} failed; killing the daemon directly"
            );
            // A wedged daemon ignores the socket close — SIGTERM/SIGKILL the
            // recorded pid so its chrome tree doesn't leak headless. The
            // children reparent to init and the rescan below sweeps them.
            if let Some(pid) = inv
                .sessions
                .iter()
                .find(|s| s.name == *name)
                .and_then(|s| s.pid)
            {
                term_then_kill(pid);
            }
        }
    }
    let mut removed_files = 0usize;
    for s in inv
        .sessions
        .iter()
        .filter(|s| plan.remove_files.contains(&s.name))
    {
        for f in &s.files {
            if fs::remove_file(f).is_ok() {
                removed_files += 1;
            }
        }
    }
    for pid in &plan.kill_chromes {
        term_then_kill(*pid);
    }
    for pid in &plan.kill_daemons {
        term_then_kill(*pid);
    }
    // Re-scan once closes/removals land: a live session's chrome children
    // only become unowned AFTER the daemon dies, so the plan-time orphan
    // list can't see them (the registry files are already gone here, so
    // remaining live sessions still own their trees and stay untouched).
    if !plan.close.is_empty() {
        std::thread::sleep(std::time::Duration::from_millis(900));
    }
    let mut killed_pids = plan.kill_chromes.clone();
    let mut extra_dirs: Vec<PathBuf> = Vec::new();
    if let Some(fresh) = collect() {
        for c in &fresh.orphan_chromes {
            if !killed_pids.contains(&c.pid) {
                term_then_kill(c.pid);
                killed_pids.push(c.pid);
                extra_dirs.push(PathBuf::from(&c.profile_dir));
            }
        }
        for d in &fresh.orphan_daemons {
            if !killed_pids.contains(&d.pid) {
                term_then_kill(d.pid);
                killed_pids.push(d.pid);
            }
        }
        for d in &fresh.stray_dirs {
            if !plan.remove_dirs.contains(d) && !extra_dirs.contains(d) {
                extra_dirs.push(d.clone());
            }
        }
    }
    let mut removed_dirs = 0usize;
    for d in plan.remove_dirs.iter().chain(extra_dirs.iter()) {
        if fs::remove_dir_all(d).is_ok() {
            removed_dirs += 1;
        }
    }

    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "closed": closed,
                "removedFiles": removed_files,
                "killedChromes": killed_pids,
                "removedDirs": removed_dirs,
            }))?
        );
    } else {
        if closed.is_empty() && removed_files == 0 && killed_pids.is_empty() && removed_dirs == 0 {
            println!("Already clean — nothing reaped.");
        } else {
            if !closed.is_empty() {
                println!("closed sessions: {}", closed.join(", "));
            }
            if removed_files > 0 {
                println!("removed {removed_files} registry files");
            }
            if !killed_pids.is_empty() {
                println!("killed {} orphan process(es)", killed_pids.len());
            }
            if removed_dirs > 0 {
                println!("removed {removed_dirs} stray profile dirs");
            }
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_units() {
        assert_eq!(parse_duration("90"), Some(90));
        assert_eq!(parse_duration("30s"), Some(30));
        assert_eq!(parse_duration("5m"), Some(300));
        assert_eq!(parse_duration("2h"), Some(7200));
        assert_eq!(parse_duration("1d"), Some(86400));
        assert_eq!(parse_duration("x"), None);
    }

    #[test]
    fn scan_groups_by_prefix() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(dir.path().join("s1.config"), "x").unwrap();
        fs::write(dir.path().join("s1.target"), "{}").unwrap();
        fs::write(dir.path().join("s2.config"), "x").unwrap();
        fs::File::create(dir.path().join("noext")).unwrap();
        let groups = scan_socket_dir(dir.path());
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "s1");
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn chrome_profile_dir_parsed() {
        let r = ProcRow {
            pid: 1,
            ppid: 0,
            etimes: 0,
            rss_kb: 0,
            args:
                "/opt/chrome/chrome --headless --user-data-dir=/tmp/agent-browser-chrome-abc123 --x"
                    .into(),
        };
        assert_eq!(
            r.chrome_profile_dir().as_deref(),
            Some("/tmp/agent-browser-chrome-abc123")
        );
        let other = ProcRow {
            args: "/usr/bin/chrome".into(),
            ..r
        };
        assert!(other.chrome_profile_dir().is_none());
    }

    #[test]
    fn plan_defaults_reap_only_dead() {
        let inv = Inventory {
            socket_dir: PathBuf::from("/x"),
            sessions: vec![
                SessionInfo {
                    name: "live1".into(),
                    state: SessionState::Live,
                    pid: Some(1),
                    age_secs: 10,
                    rss_kb: None,
                    url: None,
                    files: vec![],
                },
                SessionInfo {
                    name: "dead1".into(),
                    state: SessionState::Orphan,
                    pid: Some(2),
                    age_secs: 10,
                    rss_kb: None,
                    url: None,
                    files: vec![],
                },
                SessionInfo {
                    name: "stale1".into(),
                    state: SessionState::Stale,
                    pid: None,
                    age_secs: 10,
                    rss_kb: None,
                    url: None,
                    files: vec![],
                },
            ],
            orphan_chromes: vec![OrphanChrome {
                pid: 9,
                rss_kb: 1,
                profile_dir: "/tmp/agent-browser-chrome-x".into(),
            }],
            orphan_daemons: vec![],
            stray_dirs: vec![PathBuf::from("/tmp/agent-browser-chrome-y")],
        };
        let plan = plan_cleanup(&inv, false, None, None);
        assert!(plan.close.is_empty());
        assert_eq!(plan.remove_files, vec!["dead1", "stale1"]);
        assert_eq!(plan.kill_chromes, vec![9]);
        // Stray dir plus the killed orphan's own profile dir.
        assert_eq!(plan.remove_dirs.len(), 2);

        let plan = plan_cleanup(&inv, true, None, None);
        assert_eq!(plan.close, vec!["live1"]);
        assert_eq!(plan.remove_files.len(), 3);

        // --session on a live session closes just that one and leaves
        // global orphan/dirs untouched.
        let plan = plan_cleanup(&inv, false, Some("live1"), None);
        assert_eq!(plan.close, vec!["live1"]);
        assert!(plan.kill_chromes.is_empty());
        assert!(plan.remove_dirs.is_empty());
    }

    #[test]
    fn older_than_filters_sessions() {
        let mk = |name: &str, age: u64| SessionInfo {
            name: name.into(),
            state: SessionState::Stale,
            pid: None,
            age_secs: age,
            rss_kb: None,
            url: None,
            files: vec![],
        };
        let inv = Inventory {
            socket_dir: PathBuf::from("/x"),
            sessions: vec![mk("new", 10), mk("old", 7200)],
            orphan_chromes: vec![],
            orphan_daemons: vec![],
            stray_dirs: vec![],
        };
        let plan = plan_cleanup(&inv, false, None, Some(3600));
        assert_eq!(plan.remove_files, vec!["old"]);
    }

    #[test]
    fn ps_text_runs_on_empty_dir() {
        // Collect against a guaranteed-empty socket dir.
        let dir = tempfile::TempDir::new().unwrap();
        std::env::set_var("AGENT_BROWSER_SOCKET_DIR", dir.path());
        let _ = ps(&["--json".to_string()]);
        std::env::remove_var("AGENT_BROWSER_SOCKET_DIR");
    }
}
