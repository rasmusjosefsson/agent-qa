//! Canonical paths used by every agent-qa verb.
//!
//! Every consumer that needs a scenario / replay / compare / recording
//! sub-path goes through this module — recomputing `path::join` inline is
//! how artefacts end up in two places.
//!
//! ## Configuration
//!
//! The scenarios root is overridable via [`SCENARIOS_DIR_ENV`]:
//!
//! - Absolute path → used as-is.
//! - Relative path → resolved against the current working directory.
//! - Unset (default) → `<cwd>/tmp/agent-qa-scenarios`.
//!
//! Resolved fresh on every call so tests / scripts that mutate the env
//! between calls see the change. The cost is irrelevant next to the disk
//! I/O every caller is about to do.
//!
//! The path API lands alongside the typed scenario contract; later
//! slices consume each function as the matching verb is ported.
#![allow(dead_code)]

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
struct ConfigFile {
    #[serde(default)]
    paths: Option<PathsTable>,
    #[serde(default)]
    browser: Option<BrowserConfig>,
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

// ---------- replay tree ----------

pub fn replay_dir(sid: &str, replay_id: &str) -> Result<PathBuf> {
    Ok(scenario_dir(sid)?
        .join("replays")
        .join(safe_segment(replay_id, "replayId")?))
}

pub fn replay_screenshots_dir(sid: &str, replay_id: &str) -> Result<PathBuf> {
    Ok(replay_dir(sid, replay_id)?.join("screenshots"))
}

pub fn replay_snapshots_dir(sid: &str, replay_id: &str) -> Result<PathBuf> {
    Ok(replay_dir(sid, replay_id)?.join("snapshots"))
}

pub fn replay_evidence_dir(sid: &str, replay_id: &str) -> Result<PathBuf> {
    Ok(replay_dir(sid, replay_id)?.join("evidence"))
}

pub fn replay_evidence_channel_dir(sid: &str, replay_id: &str, channel: &str) -> Result<PathBuf> {
    Ok(replay_evidence_dir(sid, replay_id)?.join(safe_segment(channel, "channel")?))
}

// ---------- recording tree (sidecar tree spec) ----------

pub fn recording_dir(sid: &str) -> Result<PathBuf> {
    Ok(scenario_dir(sid)?.join("recording"))
}

pub fn recording_snapshots_dir(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("snapshots"))
}

pub fn recording_screenshots_dir(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("screenshots"))
}

pub fn recording_network_dir(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("network"))
}

pub fn recording_probes_dir(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("probes"))
}

pub fn recording_perf_dir(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("perf"))
}

pub fn recording_failed_dir(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("failed"))
}

pub fn recording_audit_file(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("audit.json"))
}

pub fn recording_heal_jsonl(sid: &str) -> Result<PathBuf> {
    Ok(recording_dir(sid)?.join("heal.jsonl"))
}

// ---------- compare tree ----------

pub fn compare_dir(sid: &str) -> Result<PathBuf> {
    Ok(scenario_dir(sid)?.join("compare"))
}

pub fn compare_run_dir(sid: &str, folder_name: &str) -> Result<PathBuf> {
    Ok(compare_dir(sid)?.join(safe_segment(folder_name, "compare folder")?))
}

pub fn compare_pair_dir(
    sid: &str,
    folder_name: &str,
    base_label: &str,
    cand_label: &str,
) -> Result<PathBuf> {
    Ok(compare_run_dir(sid, folder_name)?
        .join("pairs")
        .join(format!(
            "{}-vs-{}",
            safe_segment(base_label, "base label")?,
            safe_segment(cand_label, "candidate label")?,
        )))
}

// ---------- perf ----------

pub fn perf_dir(sid: &str) -> Result<PathBuf> {
    Ok(scenario_dir(sid)?.join("perf"))
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
/// (sid, replayId, compare folder, etc.). Mirrors the TS regex.
fn safe_segment<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    if value.is_empty() || value == "." || value == ".." || !is_safe(value) {
        bail!("{label} must be a non-empty safe path segment, got {value:?}");
    }
    Ok(value)
}

fn is_safe(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Display-friendly relative path string for log messages — relative to cwd
/// when the scenarios root is inside cwd, absolute otherwise.
pub fn scenarios_rel() -> String {
    let abs = scenarios_root();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match pathdiff(&abs, &cwd) {
        Some(rel) if !rel.starts_with("..") => rel.to_string_lossy().into_owned(),
        _ => abs.to_string_lossy().into_owned(),
    }
}

fn pathdiff(target: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let target = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let mut t_iter = target.components();
    let mut b_iter = base.components();
    loop {
        match (t_iter.clone().next(), b_iter.clone().next()) {
            (Some(t), Some(b)) if t == b => {
                t_iter.next();
                b_iter.next();
            }
            _ => break,
        }
    }
    let mut out = PathBuf::new();
    for c in b_iter {
        if let Component::Normal(_) | Component::ParentDir | Component::CurDir = c {
            out.push("..");
        }
    }
    for c in t_iter {
        out.push(c);
    }
    Some(out)
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
    fn replay_dir_combines() {
        let _g = lock_env();
        env::set_var(SCENARIOS_DIR_ENV, "/tmp/jx");
        let p = replay_dir("sid", "rid").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/jx/sid/replays/rid"));
        env::remove_var(SCENARIOS_DIR_ENV);
    }

    #[test]
    fn replay_dir_rejects_traversal_in_sid() {
        let _g = lock_env();
        env::set_var(SCENARIOS_DIR_ENV, "/tmp/jx");
        replay_dir("../escape", "rid").unwrap_err();
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
