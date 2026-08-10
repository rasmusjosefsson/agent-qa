//! Global (user-level) config home for agent-qa.
//!
//! Follows the common per-tool user-config directory convention. Looked up
//! *in addition to* the per-repo
//! `agent-qa.toml` walked up from cwd, so a single global file can hold
//! defaults (plugins, skill-data dirs, future session vault) for every
//! invocation regardless of which repo you're in.
//!
//! Search order — first existing file wins:
//!
//!   1. `~/.agent-qa/agent-qa.toml`                 (primary)
//!   2. `$XDG_CONFIG_HOME/agent-qa/agent-qa.toml`   (XDG fallback;
//!      default `~/.config/agent-qa/agent-qa.toml`)
//!
//! Both surfaces (`skills` extra-dirs, `plugins` table) consult this
//! module and merge its contents with whatever the per-repo cwd walk
//! finds. Merge rules are surface-specific: skills unions extra-dirs,
//! plugins lets the repo override the global on a per-kind basis.

use std::env;
use std::path::PathBuf;

/// Ordered list of global config files to try. Filters to existing
/// regular files; returns an empty list when none are present.
pub fn existing_global_config_files() -> Vec<PathBuf> {
    candidate_global_config_files()
        .into_iter()
        .filter(|p| p.is_file())
        .collect()
}

/// Resolve the user home directory across platforms. Prefers `$HOME`
/// (always set on Unix and respected by msys/cygwin/git-bash on
/// Windows) and falls back to `%USERPROFILE%` on native Windows.
pub fn user_home() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
}

/// Shell-style tilde expansion for config-supplied paths. Recognises:
///   - `~`            → `$HOME`
///   - `~/foo`        → `$HOME/foo`
///
/// Does **not** support `~user` (would require passwd lookup). Returns
/// the input unchanged if it doesn't start with `~` or if `$HOME` is
/// unresolvable.
pub fn expand_tilde(spec: &str) -> PathBuf {
    if spec == "~" {
        return user_home().unwrap_or_else(|| PathBuf::from(spec));
    }
    if let Some(rest) = spec.strip_prefix("~/") {
        if let Some(home) = user_home() {
            return home.join(rest);
        }
    }
    // `~user` or any non-tilde-prefixed string: passthrough.
    PathBuf::from(spec)
}

/// All candidate locations regardless of whether they exist. Exposed
/// for diagnostics (`config show`) and tests.
pub fn candidate_global_config_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let home = user_home();

    // 1. ~/.agent-qa/agent-qa.toml (primary)
    if let Some(h) = &home {
        out.push(h.join(".agent-qa").join("agent-qa.toml"));
    }

    // 2. $XDG_CONFIG_HOME/agent-qa/agent-qa.toml (fallback)
    let xdg = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".config")));
    if let Some(base) = xdg {
        out.push(base.join("agent-qa").join("agent-qa.toml"));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;

    #[test]
    fn candidates_include_dot_agent_qa_first() {
        let _g = lock_env();
        env::set_var("HOME", "/h");
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("USERPROFILE");
        let got = candidate_global_config_files();
        assert_eq!(
            got[0],
            PathBuf::from("/h").join(".agent-qa").join("agent-qa.toml")
        );
        assert_eq!(
            got[1],
            PathBuf::from("/h")
                .join(".config")
                .join("agent-qa")
                .join("agent-qa.toml")
        );
    }

    #[test]
    fn xdg_overrides_default_config_home() {
        let _g = lock_env();
        env::set_var("HOME", "/h");
        env::set_var("XDG_CONFIG_HOME", "/xdg");
        env::remove_var("USERPROFILE");
        let got = candidate_global_config_files();
        assert_eq!(
            got[1],
            PathBuf::from("/xdg").join("agent-qa").join("agent-qa.toml")
        );
    }

    #[test]
    fn no_home_no_candidates() {
        let _g = lock_env();
        env::remove_var("HOME");
        env::remove_var("USERPROFILE");
        env::remove_var("XDG_CONFIG_HOME");
        assert!(candidate_global_config_files().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn userprofile_is_used_when_home_unset() {
        let _g = lock_env();
        env::remove_var("HOME");
        env::remove_var("XDG_CONFIG_HOME");
        env::set_var("USERPROFILE", "C:\\Users\\test");
        let got = candidate_global_config_files();
        assert!(
            got.iter()
                .any(|p| p.ends_with(PathBuf::from(".agent-qa").join("agent-qa.toml"))),
            "{got:?}"
        );
    }

    #[test]
    fn expand_tilde_bare() {
        let _g = lock_env();
        env::set_var("HOME", "/h");
        env::remove_var("USERPROFILE");
        assert_eq!(expand_tilde("~"), PathBuf::from("/h"));
    }

    #[test]
    fn expand_tilde_slash_prefix() {
        let _g = lock_env();
        env::set_var("HOME", "/h");
        env::remove_var("USERPROFILE");
        assert_eq!(
            expand_tilde("~/.agent-qa/skills"),
            PathBuf::from("/h").join(".agent-qa").join("skills")
        );
    }

    #[test]
    fn expand_tilde_user_form_is_passthrough() {
        let _g = lock_env();
        env::set_var("HOME", "/h");
        env::remove_var("USERPROFILE");
        assert_eq!(expand_tilde("~bob/foo"), PathBuf::from("~bob/foo"));
    }

    #[test]
    fn expand_tilde_passthrough_when_no_tilde() {
        let _g = lock_env();
        env::set_var("HOME", "/h");
        env::remove_var("USERPROFILE");
        assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
        assert_eq!(expand_tilde("rel/path"), PathBuf::from("rel/path"));
    }

    #[test]
    fn expand_tilde_without_home_is_passthrough() {
        let _g = lock_env();
        env::remove_var("HOME");
        env::remove_var("USERPROFILE");
        assert_eq!(expand_tilde("~/x"), PathBuf::from("~/x"));
    }
}
