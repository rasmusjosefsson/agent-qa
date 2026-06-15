//! `profile-status` verb — probe a registered profile via the plugin
//! protocol's `auth` kind.
//!
//! Resolves the registered profile from disk, finds a plugin that
//! serves the `auth` kind (via the standard discovery chain), invokes
//! it with `<plugin> auth probe` and a JSON request carrying
//! `{ profile, session }`. Prints the response.
//!
//! The plugin's response shape is documented in
//! [`docs/plugins.md`](../../docs/plugins.md). For `auth probe` we
//! consume one well-known field — `status` ∈ {`authenticated`,
//! `expired`, `missing`, `on-login`} — and pass the rest through
//! verbatim.

use std::fs;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::json;

use crate::paths;
use crate::plugin::{self, discovery, host};
use crate::profile_add::Profile;

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let profile = load_profile(&opts.profile)?;
    let session = opts
        .session
        .unwrap_or_else(|| format!("{}-session", profile.id));

    let binary = resolve_auth_plugin(&profile)?;
    let request = json!({ "profile": profile.id, "session": session });
    let outcome = host::invoke(&binary, "auth", Some("probe"), request, PROBE_TIMEOUT)
        .map_err(|e| anyhow!("auth probe via {}: {e}", binary.display()))?;

    let status = outcome
        .response
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("(no status)")
        .to_string();
    println!("profile {} → {}", profile.id, status);
    if !opts.quiet {
        let body = serde_json::to_string_pretty(&outcome.response)?;
        println!("{body}");
    }
    Ok(if status == "authenticated" { 0 } else { 1 })
}

fn print_help() {
    println!(
        "agent-qa profile-status — probe a registered profile via plugin auth\n\nUsage:\n  agent-qa profile-status <profile> [--session <name>] [--quiet]\n\nResolves <record_root>/profiles/<profile>/profile.json, discovers a\nplugin that serves the 'auth' kind (via --plugin / agent-qa.toml /\nAGENT_QA_PLUGINS / $PATH), invokes 'auth probe' with a JSON request\ncarrying profile + session, and prints the response.\n\nExits 0 when the response status is 'authenticated', 1 otherwise."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    profile: String,
    session: Option<String>,
    quiet: bool,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut profile: Option<String> = None;
    let mut session: Option<String> = None;
    let mut quiet = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            "--quiet" => quiet = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if profile.is_some() {
                    bail!(
                        "unexpected positional {other:?}; usage: profile-status <profile> [flags]"
                    );
                }
                profile = Some(other.to_string());
            }
        }
    }
    let profile = profile.ok_or_else(|| anyhow!("usage: profile-status <profile> [flags]"))?;
    Ok(Opts {
        profile,
        session,
        quiet,
    })
}

fn load_profile(id: &str) -> Result<Profile> {
    let path = paths::profile_file(id)?;
    let body = fs::read(&path).map_err(|_| {
        anyhow!(
            "no profile registered at {} — run `agent-qa profile-add {id}` first",
            path.display()
        )
    })?;
    let p: Profile =
        serde_json::from_slice(&body).map_err(|e| anyhow!("parse {}: {e}", path.display()))?;
    Ok(p)
}

fn resolve_auth_plugin(profile: &Profile) -> Result<std::path::PathBuf> {
    let plugins = discovery::discover(&discovery::DiscoveryOpts::default())?;
    if plugins.is_empty() {
        bail!("no plugins discovered; install an auth plugin or pass --plugin <path>");
    }
    // First: declared-kind match in agent-qa.toml.
    for p in &plugins {
        if p.declared_kind.as_deref() == Some("auth") {
            return Ok(p.binary.clone());
        }
    }
    // Then: ping every plugin to learn kinds. Prefer plugins whose
    // `name` field matches `profile.adapter` if specified.
    for p in &plugins {
        if let Ok(pong) = host::ping(&p.binary) {
            if !pong.kinds.iter().any(|k| k == "auth") {
                continue;
            }
            if let Some(adapter) = &profile.adapter {
                if pong.name == *adapter {
                    return Ok(p.binary.clone());
                }
            }
        }
    }
    // Otherwise: first plugin that pings as serving auth.
    for p in &plugins {
        if let Ok(pong) = host::ping(&p.binary) {
            if pong.kinds.iter().any(|k| k == "auth") {
                return Ok(p.binary.clone());
            }
        }
    }
    bail!(
        "no plugin serves the 'auth' kind (checked {} plugin(s))",
        plugins.len()
    )
}

