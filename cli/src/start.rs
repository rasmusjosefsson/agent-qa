//! `start` verb — begin a new recording session.
//!
//! Mints a scenario id, primes the recorder workfiles, and (optionally)
//! opens the dashboard URL via the live agent-browser session. Each
//! subsequent `record-step` call appends to `scenario.steps.jsonl`; the
//! `flush` verb (lands later) turns the buffer into a sealed
//! `scenario.json`.
//!
//! Scope is intentionally narrow:
//!
//!   - Mint `<sid>` = `s-<isoTs>__<hex>` (deterministic shape, safe segment)
//!   - mkdir the scenario directory + the recorder root
//!   - Write `tmp/agent-qa-record/scenario.env` carrying SID + INTENT + STARTED
//!   - Truncate `scenario.steps.jsonl` (fresh buffer)
//!   - Write `tmp/agent-qa-record/scenario.last` carrying the SID
//!   - If `--open <url>` is passed, also `browser::open(session, url)`
//!
//! Plugin-supplied auth, session policy, and the "prep" lifecycle
//! integrate here later via the plugin protocol.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};

use crate::browser;
use crate::paths;

const DEFAULT_SESSION: &str = "default";

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let summary = start(&opts)?;
    println!("started sid={}", summary.sid);
    if let Some(env_file) = &summary.env_file {
        println!("env:    {}", env_file.display());
    }
    if let Some(jdir) = &summary.scenario_dir {
        println!("dir:    {}", jdir.display());
    }
    if let Some(url) = &summary.opened_url {
        println!("opened: {url}");
    }
    Ok(0)
}

#[derive(Debug, Clone)]
struct Opts {
    intent: String,
    session_name: String,
    open_url: Option<String>,
    profile: Option<String>,
    keep_session: bool,
}

#[derive(Debug, Clone)]
struct StartSummary {
    sid: String,
    scenario_dir: Option<PathBuf>,
    env_file: Option<PathBuf>,
    opened_url: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut intent: Option<String> = None;
    let mut session: Option<String> = None;
    let mut open_url: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut keep_session = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            "--open" => open_url = it.next().cloned(),
            s if s.starts_with("--open=") => open_url = Some(s["--open=".len()..].to_string()),
            "--profile" => profile = it.next().cloned(),
            s if s.starts_with("--profile=") => profile = Some(s["--profile=".len()..].to_string()),
            "--keep-session" => keep_session = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if intent.is_some() {
                    bail!("unexpected positional {other:?}; usage: start \"<intent>\"");
                }
                intent = Some(other.to_string());
            }
        }
    }
    if profile.is_some() && keep_session {
        bail!("--profile and --keep-session are mutually exclusive");
    }
    let intent = intent.ok_or_else(|| {
        anyhow!("usage: start \"<intent>\" [--session <n>] [--open <url>] [--profile <p>] [--keep-session]")
    })?;
    let session_name = session.unwrap_or_else(|| DEFAULT_SESSION.to_string());
    Ok(Opts {
        intent,
        session_name,
        open_url,
        profile,
        keep_session,
    })
}

