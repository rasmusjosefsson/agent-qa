//! `profile-bootstrap` verb — drive a plugin-supplied auth flow for a
//! registered profile.
//!
//! Resolves the registered profile, finds an `auth` plugin via the
//! standard discovery chain, and invokes `<plugin> auth login` with a
//! JSON request carrying the profile id, session name, and (when
//! configured) credentials read from the env vars registered with
//! `profile-add`. The plugin's response status decides exit code:
//!   - `authenticated` → exit 0
//!   - everything else → exit 1 (the plugin's error message is printed)
//!
//! Idempotent at the verb level — plugins implementing `login` are
//! expected to no-op when the session is already authenticated. Any
//! preflight that wants the cheap "already authed?" answer can call
//! `profile-status` first.

use std::env;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::json;

use crate::browser;
use crate::plugin::host;
use crate::profile_status as ps;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(120);

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    // Auth plugins inherit this process environment and use agent-browser for
    // login. Pin the launch mode before invoking the plugin so an ambient
    // AGENT_BROWSER_HEADED value cannot override the headless default.
    browser::set_headed_mode(opts.headed);
    let profile = ps::_load_profile(&opts.profile)?;
    let session = opts
        .session
        .unwrap_or_else(|| format!("{}-session", profile.id));
    let binary = ps::_resolve_auth_plugin(&profile)?;

    let credentials = build_credentials(&profile);
    let request = json!({
        "profile": profile.id,
        "session": session,
        "credentials": credentials,
    });

    let outcome = host::invoke(&binary, "auth", Some("login"), request, LOGIN_TIMEOUT)
        .map_err(|e| anyhow!("auth login via {}: {e}", binary.display()))?;

    let status = outcome
        .response
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("(no status)");
    println!("profile {} → {}", profile.id, status);
    if !opts.quiet {
        let body = serde_json::to_string_pretty(&outcome.response)?;
        println!("{body}");
    }
    Ok(if status == "authenticated" { 0 } else { 1 })
}

fn print_help() {
    println!(
        "agent-qa profile-bootstrap — drive plugin auth login for a profile\n\nUsage:\n  agent-qa profile-bootstrap <profile> [--session <name>] [--headed|--headless] [--quiet]\n\nBrowser mode defaults to headless. --headed shows the browser window; --headless\nexplicitly keeps it hidden. The mode takes effect when agent-browser launches\nthe session.\n\nReads the registered profile from <record_root>/profiles/<profile>/profile.json,\ndiscovers the auth plugin (via the standard chain), and invokes\n'<plugin> auth login' with credentials read from the env vars registered\nvia 'profile-add --email-var / --password-var'."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    profile: String,
    session: Option<String>,
    headed: bool,
    quiet: bool,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut profile: Option<String> = None;
    let mut session: Option<String> = None;
    let mut headed = false;
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
            "--headed" => headed = true,
            "--headless" => headed = false,
            "--quiet" => quiet = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if profile.is_some() {
                    bail!("unexpected positional {other:?}; usage: profile-bootstrap <profile> [flags]");
                }
                profile = Some(other.to_string());
            }
        }
    }
    let profile = profile.ok_or_else(|| anyhow!("usage: profile-bootstrap <profile> [flags]"))?;
    Ok(Opts {
        profile,
        session,
        headed,
        quiet,
    })
}

fn build_credentials(profile: &crate::profile_add::Profile) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if let Some(name) = &profile.email_var {
        if let Ok(v) = env::var(name) {
            map.insert("email".into(), json!(v));
        }
    }
    if let Some(name) = &profile.password_var {
        if let Ok(v) = env::var(name) {
            map.insert("password".into(), json!(v));
        }
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_add::Profile;
    use crate::test_util::lock_env;

    #[test]
    fn build_credentials_reads_env_vars_when_set() {
        let _g = lock_env();
        env::set_var("BS_TEST_EMAIL", "alice@example.com");
        env::set_var("BS_TEST_PW", "s3cret");
        let p = Profile {
            id: "alice".into(),
            adapter: None,
            email_var: Some("BS_TEST_EMAIL".into()),
            password_var: Some("BS_TEST_PW".into()),
            default: None,
            registered_at: "now".into(),
        };
        let creds = build_credentials(&p);
        assert_eq!(creds["email"], "alice@example.com");
        assert_eq!(creds["password"], "s3cret");
        env::remove_var("BS_TEST_EMAIL");
        env::remove_var("BS_TEST_PW");
    }

    #[test]
    fn build_credentials_omits_unset_vars() {
        let _g = lock_env();
        env::remove_var("BS_TEST_EMPTY");
        let p = Profile {
            id: "x".into(),
            adapter: None,
            email_var: Some("BS_TEST_EMPTY".into()),
            password_var: None,
            default: None,
            registered_at: "now".into(),
        };
        let creds = build_credentials(&p);
        assert!(creds.as_object().unwrap().is_empty());
    }

    #[test]
    fn parse_args_requires_profile() {
        parse_args(&["--quiet".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_accepts_session_mode_and_quiet() {
        let opts = parse_args(&[
            "alice".into(),
            "--session=sx".into(),
            "--headed".into(),
            "--quiet".into(),
        ])
        .unwrap();
        assert_eq!(opts.profile, "alice");
        assert_eq!(opts.session.as_deref(), Some("sx"));
        assert!(opts.headed);
        assert!(opts.quiet);
    }

    #[test]
    fn parse_args_defaults_headless_and_last_mode_flag_wins() {
        let default = parse_args(&["alice".into()]).unwrap();
        assert!(!default.headed);

        let overridden =
            parse_args(&["alice".into(), "--headed".into(), "--headless".into()]).unwrap();
        assert!(!overridden.headed);
    }
}