// Re-export for the parallel `profile-bootstrap` verb (later PR).
#[allow(dead_code)]
pub(crate) fn _resolve_auth_plugin(profile: &Profile) -> Result<std::path::PathBuf> {
    resolve_auth_plugin(profile)
}

#[allow(dead_code)]
pub(crate) fn _load_profile(id: &str) -> Result<Profile> {
    load_profile(id)
}

#[allow(dead_code)]
pub(crate) const _PROBE_TIMEOUT: Duration = PROBE_TIMEOUT;

// Force the `plugin::PROTOCOL_VERSION` constant alive (slot for future
// version-negotiation logic; lints would otherwise flag it).
#[allow(dead_code)]
const _PROTOCOL_VERSION: u32 = plugin::protocol::PROTOCOL_VERSION;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.join("rec"));
    }
    fn teardown() {
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    fn write_exec(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn write_authed_plugin(dir: &Path) -> std::path::PathBuf {
        write_exec(
            dir,
            "agent-qa-plugin-noop-auth",
            "#!/bin/sh\nif [ \"$1\" = ping ]; then\n  echo '{\"ok\":true,\"response\":{\"protocolVersion\":1,\"name\":\"noop\",\"kinds\":[\"auth\"]}}'\n  exit 0\nfi\nif [ \"$1\" = auth ] && [ \"$2\" = probe ]; then\n  cat >/dev/null\n  echo '{\"ok\":true,\"response\":{\"status\":\"authenticated\",\"url\":\"https://example.com/\"}}'\n  exit 0\nfi\nexit 0\n",
        )
    }

    #[test]
    fn resolve_auth_plugin_via_path_discovery() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());

        // Stage the plugin on $PATH via a custom dir.
        let plugin = write_authed_plugin(tmp.path());
        let original_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            format!("{}:{}", tmp.path().display(), original_path),
        );

        // Plain registered profile, no declared adapter.
        let profile = Profile {
            id: "alice".into(),
            adapter: None,
            email_var: None,
            password_var: None,
            default: None,
            registered_at: "now".into(),
        };
        let resolved = resolve_auth_plugin(&profile).unwrap();
        assert_eq!(resolved, plugin);

        std::env::set_var("PATH", original_path);
        teardown();
    }

    #[test]
    fn resolve_auth_plugin_errors_when_none_serve_auth() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        // Plugin that does NOT advertise auth.
        write_exec(
            tmp.path(),
            "agent-qa-plugin-other",
            "#!/bin/sh\nif [ \"$1\" = ping ]; then\n  echo '{\"ok\":true,\"response\":{\"protocolVersion\":1,\"name\":\"x\",\"kinds\":[\"setup-hook\"]}}'\n  exit 0\nfi\nexit 0\n",
        );
        let original_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            format!("{}:{}", tmp.path().display(), original_path),
        );

        let profile = Profile {
            id: "x".into(),
            adapter: None,
            email_var: None,
            password_var: None,
            default: None,
            registered_at: "now".into(),
        };
        let err = resolve_auth_plugin(&profile).unwrap_err().to_string();
        assert!(err.contains("no plugin serves"), "got: {err}");

        std::env::set_var("PATH", original_path);
        teardown();
    }

    #[test]
    fn parse_args_requires_profile() {
        parse_args(&["--quiet".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_session_and_quiet() {
        let opts = parse_args(&[
            "alice".into(),
            "--session".into(),
            "sx".into(),
            "--quiet".into(),
        ])
        .unwrap();
        assert_eq!(opts.profile, "alice");
        assert_eq!(opts.session.as_deref(), Some("sx"));
        assert!(opts.quiet);
    }

    #[test]
    fn load_profile_errors_when_unregistered() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let err = load_profile("nope").unwrap_err().to_string();
        assert!(err.contains("profile-add"), "got: {err}");
        teardown();
    }
}
