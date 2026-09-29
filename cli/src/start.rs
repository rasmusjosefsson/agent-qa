//! `start` begins a typed recording session.

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};

use crate::browser;
use crate::paths;
use crate::recorder_state::{RecorderBaseline, RecorderState};

const DEFAULT_SESSION: &str = "default";

pub fn run(args: &[String]) -> Result<u8> {
    let summary = start(&parse_args(args)?)?;
    println!("started sid={}", summary.sid);
    if let Some(dir) = summary.scenario_dir {
        println!("dir:    {}", dir.display());
    }
    if let Some(url) = summary.opened_url {
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
    headed: bool,
    source_ref: Option<String>,
    mock_from: Option<String>,
    offline: bool,
}

#[derive(Debug, Clone)]
struct StartSummary {
    sid: String,
    scenario_dir: Option<PathBuf>,
    opened_url: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut intent = None;
    let mut session = None;
    let mut open_url = None;
    let mut profile = None;
    let mut keep_session = false;
    let mut headed = false;
    let mut source_ref = None;
    let mut mock_from = None;
    let mut offline = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--session" => session = it.next().cloned(),
            value if value.starts_with("--session=") => {
                session = Some(value["--session=".len()..].to_string())
            }
            "--open" => open_url = it.next().cloned(),
            value if value.starts_with("--open=") => {
                open_url = Some(value["--open=".len()..].to_string())
            }
            "--profile" => profile = it.next().cloned(),
            value if value.starts_with("--profile=") => {
                profile = Some(value["--profile=".len()..].to_string())
            }
            "--keep-session" => keep_session = true,
            "--headed" => headed = true,
            "--headless" => headed = false,
            "--source-ref" => source_ref = it.next().cloned(),
            value if value.starts_with("--source-ref=") => {
                source_ref = Some(value["--source-ref=".len()..].to_string())
            }
            "--mock-from" => mock_from = it.next().cloned(),
            value if value.starts_with("--mock-from=") => {
                mock_from = Some(value["--mock-from=".len()..].to_string())
            }
            "--offline" => offline = true,
            value if value.starts_with("--") => bail!("unknown flag {value:?}"),
            value => {
                if intent.is_some() {
                    bail!("unexpected positional {value:?}; usage: start \"<intent>\"");
                }
                intent = Some(value.to_string());
            }
        }
    }
    if profile.is_some() && keep_session {
        bail!("--profile and --keep-session are mutually exclusive");
    }
    let intent = intent.ok_or_else(|| anyhow!("usage: start \"<intent>\" [--session <n>] [--open <url>] [--profile <p>] [--keep-session]"))?;
    if source_ref.as_deref().is_some_and(str::is_empty) {
        bail!("--source-ref must not be empty");
    }
    let session_name = session
        .or_else(|| {
            std::env::var("AGENT_BROWSER_SESSION")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| DEFAULT_SESSION.to_string());
    Ok(Opts {
        intent,
        session_name,
        open_url,
        profile,
        keep_session,
        headed,
        source_ref,
        mock_from,
        offline,
    })
}

fn print_help() {
    println!(
        "agent-qa start - begin a recording session

Usage:
  agent-qa start \"<intent>\" [--session <name>] [--open <url>]
                              [--profile <name> | --keep-session]
                              [--source-ref <opaque-reference>]
                              [--mock-from <network.har>] [--offline]

Writes one local recorder-state.json file. The sealed scenario never includes browser connection settings.

--mock-from stubs the session's fetch/XHR from a recorded HAR (any
`network.har` a `replay --har` or a previous recording produced); the
stub installs before every navigation, so even page-load calls are
covered. --offline rejects every unmatched fetch/XHR — combine both to
record hermetically against a dead backend."
    );
}

