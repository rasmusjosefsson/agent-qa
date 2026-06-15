//! Plugin discovery.
//!
//! Priority order (first match wins per binary):
//!   1. `--plugin <path>` CLI flag (forwarded into `PluginConfig::overrides`)
//!   2. `agent-qa.toml` walked from cwd up to root  →  `[plugins]` table
//!      (per-repo).
//!   3. Global user-level config:
//!      - `~/.agent-qa/agent-qa.toml`                  (primary)
//!      - `$XDG_CONFIG_HOME/agent-qa/agent-qa.toml`    (XDG fallback)
//!
//!      Same `[plugins]` schema as per-repo; repo wins per-binary on dedupe.
//!   4. `AGENT_QA_PLUGINS` env var — colon-separated list of binary paths.
//!   5. `$PATH` — any executable named `agent-qa-plugin-*` (gh extension
//!      convention).
//!
//! Each discovered plugin is then `ping`ed so we know which kinds it
//! actually serves. Discovery is lazy w.r.t. the ping (caller pings as
//! needed) but eager about the file-system enumeration.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Where a plugin was found. Surfaced by `plugins list` for transparency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoverySource {
    CliFlag,
    ConfigFile(PathBuf),
    EnvVar,
    Path,
}

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub binary: PathBuf,
    pub source: DiscoverySource,
    /// If discovered from `agent-qa.toml`, this is the kind that mapped to
    /// the binary. `None` means the kinds are not yet known and must be
    /// learned via `ping`.
    pub declared_kind: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct DiscoveryOpts {
    /// `--plugin <path>` overrides; absolute or relative-to-cwd paths.
    pub cli_overrides: Vec<PathBuf>,
    /// Optional cwd override for testing.
    pub cwd: Option<PathBuf>,
}

/// Run discovery. Returns the plugins in priority order; duplicates (same
/// canonical binary path) are de-duplicated keeping the highest-priority
/// entry.
pub fn discover(opts: &DiscoveryOpts) -> Result<Vec<DiscoveredPlugin>> {
    let cwd = match &opts.cwd {
        Some(p) => p.clone(),
        None => env::current_dir().context("get cwd")?,
    };

    let mut out: Vec<DiscoveredPlugin> = Vec::new();
    let mut seen: BTreeMap<PathBuf, ()> = BTreeMap::new();

    let push =
        |p: DiscoveredPlugin, out: &mut Vec<DiscoveredPlugin>, seen: &mut BTreeMap<PathBuf, ()>| {
            let key = p.binary.canonicalize().unwrap_or_else(|_| p.binary.clone());
            if seen.insert(key, ()).is_none() {
                out.push(p);
            }
        };

    // 1. CLI overrides.
    for path in &opts.cli_overrides {
        let resolved = if path.is_absolute() {
            path.clone()
        } else {
            cwd.join(path)
        };
        push(
            DiscoveredPlugin {
                binary: resolved,
                source: DiscoverySource::CliFlag,
                declared_kind: None,
            },
            &mut out,
            &mut seen,
        );
    }

    // 2. agent-qa.toml walking up from cwd (per-repo).
    if let Some((toml_path, cfg)) = load_config_walking_up(&cwd)? {
        for (kind, binary_spec) in cfg.plugins.unwrap_or_default() {
            let resolved = resolve_plugin_spec(&binary_spec, &toml_path)?;
            push(
                DiscoveredPlugin {
                    binary: resolved,
                    source: DiscoverySource::ConfigFile(toml_path.clone()),
                    declared_kind: Some(kind),
                },
                &mut out,
                &mut seen,
            );
        }
    }

    // 2b. Global user-level config(s) — ~/.agent-qa/agent-qa.toml and the
    //     XDG fallback. Pushed *after* the per-repo config so the repo
    //     wins on dedupe (same binary path); for distinct binaries serving
    //     the same kind, `plugins path <kind>` still resolves to the
    //     higher-priority entry (repo over global).
    for global_path in crate::global_config::existing_global_config_files() {
        let bytes = match fs::read_to_string(&global_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let cfg: ConfigFile = match toml::from_str(&bytes) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (kind, binary_spec) in cfg.plugins.unwrap_or_default() {
            let resolved = resolve_plugin_spec(&binary_spec, &global_path)?;
            push(
                DiscoveredPlugin {
                    binary: resolved,
                    source: DiscoverySource::ConfigFile(global_path.clone()),
                    declared_kind: Some(kind),
                },
                &mut out,
                &mut seen,
            );
        }
    }

    // 3. AGENT_QA_PLUGINS env var.
    if let Ok(val) = env::var("AGENT_QA_PLUGINS") {
        for entry in val.split(':').filter(|s| !s.is_empty()) {
            let path = PathBuf::from(entry);
            let resolved = if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            };
            push(
                DiscoveredPlugin {
                    binary: resolved,
                    source: DiscoverySource::EnvVar,
                    declared_kind: None,
                },
                &mut out,
                &mut seen,
            );
        }
    }

    // 4. $PATH for executables named agent-qa-plugin-*.
    for entry in path_scan_for_plugin_prefix() {
        push(
            DiscoveredPlugin {
                binary: entry,
                source: DiscoverySource::Path,
                declared_kind: None,
            },
            &mut out,
            &mut seen,
        );
    }

    Ok(out)
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    plugins: Option<BTreeMap<String, String>>,
}

