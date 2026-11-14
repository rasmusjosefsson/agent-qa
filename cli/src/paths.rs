//! Shared path helpers for the scenarios root, a scenario directory,
//! the artefact tree (`replays/`, `recording/`, `baselines/`, `compare/`,
//! `perf/`), recorder workfiles, and profiles.
//!
//! Production code appends those segments through the helpers below.
//! The layout test locks the segment names.
//!
//! ## Configuration
//!
//! The scenarios root is overridable via [`SCENARIOS_DIR_ENV`]:
//!
//! - Absolute path is used as-is.
//! - Relative path resolves against the current working directory.
//! - When unset, the root is `<cwd>/tmp/agent-qa-scenarios`.
//!
//! Resolved fresh on every call so tests and scripts that change the env
//! between calls see the change.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Deserialize;
use std::fs;

pub const SCENARIOS_DIR_ENV: &str = "AGENT_QA_SCENARIOS_DIR";
pub const DEFAULT_SCENARIOS_SUBDIR: &str = "tmp/agent-qa-scenarios";

/// Filename the config loader walks up to find. Mirrors the plugin
/// discovery filename so both surfaces share one config file.
const CONFIG_FILES: &[&str] = &["agent-qa.toml", ".agent-qa.toml"];

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ConfigFile {
    #[serde(default)]
    paths: Option<PathsTable>,
    #[serde(default)]
    browser: Option<BrowserConfig>,
    #[serde(default)]
    pub(crate) baselines: Option<BaselinesTable>,
    #[serde(default)]
    pub(crate) mail: Option<MailTable>,
    #[serde(default)]
    pub(crate) notify: Option<NotifyTable>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct BrowserConfig {
    pub(crate) cdp: Option<String>,
    pub(crate) pin_tab: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct PathsTable {
    scenarios_root: Option<String>,
    record_root: Option<String>,
}

/// `[baselines]` — remote golden store config. `store` picks the backend:
/// absent or "local" = `<sid>/baselines/` lives only in the scenario dir
/// (today's behaviour); "github" = a second repository's Contents API;
/// "turso" = a libSQL/Turso database over its HTTP pipeline. Remote refs are
/// never embedded in the toml — tokens come from `token_env`'s env var.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct BaselinesTable {
    pub(crate) store: Option<String>,
    pub(crate) repo: Option<String>,
    pub(crate) branch: Option<String>,
    pub(crate) prefix: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) token_env: Option<String>,
}

/// The `[baselines]` section of the active `agent-qa.toml` (None when the
/// table — or any config file — is absent).
pub(crate) fn baselines_config() -> Option<(PathBuf, BaselinesTable)> {
    let cwd = env::current_dir().ok()?;
    let (cfg_path, cfg) = load_config(&cwd)?;
    cfg.baselines.map(|t| (cfg_path, t))
}

/// `[mail]` — HTTP API of the test inbox the `mail` verb reads.
/// `url` points at the server's API base (mailpit: `http://host:8025`,
/// mailhog: `http://host:8025` — both detected by response shape).
/// No credentials are read here; if the inbox sits behind auth, front it
/// with a header through a local proxy instead of storing a token.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MailTable {
    pub(crate) url: Option<String>,
}

/// The `[mail]` section of the active `agent-qa.toml`.
pub(crate) fn mail_config() -> Option<(PathBuf, MailTable)> {
    let cwd = env::current_dir().ok()?;
    let (cfg_path, cfg) = load_config(&cwd)?;
    cfg.mail.map(|t| (cfg_path, t))
}

/// `[notify]` — the run-verdict alert sink. `url` is the webhook the
/// finished run's verdict is POSTed to; `on` narrows when: "failure"
/// (default) posts only non-pass runs, "always" posts every run.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct NotifyTable {
    pub(crate) url: Option<String>,
    pub(crate) on: Option<String>,
}

/// The `[notify]` section of the active `agent-qa.toml`.
pub(crate) fn notify_config() -> Option<(PathBuf, NotifyTable)> {
    let cwd = env::current_dir().ok()?;
    let (cfg_path, cfg) = load_config(&cwd)?;
    cfg.notify.map(|t| (cfg_path, t))
}