fn start(opts: &Opts) -> Result<StartSummary> {
    browser::set_headed_mode(opts.headed);
    let connection = browser::BrowserConnection::resolve()?;
    browser::set_connection(&connection);
    let sid = mint_sid();
    let scenario_dir = paths::scenario_dir(&sid)?;
    fs::create_dir_all(&scenario_dir)
        .with_context(|| format!("mkdir -p {}", scenario_dir.display()))?;
    fs::create_dir_all(paths::record_root())
        .with_context(|| format!("mkdir -p {}", paths::record_root().display()))?;
    let state = RecorderState::new(
        sid.clone(),
        opts.intent.clone(),
        opts.session_name.clone(),
        resolve_baseline(opts),
        opts.source_ref.clone(),
        connection,
    );
    state.save()?;
    fs::write(paths::record_last_sid_file(), format!("{sid}\n"))
        .with_context(|| format!("write {}", paths::record_last_sid_file().display()))?;
    // Hermetic capture: seed stubs + write the page-init script BEFORE the
    // session launches so agent-browser registers it ahead of the first
    // navigation (mirrors replay's --mock-from wiring; the eval below covers
    // a reused warm session, which ignores init scripts).
    if opts.mock_from.is_some() || opts.offline {
        if opts.offline {
            crate::mock::set_strict(&opts.session_name, true);
        }
        if let Some(from) = &opts.mock_from {
            let n = crate::mock::seed_from_har_path(&opts.session_name, std::path::Path::new(from))
                .with_context(|| format!("--mock-from {from:?}"))?;
            eprintln!("[v2-record] mock-from {from}: {n} stub(s) seeded");
        }
        let js_path = crate::mock::write_init_script(&opts.session_name, &scenario_dir)
            .with_context(|| "mock seed: write init script")?;
        std::env::set_var("AGENT_BROWSER_INIT_SCRIPTS", &js_path);
        eprintln!("[v2-record] mock init script {}", js_path.display());
    }
    let mut summary = StartSummary {
        sid,
        scenario_dir: Some(scenario_dir),
        opened_url: None,
    };
    if let Some(url) = &opts.open_url {
        browser::open(&opts.session_name, url)
            .with_context(|| format!("agent-browser open {url}"))?;
        summary.opened_url = Some(url.clone());
    }
    // Record the session's traffic alongside the steps: flush stops it into
    // `replays/recorded/network.har`, and `replay --mock-from recorded`
    // replays the scenario hermetically from that recording.
    if let Err(e) = browser::network_har_start(&opts.session_name) {
        eprintln!("[v2-record] har start skipped: {e}");
    }
    // A reused warm session never saw the init script — install the stub
    // into the live document so its in-page calls are covered too.
    if let Err(e) = crate::mock::reapply_if_any(&opts.session_name) {
        eprintln!("[v2-record] mock apply skipped: {e}");
    }
    Ok(summary)
}

fn resolve_baseline(opts: &Opts) -> RecorderBaseline {
    let environment = std::env::var("AGENT_QA_PROFILE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    RecorderBaseline::from_start(
        opts.profile.as_deref().or(environment.as_deref()),
        opts.keep_session,
    )
}

fn mint_sid() -> String {
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S-%3fZ");
    let mut bytes = [0u8; 4];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    format!(
        "s-{timestamp}__{}",
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn install_fake_browser(dir: &std::path::Path, log: &std::path::Path) {
        let path = dir.join("agent-browser");
        fs::write(
            &path,
            format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, &path);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn start_writes_only_typed_recorder_state() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        install_fake_browser(tmp.path(), &tmp.path().join("browser.log"));
        let summary = start(&Opts {
            intent: "open home".into(),
            session_name: "default".into(),
            open_url: None,
            profile: None,
            keep_session: false,
            headed: false,
            source_ref: None,
            mock_from: None,
            offline: false,
        })
        .unwrap();
        assert_eq!(RecorderState::load_active().unwrap().sid, summary.sid);
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn mock_from_writes_an_init_script_into_the_scenario_dir() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        install_fake_browser(tmp.path(), &tmp.path().join("browser.log"));
        let har = tmp.path().join("network.har");
        fs::write(
            &har,
            r#"{"log":{"entries":[
              {"request":{"url":"https://x/api/u","method":"GET"},
               "response":{"status":200,"content":{"text":"{\"u\":1}"}}}
            ]}}"#,
        )
        .unwrap();
        let summary = start(&Opts {
            intent: "stubbed record".into(),
            session_name: "default".into(),
            open_url: None,
            profile: None,
            keep_session: false,
            headed: false,
            source_ref: None,
            mock_from: Some(har.display().to_string()),
            offline: true,
        })
        .unwrap();
        let init = scenario_dir_of(&summary).join("mock-init.js");
        assert!(init.is_file(), "init script at {}", init.display());
        let body = fs::read_to_string(&init).unwrap();
        assert!(body.contains("https://x/api/u"), "HAR URL seeded");
        assert!(body.contains("Failed to fetch (offline)"), "strict mode on");
        assert_eq!(
            std::env::var("AGENT_BROWSER_INIT_SCRIPTS").unwrap(),
            init.display().to_string(),
            "the launch env registers the init script"
        );
        std::env::remove_var("AGENT_BROWSER_INIT_SCRIPTS");
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }

    fn scenario_dir_of(summary: &StartSummary) -> PathBuf {
        summary.scenario_dir.clone().unwrap()
    }
}