fn load_config_walking_up(start: &Path) -> Result<Option<(PathBuf, ConfigFile)>> {
    let mut cur: Option<&Path> = Some(start);
    while let Some(dir) = cur {
        for name in ["agent-qa.toml", ".agent-qa.toml"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                let bytes = fs::read_to_string(&candidate)
                    .with_context(|| format!("read {}", candidate.display()))?;
                let parsed: ConfigFile = toml::from_str(&bytes)
                    .with_context(|| format!("parse {}", candidate.display()))?;
                return Ok(Some((candidate, parsed)));
            }
        }
        cur = dir.parent();
    }
    Ok(None)
}

/// A plugin spec in `agent-qa.toml` may be either:
///   - an absolute or relative path (relative to the toml file's directory)
///   - a bare name resolved via `$PATH`
fn resolve_plugin_spec(spec: &str, toml_path: &Path) -> Result<PathBuf> {
    // Expand ~ / ~/foo against $HOME (or %USERPROFILE% on Windows) so
    // global configs can use the conventional ~/.agent-qa/plugins/...
    // form without hardcoding an absolute path per machine.
    let expanded = crate::global_config::expand_tilde(spec);
    if expanded.is_absolute() {
        return Ok(expanded);
    }
    // Treat anything containing a path separator as a relative path.
    if spec.contains('/') || spec.contains('\\') {
        let base = toml_path.parent().unwrap_or_else(|| Path::new("."));
        return Ok(base.join(expanded));
    }
    // Otherwise resolve via $PATH.
    which::which(spec).with_context(|| format!("resolve plugin `{spec}` on $PATH"))
}

/// Enumerate `$PATH` for executables whose filename starts with
/// `agent-qa-plugin-`. Returns absolute paths, deduplicated, in PATH order.
fn path_scan_for_plugin_prefix() -> Vec<PathBuf> {
    let path = match env::var_os("PATH") {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    for dir in env::split_paths(&path) {
        let entries = match fs::read_dir(&dir) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if !s.starts_with("agent-qa-plugin-") {
                continue;
            }
            // Skip extensions like .ps1 / .cmd for simplicity; real check is
            // executable bit on unix, which would require a syscall.
            if seen.insert(s.to_string(), ()).is_some() {
                continue;
            }
            out.push(entry.path());
        }
    }
    out
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    #[test]
    fn cli_override_is_first() {
        let tmp = TempDir::new().unwrap();
        let bin = write_exec(tmp.path(), "fake-plugin", "#!/bin/sh\necho hi\n");
        let opts = DiscoveryOpts {
            cli_overrides: vec![bin.clone()],
            cwd: Some(tmp.path().to_path_buf()),
        };
        let got = discover(&opts).unwrap();
        assert!(!got.is_empty());
        assert_eq!(got[0].binary, bin);
        assert_eq!(got[0].source, DiscoverySource::CliFlag);
    }

    #[test]
    fn agent_qa_toml_discovered_with_declared_kind() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(tmp.path(), "my-auth", "#!/bin/sh\necho hi\n");
        let toml = format!("[plugins]\nauth = \"{}\"\n", plugin.display());
        fs::write(tmp.path().join("agent-qa.toml"), toml).unwrap();

        let opts = DiscoveryOpts {
            cli_overrides: vec![],
            cwd: Some(tmp.path().to_path_buf()),
        };
        let got = discover(&opts).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].declared_kind.as_deref(), Some("auth"));
        match &got[0].source {
            DiscoverySource::ConfigFile(p) => assert!(p.ends_with("agent-qa.toml")),
            other => panic!("expected ConfigFile, got {other:?}"),
        }
    }

    #[test]
    fn duplicates_are_deduplicated_keeping_highest_priority() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(tmp.path(), "my-auth", "#!/bin/sh\necho hi\n");
        let toml = format!("[plugins]\nauth = \"{}\"\n", plugin.display());
        fs::write(tmp.path().join("agent-qa.toml"), toml).unwrap();

        let opts = DiscoveryOpts {
            cli_overrides: vec![plugin.clone()], // also listed as CLI override
            cwd: Some(tmp.path().to_path_buf()),
        };
        let got = discover(&opts).unwrap();
        assert_eq!(got.len(), 1, "duplicate binary should appear once");
        assert_eq!(
            got[0].source,
            DiscoverySource::CliFlag,
            "higher priority wins"
        );
    }
}
