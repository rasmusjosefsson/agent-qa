//! `profile-list` verb — enumerate registered profiles.
//!
//! Reads every `<profiles_root>/<id>/profile.json` and prints a per-row
//! summary. Useful for diagnostics and shell completion.
//!
//! Schema overrides via env / `agent-qa.toml [paths]` are honoured by
//! the upstream `paths::profiles_root()` helper.

use std::fs;
use std::io::Write;

use anyhow::{bail, Result};
use serde::Serialize;

use crate::paths;
use crate::profile_add::Profile;

pub fn run(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    for a in args {
        match a.as_str() {
            "--json" => json_out = true,
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            other => bail!("unknown flag {other:?}"),
        }
    }
    let report = collect()?;
    if json_out {
        let body = serde_json::to_string_pretty(&report)?;
        std::io::stdout().write_all(body.as_bytes())?;
        std::io::stdout().write_all(b"\n")?;
    } else {
        render_text(&report);
    }
    Ok(0)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    profiles_root: String,
    profiles: Vec<Row>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: String,
    dir: String,
    adapter: Option<String>,
    default: bool,
    registered_at: Option<String>,
}

fn collect() -> Result<Report> {
    let root = paths::profiles_root();
    let mut rows: Vec<Row> = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let pf = dir.join("profile.json");
            let id = dir
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let parsed: Option<Profile> = fs::read(&pf)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            rows.push(Row {
                id,
                dir: dir.display().to_string(),
                adapter: parsed.as_ref().and_then(|p| p.adapter.clone()),
                default: parsed.as_ref().and_then(|p| p.default).unwrap_or(false),
                registered_at: parsed.as_ref().map(|p| p.registered_at.clone()),
            });
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Report {
        profiles_root: root.display().to_string(),
        profiles: rows,
    })
}

fn render_text(report: &Report) {
    println!("profiles root: {}", report.profiles_root);
    if report.profiles.is_empty() {
        println!("(no profiles registered \u{2014} use `agent-qa profile-add`)");
        return;
    }
    let id_h = "id";
    let adapter_h = "adapter";
    let default_h = "default";
    let registered_h = "registeredAt";
    println!("{id_h:<20}  {adapter_h:<14}  {default_h:<8}  {registered_h}");
    for row in &report.profiles {
        let adapter = row.adapter.as_deref().unwrap_or("-");
        let default = if row.default { "yes" } else { "-" };
        let registered = row.registered_at.as_deref().unwrap_or("-");
        let id = &row.id;
        println!("{id:<20}  {adapter:<14}  {default:<8}  {registered}");
    }
}

fn print_help() {
    println!(
        "agent-qa profile-list \u{2014} enumerate registered profiles\n\nUsage:\n  agent-qa profile-list                      Table view\n  agent-qa profile-list --json               Structured JSON on stdout"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_profile(root: &std::path::Path, id: &str, adapter: Option<&str>, default: bool) {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let mut body = serde_json::json!({
            "id": id,
            "registeredAt": "2026-01-01T00:00:00.000Z",
        });
        if let Some(a) = adapter {
            body["adapter"] = serde_json::Value::String(a.into());
        }
        if default {
            body["default"] = serde_json::Value::Bool(true);
        }
        std::fs::write(dir.join("profile.json"), body.to_string()).unwrap();
    }

    #[test]
    fn collect_lists_all_profiles_sorted() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_RECORD_DIR").ok();
        std::env::set_var("AGENT_QA_RECORD_DIR", tmp.path());
        let root = crate::paths::profiles_root();
        std::fs::create_dir_all(&root).unwrap();
        write_profile(&root, "zeta", Some("z-adapter"), true);
        write_profile(&root, "alpha", Some("a-adapter"), false);
        let report = collect().unwrap();
        let ids: Vec<&str> = report.profiles.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "zeta"]);
        let zeta = report.profiles.iter().find(|r| r.id == "zeta").unwrap();
        assert!(zeta.default);
        assert_eq!(zeta.adapter.as_deref(), Some("z-adapter"));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_RECORD_DIR", v),
            None => std::env::remove_var("AGENT_QA_RECORD_DIR"),
        }
    }

    #[test]
    fn collect_tolerates_missing_root() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_RECORD_DIR").ok();
        std::env::set_var("AGENT_QA_RECORD_DIR", tmp.path().join("does-not-exist"));
        let report = collect().unwrap();
        assert!(report.profiles.is_empty());
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_RECORD_DIR", v),
            None => std::env::remove_var("AGENT_QA_RECORD_DIR"),
        }
    }
}