fn load_config(start: &Path) -> Option<(PathBuf, ConfigFile)> {
    let mut cur: Option<&Path> = Some(start);
    while let Some(dir) = cur {
        for name in CONFIG_FILES {
            let candidate = dir.join(name);
            if candidate.is_file() {
                let bytes = match fs::read_to_string(&candidate) {
                    Ok(b) => b,
                    Err(_) => return None,
                };
                let cfg: ConfigFile = toml::from_str(&bytes).ok()?;
                return Some((candidate, cfg));
            }
        }
        cur = dir.parent();
    }
    None
}

/// Locate the active `agent-qa.toml` walking from `cwd` up to root.
/// Returns the path on success, None otherwise. Public so callers like
/// `config show` can surface which file (if any) drove resolution.
pub fn locate_config_file() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    load_config(&cwd).map(|(path, _)| path)
}

fn resolve_relative(value: &str, base: &Path) -> PathBuf {
    let p = PathBuf::from(value);
    if p.is_absolute() {
        return p;
    }
    base.join(p)
}

/// Resolve the scenarios root.
///
/// Priority (first hit wins):
///   1. `AGENT_QA_SCENARIOS_DIR` env var (absolute or cwd-relative)
///   2. `agent-qa.toml [paths].scenarios_root` (absolute or relative to
///      the directory holding the toml file)
///   3. default `<cwd>/tmp/agent-qa-scenarios`
pub fn scenarios_root() -> PathBuf {
    if let Ok(v) = env::var(SCENARIOS_DIR_ENV) {
        if !v.is_empty() {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            return resolve_relative(&v, &cwd);
        }
    }
    if let Ok(cwd) = env::current_dir() {
        if let Some((cfg_path, cfg)) = load_config(&cwd) {
            if let Some(p) = cfg.paths.as_ref().and_then(|t| t.scenarios_root.as_deref()) {
                let base = cfg_path.parent().unwrap_or(&cwd);
                return resolve_relative(p, base);
            }
        }
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(DEFAULT_SCENARIOS_SUBDIR)
}

/// Per-scenario directory: `<scenarios_root>/<sid>/`.
pub fn scenario_dir(sid: &str) -> Result<PathBuf> {
    Ok(scenarios_root().join(safe_segment(sid, "sid")?))
}

// ---------- artefact tree ----------
//
// One function per on-disk segment. Callers pass the directory they
// already have; these helpers only append the segment.

/// `screenshots/` — sidecar kind and shot baselines.
pub(crate) const SCREENSHOTS: &str = "screenshots";
/// `snapshots/` — sidecar kind and domshot baselines.
pub(crate) const SNAPSHOTS: &str = "snapshots";
/// `layouts/` — sidecar kind.
pub(crate) const LAYOUTS: &str = "layouts";
/// `probes/` — recording sidecar `truncate` archives.
pub(crate) const PROBES: &str = "probes";
/// `network/` — recording sidecar `truncate` archives.
pub(crate) const NETWORK: &str = "network";

/// `<scenario>/replays`
pub(crate) fn replays_dir(scenario_dir: &Path) -> PathBuf {
    scenario_dir.join("replays")
}

/// `<scenario>/replays/latest.txt`
pub(crate) fn latest_replay_file(scenario_dir: &Path) -> PathBuf {
    replays_dir(scenario_dir).join("latest.txt")
}

/// `<replays>/latest.txt` when the caller already has the replays directory.
pub(crate) fn latest_pointer(replays_dir: &Path) -> PathBuf {
    replays_dir.join("latest.txt")
}

/// `<replays>/recorded` — the session HAR `flush` keeps.
pub(crate) fn recorded_dir(replays_dir: &Path) -> PathBuf {
    replays_dir.join("recorded")
}

/// `<scenario>/recording`
pub(crate) fn recording_dir(scenario_dir: &Path) -> PathBuf {
    scenario_dir.join("recording")
}

/// `<scenario>/recording/failed`
pub(crate) fn recording_failed_dir(scenario_dir: &Path) -> PathBuf {
    recording_dir(scenario_dir).join("failed")
}

/// `<scenario>/recording/heal.jsonl`
pub(crate) fn recording_heal_jsonl(scenario_dir: &Path) -> PathBuf {
    recording_dir(scenario_dir).join("heal.jsonl")
}

/// `<scenario>/compare`
pub(crate) fn compare_dir(scenario_dir: &Path) -> PathBuf {
    scenario_dir.join("compare")
}

/// `<scenario>/baselines`
pub(crate) fn baselines_dir(scenario_dir: &Path) -> PathBuf {
    scenario_dir.join("baselines")
}

/// `<parent>/screenshots` — a replay run, or a compare folder.
pub(crate) fn screenshots_dir(parent: &Path) -> PathBuf {
    parent.join(SCREENSHOTS)
}

/// `<parent>/snapshots`
pub(crate) fn snapshots_dir(parent: &Path) -> PathBuf {
    parent.join(SNAPSHOTS)
}

/// `<parent>/layouts`
pub(crate) fn layouts_dir(parent: &Path) -> PathBuf {
    parent.join(LAYOUTS)
}

/// `<run>/layouts-diff`
pub(crate) fn layouts_diff_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("layouts-diff")
}

