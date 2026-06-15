//! `profile-add` verb — register a per-identity profile.
//!
//! Lightweight registration only: no auth bootstrap (that's a plugin
//! concern, lands when the plugin protocol grows the `auth` kind handler).
//! This module establishes the on-disk shape; later work wires the
//! plugin-driven bootstrap + status probe.
//!
//! Effect:
//!   mkdir <record_root>/profiles/<id>/
//!   write <record_root>/profiles/<id>/profile.json with the metadata
//!   passed on the CLI; merges over an existing file (idempotent).

use std::fs;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;
use crate::sidecar::atomic_write_file;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let summary = add(&opts)?;
    println!("registered profile {}", summary.id);
    println!("file: {}", summary.profile_file.display());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa profile-add — register a per-identity profile\n\nUsage:\n  agent-qa profile-add <id> [--adapter <plugin-id>] [--email-var <NAME>]\n                            [--password-var <NAME>] [--default]\n\nEffect:\n  mkdir <record_root>/profiles/<id>/\n  write <record_root>/profiles/<id>/profile.json with the metadata\n  passed on the CLI; merges over an existing file (idempotent).\n  Auth bootstrap is a plugin concern; this verb only does registration."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    id: String,
    adapter: Option<String>,
    email_var: Option<String>,
    password_var: Option<String>,
    default: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_var: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_var: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
    pub registered_at: String,
}

#[derive(Debug)]
struct Summary {
    id: String,
    profile_file: std::path::PathBuf,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut id: Option<String> = None;
    let mut adapter: Option<String> = None;
    let mut email_var: Option<String> = None;
    let mut password_var: Option<String> = None;
    let mut default = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--adapter" => adapter = it.next().cloned(),
            s if s.starts_with("--adapter=") => adapter = Some(s["--adapter=".len()..].to_string()),
            "--email-var" => email_var = it.next().cloned(),
            s if s.starts_with("--email-var=") => {
                email_var = Some(s["--email-var=".len()..].to_string())
            }
            "--password-var" => password_var = it.next().cloned(),
            s if s.starts_with("--password-var=") => {
                password_var = Some(s["--password-var=".len()..].to_string())
            }
            "--default" => default = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if id.is_some() {
                    bail!("unexpected positional {other:?}; usage: profile-add <id> [flags]");
                }
                id = Some(other.to_string());
            }
        }
    }
    let id = id.ok_or_else(|| anyhow!("usage: profile-add <id> [flags]"))?;
    Ok(Opts {
        id,
        adapter,
        email_var,
        password_var,
        default,
    })
}

fn add(opts: &Opts) -> Result<Summary> {
    let dir = paths::profile_dir(&opts.id)?;
    fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    let path = paths::profile_file(&opts.id)?;

    // Idempotent merge: read any existing profile.json and merge new
    // values over the top so re-registering doesn't drop fields the
    // user set on a previous call.
    let existing: Option<Profile> = fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let registered_at = existing
        .as_ref()
        .map(|p| p.registered_at.clone())
        .unwrap_or_else(|| {
            chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string()
        });

    let merged = Profile {
        id: opts.id.clone(),
        adapter: opts
            .adapter
            .clone()
            .or_else(|| existing.as_ref().and_then(|p| p.adapter.clone())),
        email_var: opts
            .email_var
            .clone()
            .or_else(|| existing.as_ref().and_then(|p| p.email_var.clone())),
        password_var: opts
            .password_var
            .clone()
            .or_else(|| existing.as_ref().and_then(|p| p.password_var.clone())),
        default: if opts.default {
            Some(true)
        } else {
            existing.as_ref().and_then(|p| p.default)
        },
        registered_at,
    };

    let body = serde_json::to_string_pretty(&merged)?;
    let mut bytes = body.into_bytes();
    bytes.push(b'\n');
    atomic_write_file(&path, &bytes)?;
    Ok(Summary {
        id: opts.id.clone(),
        profile_file: path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.join("rec"));
    }
    fn teardown() {
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn add_writes_profile_json() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let s = add(&Opts {
            id: "alice".into(),
            adapter: Some("vendor-auth".into()),
            email_var: Some("ALICE_EMAIL".into()),
            password_var: Some("ALICE_PW".into()),
            default: true,
        })
        .unwrap();
        let body = fs::read_to_string(&s.profile_file).unwrap();
        let p: Profile = serde_json::from_str(&body).unwrap();
        assert_eq!(p.id, "alice");
        assert_eq!(p.adapter.as_deref(), Some("vendor-auth"));
        assert_eq!(p.email_var.as_deref(), Some("ALICE_EMAIL"));
        assert_eq!(p.default, Some(true));
        teardown();
    }

    #[test]
    fn add_is_idempotent_and_merges() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        // First call sets adapter only.
        add(&Opts {
            id: "alice".into(),
            adapter: Some("vendor-auth".into()),
            email_var: None,
            password_var: None,
            default: false,
        })
        .unwrap();
        // Second call adds email_var only — adapter must survive.
        let s = add(&Opts {
            id: "alice".into(),
            adapter: None,
            email_var: Some("ALICE_EMAIL".into()),
            password_var: None,
            default: false,
        })
        .unwrap();
        let p: Profile = serde_json::from_slice(&fs::read(&s.profile_file).unwrap()).unwrap();
        assert_eq!(p.adapter.as_deref(), Some("vendor-auth"));
        assert_eq!(p.email_var.as_deref(), Some("ALICE_EMAIL"));
        teardown();
    }

    #[test]
    fn add_rejects_unsafe_id() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        add(&Opts {
            id: "../escape".into(),
            adapter: None,
            email_var: None,
            password_var: None,
            default: false,
        })
        .unwrap_err();
        teardown();
    }

    #[test]
    fn parse_args_requires_id() {
        parse_args(&["--adapter".into(), "x".into()]).unwrap_err();
    }
}