fn print_help() {
    println!(
        "agent-qa start — begin a new recording session

Usage:
  agent-qa start \"<intent>\" [--session <name>] [--open <url>]
                              [--profile <name> | --keep-session]

Session baseline (drives `env.open[]` in the final scenario.json):
  (default)        env.open = [{{ \"kind\": \"fresh\" }}] — replay clears
                   cookies + localStorage + sessionStorage before step 0.
  --profile <n>    env.open = [{{ \"kind\": \"useProfile\", \"name\": \"<n>\" }}]
                   — replay re-runs `profile-bootstrap <n>` first.
  --keep-session   env.open = [] — replay against whatever cookies exist
                   (use for debugging only; defeats reproducibility).

Effects:
  - Mints a new <sid>
  - Creates <scenarios_root>/<sid>/
  - Writes tmp/agent-qa-record/scenario.env (SID + INTENT + STARTED +
    SESSION + the chosen baseline marker)
  - Truncates tmp/agent-qa-record/scenario.steps.jsonl
  - Writes tmp/agent-qa-record/scenario.last
  - If --open <url> is passed, agent-browser open <url>"
    );
}

fn start(opts: &Opts) -> Result<StartSummary> {
    let sid = mint_sid();
    let scenario_dir = paths::scenario_dir(&sid)?;
    fs::create_dir_all(&scenario_dir)
        .with_context(|| format!("mkdir -p {}", scenario_dir.display()))?;

    let record_root = paths::record_root();
    fs::create_dir_all(&record_root)
        .with_context(|| format!("mkdir -p {}", record_root.display()))?;

    let env_file = paths::record_env_file();
    write_env_file(&env_file, &sid, &opts.intent, &opts.session_name, opts)?;

    // Truncate (or create) the steps buffer.
    fs::write(paths::record_steps_jsonl(), b"")
        .with_context(|| format!("truncate {}", paths::record_steps_jsonl().display()))?;

    // Stamp the last-sid pointer.
    fs::write(paths::record_last_sid_file(), format!("{sid}\n"))
        .with_context(|| format!("write {}", paths::record_last_sid_file().display()))?;

    let mut summary = StartSummary {
        sid: sid.clone(),
        scenario_dir: Some(scenario_dir),
        env_file: Some(env_file),
        opened_url: None,
    };

    if let Some(url) = &opts.open_url {
        browser::open(&opts.session_name, url)
            .with_context(|| format!("agent-browser open {url}"))?;
        summary.opened_url = Some(url.clone());
    }
    Ok(summary)
}

fn write_env_file(
    path: &std::path::Path,
    sid: &str,
    intent: &str,
    session: &str,
    opts: &Opts,
) -> Result<()> {
    let baseline = if opts.keep_session {
        "KEEP_SESSION".to_string()
    } else if let Some(p) = &opts.profile {
        format!("PROFILE={p}")
    } else {
        "FRESH".to_string()
    };
    let body = format!(
        "SID={sid}\nINTENT={intent_quoted}\nSESSION={session}\nBASELINE={baseline}\nSTARTED={ts}\n",
        intent_quoted = quote_env(intent),
        ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
    );
    let mut f = fs::File::create(path).with_context(|| format!("create {}", path.display()))?;
    f.write_all(body.as_bytes())?;
    Ok(())
}

fn quote_env(s: &str) -> String {
    // shell-friendly single-quote escape
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn mint_sid() -> String {
    let ts = chrono::Utc::now()
        .format("%Y-%m-%dT%H-%M-%S-%3fZ")
        .to_string();
    let mut bytes = [0u8; 4];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    use std::fmt::Write;
    let mut hex = String::with_capacity(8);
    for b in bytes {
        write!(hex, "{b:02x}").unwrap();
    }
    format!("s-{ts}__{hex}")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    fn install_fake_browser(dir: &Path, log: &Path) {
        let body = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let p = dir.join("agent-browser");
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        std::env::set_var(browser::BIN_ENV, &p);
        browser::_reset_bin_cache_for_tests();
    }

    fn clear_fake_browser() {
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn start_mints_sid_and_writes_env_file() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("rec"));
        install_fake_browser(tmp.path(), &tmp.path().join("ab.log"));

        let opts = Opts {
            intent: "open the home page".into(),
            session_name: "default".into(),
            open_url: None,
            profile: None,
            keep_session: false,
        };
        let s = start(&opts).unwrap();
        assert!(s.sid.starts_with("s-"));
        let env = fs::read_to_string(s.env_file.as_ref().unwrap()).unwrap();
        assert!(env.contains(&format!("SID={}", s.sid)));
        assert!(env.contains("INTENT='open the home page'"));
        // steps.jsonl truncated (file exists, empty).
        let steps = fs::read(paths::record_steps_jsonl()).unwrap();
        assert!(steps.is_empty());
        // last-sid pointer.
        let last = fs::read_to_string(paths::record_last_sid_file()).unwrap();
        assert_eq!(last.trim(), s.sid);
        // scenario dir created.
        assert!(s.scenario_dir.unwrap().is_dir());

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake_browser();
    }

    #[test]
    fn start_with_open_invokes_browser() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("rec"));
        let log = tmp.path().join("ab.log");
        install_fake_browser(tmp.path(), &log);

        let opts = Opts {
            intent: "land on home".into(),
            session_name: "sx".into(),
            open_url: Some("https://example.com/".into()),
            profile: None,
            keep_session: false,
        };
        let s = start(&opts).unwrap();
        assert_eq!(s.opened_url.as_deref(), Some("https://example.com/"));
        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            invocation.contains("--session sx open https://example.com/"),
            "got: {invocation}"
        );

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake_browser();
    }

    #[test]
    fn mint_sid_shape() {
        let id = mint_sid();
        let parts: Vec<&str> = id.split("__").collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("s-"));
        assert_eq!(parts[1].len(), 8);
        assert!(parts[1].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parse_args_requires_intent() {
        parse_args(&["--session=x".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_session_and_open() {
        let opts = parse_args(&[
            "smoke".into(),
            "--session".into(),
            "sx".into(),
            "--open=https://x/".into(),
        ])
        .unwrap();
        assert_eq!(opts.intent, "smoke");
        assert_eq!(opts.session_name, "sx");
        assert_eq!(opts.open_url.as_deref(), Some("https://x/"));
    }
}