/// `<run>/shots-diff`
pub(crate) fn shots_diff_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("shots-diff")
}

/// `<run>/domshot-diff`
pub(crate) fn domshot_diff_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("domshot-diff")
}

/// `<run>/diffs` — auto-heal patch files. Also the compare output's sibling name.
pub(crate) fn diffs_dir(parent: &Path) -> PathBuf {
    parent.join("diffs")
}

/// `<run>/heal-responses`
pub(crate) fn heal_responses_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("heal-responses")
}

/// `<parent>/perf` — scenario perf snapshots, or a run's trace directory.
pub(crate) fn perf_subdir(parent: &Path) -> PathBuf {
    parent.join("perf")
}

/// `<run>/audit.json`
pub(crate) fn audit_file(run_dir: &Path) -> PathBuf {
    run_dir.join("audit.json")
}

/// `<run>/events.jsonl`
pub(crate) fn events_file(run_dir: &Path) -> PathBuf {
    run_dir.join("events.jsonl")
}

/// `<run>/heal.jsonl` or `<recording>/heal.jsonl`
pub(crate) fn heal_jsonl(dir: &Path) -> PathBuf {
    dir.join("heal.jsonl")
}

/// `<run>/network.json`
pub(crate) fn network_json(run_dir: &Path) -> PathBuf {
    run_dir.join("network.json")
}

/// `<dir>/network.har`
pub(crate) fn network_har(dir: &Path) -> PathBuf {
    dir.join("network.har")
}

/// `<compare-folder>/compare.md`
pub(crate) fn compare_report(compare_folder: &Path) -> PathBuf {
    compare_folder.join("compare.md")
}

/// `<run>/status.json`
pub(crate) fn status_file(run_dir: &Path) -> PathBuf {
    run_dir.join("status.json")
}

/// Directories under `<sid>/replays/` that are actual replay runs —
/// i.e. they carry `audit.json` (finished) or `events.jsonl`
/// (in-flight / crashed before the audit write). Anything else —
/// notably the `recorded/` sidecar dir `flush` writes for the session
/// HAR — is not a run and must not win "latest" picks or appear in
/// audit listings. Sorted by directory name (run ids are
/// timestamp-prefixed, so the sort is chronological).
pub fn run_dirs(replays_dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = match fs::read_dir(replays_dir) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && (audit_file(p).is_file() || events_file(p).is_file()))
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

// ---------- perf ----------

pub fn perf_dir(sid: &str) -> Result<PathBuf> {
    Ok(perf_subdir(&scenario_dir(sid)?))
}

// ---------- recorder workfiles ----------

/// Directory under which all recorder workfiles live. Default
/// `<cwd>/tmp/agent-qa-record/`; override with [`RECORD_DIR_ENV`].
pub const RECORD_DIR_ENV: &str = "AGENT_QA_RECORD_DIR";
pub const DEFAULT_RECORD_SUBDIR: &str = "tmp/agent-qa-record";

