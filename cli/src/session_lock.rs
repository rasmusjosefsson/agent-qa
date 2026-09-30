//! Per-session-name lock so two agent-qa processes never drive the same
//! agent-browser session at once.
//!
//! `<record_root>/locks/<session>.lock` is created atomically
//! (`create_new`) at run start and removed on drop. The body carries the
//! holder's pid + start timestamp: a lock whose pid is dead is stale and
//! stolen silently; a live pid refuses the second run with an error that
//! names the session, the pid, and the remedy. Replay runs hold the lock
//! for their whole lifetime; `record start` reads it to refuse starting a
//! recording on a session that is mid-run.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::paths;

/// Held for the lifetime of a replay; dropping removes the lock file.
#[derive(Debug)]
pub struct SessionLock {
    path: PathBuf,
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Who holds the lock — surfaced in refusal + status messages.
pub struct Holder {
    pub pid: u32,
    pub started: u64,
    pub path: PathBuf,
}

fn lock_path(session: &str) -> PathBuf {
    // The session name is free-form for agent-browser — slug it for the
    // filename so `../`-style or `/`-containing names can't escape the
    // locks dir, and append a digest so names that slugify alike stay
    // distinct locks.
    let slug: String = session
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let slug = if slug.is_empty() || slug == "." || slug == ".." {
        "session".to_string()
    } else {
        slug
    };
    let digest = Sha256::digest(session.as_bytes());
    let short: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
    paths::record_root()
        .join("locks")
        .join(format!("{slug}__{short}.lock"))
}

/// Take the lock for `session`. Refuses when another live process holds
/// it; steals the file silently when the recorded pid is dead.
pub fn acquire(session: &str) -> Result<SessionLock> {
    let path = lock_path(session);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .with_context(|| format!("cannot create lock dir {}", dir.display()))?;
    }
    for attempt in 0..2 {
        match create_lock(&path) {
            Ok(()) => return Ok(SessionLock { path }),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                if attempt > 0 {
                    bail!("could not take session lock {}: {e}", path.display());
                }
                let holder = read_holder(&path);
                if let Some(h) = &holder {
                    if pid_alive(h.pid) {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        bail!(
                            "session '{session}' is in use by another agent-qa run \
                             (pid {}, started {}s ago) — the two runs would drive the \
                             same browser. Use --session <other>, wait for it to \
                             finish, or remove {} if the pid is stale.",
                            h.pid,
                            now.saturating_sub(h.started),
                            h.path.display()
                        );
                    }
                }
                // Dead holder or unreadable body — stale file, steal it.
                let _ = fs::remove_file(&path);
            }
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("cannot create session lock {}", path.display()))
            }
        }
    }
    unreachable!()
}

/// The live holder of `session`'s lock, when one exists.
pub fn held_by_live_process(session: &str) -> Option<Holder> {
    read_holder(&lock_path(session)).filter(|h| pid_alive(h.pid))
}

fn create_lock(path: &PathBuf) -> std::io::Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let body = serde_json::json!({"pid": std::process::id(), "started": started});
    f.write_all(body.to_string().as_bytes())
}

fn read_holder(path: &PathBuf) -> Option<Holder> {
    let body = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    Some(Holder {
        pid: v.get("pid")?.as_u64()? as u32,
        started: v.get("started")?.as_u64().unwrap_or(0),
        path: path.clone(),
    })
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0): 0 = signalable, EPERM = alive under another user,
    // ESRCH = no such process.
    let rc = unsafe { libc::kill(pid as i32, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    // `tasklist /NH /FI "PID eq N"` prints `image.exe <pid> ...` rows, or
    // an INFO line when nothing matches — parse the pid column, don't
    // substring-match (the digits could appear in a memory-size field).
    std::process::Command::new("tasklist")
        .args(["/NH", "/FI", &format!("PID eq {pid}")])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.split_whitespace().nth(1) == Some(pid.to_string().as_str()))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    fn with_record_root(f: impl FnOnce(&TempDir)) {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path());
        f(&tmp);
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn second_acquire_on_a_live_holder_refuses() {
        with_record_root(|_t| {
            let _lock = acquire("demo").unwrap();
            let err = acquire("demo").unwrap_err().to_string();
            assert!(err.contains("in use"), "{err}");
            assert!(err.contains("demo"), "{err}");
        });
    }

    #[test]
    fn a_dead_holders_lock_is_stolen() {
        with_record_root(|_t| {
            // Mint a real-but-dead pid by spawning a trivial child.
            let mut spawned = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--version")
                .spawn()
                .unwrap();
            let pid = spawned.id();
            spawned.wait().unwrap();
            let path = lock_path("demo");
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                &path,
                serde_json::json!({"pid": pid, "started": 1}).to_string(),
            )
            .unwrap();
            assert!(held_by_live_process("demo").is_none());
            let _lock = acquire("demo").unwrap();
            // The dead holder's file was stolen — same path, our pid now.
            let h = read_holder(&path).unwrap();
            assert_eq!(h.pid, std::process::id());
        });
    }

    #[test]
    fn drop_removes_the_lock_file() {
        with_record_root(|_t| {
            let path = lock_path("demo");
            {
                let _lock = acquire("demo").unwrap();
                assert!(path.is_file());
            }
            assert!(!path.exists());
        });
    }

    #[test]
    fn unsafe_session_names_stay_inside_the_locks_dir() {
        with_record_root(|t| {
            for name in ["../../etc/cfg", "a/b", "..", "c:\\x", "a b"] {
                let p = lock_path(name);
                assert!(
                    p.starts_with(t.path().join("locks")),
                    "{name} → {}",
                    p.display()
                );
                assert!(p.file_name().unwrap().to_str().unwrap().ends_with(".lock"));
            }
            // Different names never share a lock file.
            assert_ne!(lock_path("a/b"), lock_path("a_b"));
            assert_ne!(lock_path(".."), lock_path("."));
        });
    }

    #[test]
    fn holder_reports_pid_and_started() {
        with_record_root(|_t| {
            let _lock = acquire("demo").unwrap();
            let h = held_by_live_process("demo").unwrap();
            assert_eq!(h.pid, std::process::id());
            assert!(h.started > 0);
        });
    }
}