pub fn record_root() -> PathBuf {
    if let Ok(v) = env::var(RECORD_DIR_ENV) {
        if !v.is_empty() {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            return resolve_relative(&v, &cwd);
        }
    }
    if let Ok(cwd) = env::current_dir() {
        if let Some((cfg_path, cfg)) = load_config(&cwd) {
            if let Some(p) = cfg.paths.as_ref().and_then(|t| t.record_root.as_deref()) {
                let base = cfg_path.parent().unwrap_or(&cwd);
                return resolve_relative(p, base);
            }
        }
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(DEFAULT_RECORD_SUBDIR)
}

/// Resolve local browser settings from `[browser]` in `agent-qa.toml`.
/// Environment variables take precedence in `browser::BrowserConnection`.
pub(crate) fn browser_config() -> BrowserConfig {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(_) => return BrowserConfig::default(),
    };
    load_config(&cwd)
        .and_then(|(_, config)| config.browser)
        .unwrap_or_default()
}

pub fn record_state_file() -> PathBuf {
    record_root().join("recorder-state.json")
}

pub fn record_last_sid_file() -> PathBuf {
    record_root().join("scenario.last")
}

/// Per-profile sidecar root — holds metadata that survives across
/// recordings (registered profile, plugin handle, future profile-scoped
/// state). Distinct from agent-browser's session dir (cookies + storage).
pub fn profiles_root() -> PathBuf {
    record_root().join("profiles")
}

pub fn profile_dir(profile: &str) -> Result<PathBuf> {
    Ok(profiles_root().join(safe_segment(profile, "profile")?))
}

pub fn profile_file(profile: &str) -> Result<PathBuf> {
    Ok(profile_dir(profile)?.join("profile.json"))
}

// ---------- safety ----------

/// Permitted characters for path segments that originate from user input
/// (sid, replayId, compare folder, etc.): ASCII alphanumeric, `.`, `_`, `-`.
fn safe_segment<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    if value.is_empty() || value == "." || value == ".." || !is_safe(value) {
        bail!("{label} must be a non-empty safe path segment, got {value:?}");
    }
    Ok(value)
}

pub(crate) fn is_safe(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

#[cfg(test)]
mod layout {
    use std::path::Path;

    use super::*;

    #[test]
    fn helpers_match_the_on_disk_segments() {
        let sid = Path::new("sid");
        let run = Path::new("run");
        let cmp = Path::new("cmp");
        assert_eq!(replays_dir(sid), sid.join("replays"));
        assert_eq!(
            latest_replay_file(sid),
            sid.join("replays").join("latest.txt")
        );
        assert_eq!(
            latest_pointer(&replays_dir(sid)),
            replays_dir(sid).join("latest.txt")
        );
        assert_eq!(
            recorded_dir(&replays_dir(sid)),
            replays_dir(sid).join("recorded")
        );
        assert_eq!(recording_dir(sid), sid.join("recording"));
        assert_eq!(
            recording_failed_dir(sid),
            sid.join("recording").join("failed")
        );
        assert_eq!(
            recording_heal_jsonl(sid),
            sid.join("recording").join("heal.jsonl")
        );
        assert_eq!(compare_dir(sid), sid.join("compare"));
        assert_eq!(baselines_dir(sid), sid.join("baselines"));
        assert_eq!(screenshots_dir(run), run.join(SCREENSHOTS));
        assert_eq!(snapshots_dir(run), run.join(SNAPSHOTS));
        assert_eq!(layouts_dir(run), run.join(LAYOUTS));
        assert_eq!(layouts_diff_dir(run), run.join("layouts-diff"));
        assert_eq!(shots_diff_dir(run), run.join("shots-diff"));
        assert_eq!(domshot_diff_dir(run), run.join("domshot-diff"));
        assert_eq!(diffs_dir(run), run.join("diffs"));
        assert_eq!(heal_responses_dir(run), run.join("heal-responses"));
        assert_eq!(perf_subdir(sid), sid.join("perf"));
        assert_eq!(audit_file(run), run.join("audit.json"));
        assert_eq!(events_file(run), run.join("events.jsonl"));
        assert_eq!(heal_jsonl(run), run.join("heal.jsonl"));
        assert_eq!(network_json(run), run.join("network.json"));
        assert_eq!(network_har(run), run.join("network.har"));
        assert_eq!(compare_report(cmp), cmp.join("compare.md"));
        assert_eq!(status_file(run), run.join("status.json"));
        assert_eq!(SCREENSHOTS, "screenshots");
        assert_eq!(SNAPSHOTS, "snapshots");
        assert_eq!(LAYOUTS, "layouts");
        assert_eq!(PROBES, "probes");
        assert_eq!(NETWORK, "network");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    // Env mutation isn't thread-safe; serialize via the shared lock.
    use crate::test_util::lock_env;

    #[test]
    fn default_root_is_under_cwd() {
        let _g = lock_env();
        env::remove_var(SCENARIOS_DIR_ENV);
        let cwd = env::current_dir().unwrap();
        assert_eq!(scenarios_root(), cwd.join(DEFAULT_SCENARIOS_SUBDIR));
    }

    #[test]
    fn env_override_absolute() {
        let _g = lock_env();
        env::set_var(SCENARIOS_DIR_ENV, "/var/agent-qa");
        assert_eq!(scenarios_root(), PathBuf::from("/var/agent-qa"));
        env::remove_var(SCENARIOS_DIR_ENV);
    }

    #[test]
    fn env_override_relative() {
        let _g = lock_env();
        env::set_var(SCENARIOS_DIR_ENV, "elsewhere");
        let cwd = env::current_dir().unwrap();
        assert_eq!(scenarios_root(), cwd.join("elsewhere"));
        env::remove_var(SCENARIOS_DIR_ENV);
    }

    #[test]
    fn safe_segment_allows_normal_ids() {
        assert!(safe_segment("abc-123_v2.json", "x").is_ok());
        assert!(safe_segment("hello.world", "x").is_ok());
    }

    #[test]
    fn safe_segment_rejects_traversal_and_specials() {
        assert!(safe_segment(".", "x").is_err());
        assert!(safe_segment("..", "x").is_err());
        assert!(safe_segment("", "x").is_err());
        assert!(safe_segment("a/b", "x").is_err());
        assert!(safe_segment("a b", "x").is_err());
        assert!(safe_segment("a$b", "x").is_err());
    }

    #[test]
    fn perf_dir_is_under_the_scenario() {
        let _g = lock_env();
        env::set_var(SCENARIOS_DIR_ENV, "/tmp/jx");
        assert_eq!(perf_dir("sid").unwrap(), PathBuf::from("/tmp/jx/sid/perf"));
        env::remove_var(SCENARIOS_DIR_ENV);
    }

    #[test]
    fn paths_table_in_agent_qa_toml_picks_up_when_env_missing() {
        let _g = lock_env();
        env::remove_var(SCENARIOS_DIR_ENV);
        env::remove_var(RECORD_DIR_ENV);

        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[paths]\nscenarios_root = \"./customScenarios\"\nrecord_root = \"/abs/rec\"\n",
        )
        .unwrap();

        let prev_cwd = env::current_dir().unwrap();
        env::set_current_dir(tmp.path()).unwrap();

        // Relative path resolves against the toml's directory.
        assert_eq!(
            scenarios_root(),
            tmp.path().canonicalize().unwrap().join("customScenarios")
        );
        // Absolute path passes through.
        assert_eq!(record_root(), PathBuf::from("/abs/rec"));

        env::set_current_dir(prev_cwd).unwrap();
    }

    #[test]
    fn browser_table_loads_from_agent_qa_toml() {
        let _g = lock_env();
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[browser]\ncdp = \"9223\"\npin_tab = true\n",
        )
        .unwrap();
        let prev_cwd = env::current_dir().unwrap();
        env::set_current_dir(tmp.path()).unwrap();

        let config = browser_config();
        assert_eq!(config.cdp.as_deref(), Some("9223"));
        assert_eq!(config.pin_tab, Some(true));

        env::set_current_dir(prev_cwd).unwrap();
    }

    #[test]
    fn env_var_wins_over_toml() {
        let _g = lock_env();
        let tmp = tempfile::TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[paths]\nscenarios_root = \"/from-toml\"\n",
        )
        .unwrap();
        let prev_cwd = env::current_dir().unwrap();
        env::set_current_dir(tmp.path()).unwrap();

        env::set_var(SCENARIOS_DIR_ENV, "/from-env");
        assert_eq!(scenarios_root(), PathBuf::from("/from-env"));

        env::remove_var(SCENARIOS_DIR_ENV);
        env::set_current_dir(prev_cwd).unwrap();
    }
}
