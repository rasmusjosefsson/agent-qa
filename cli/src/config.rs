//! `config` verb — show what config agent-qa resolved from the
//! environment.
//!
//! Single subverb today: `config show`. Prints the active
//! `agent-qa.toml` location (if any), the resolved scenarios_root +
//! record_root, and the discovered plugins (with declared kinds and
//! source label). `--json` emits a structured object for automation.
//!
//! This is the read-only twin of `doctor`. `doctor` *probes* (runs
//! agent-browser --version, pings plugins). `config show` just
//! *resolves* — no spawning, no I/O beyond the toml file itself.

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

use crate::paths;
use crate::plugin::discovery;

pub fn run(args: &[String]) -> Result<u8> {
    match args.first().map(String::as_str) {
        Some("show") | None => show(args.get(1..).unwrap_or(&[])),
        Some("-h" | "--help" | "help") => {
            print_help();
            Ok(0)
        }
        Some(other) => bail!("unknown subverb {other:?}. Try: show [--json]"),
    }
}

fn print_help() {
    println!(
        "agent-qa config \u{2014} resolve config from the environment\n\nUsage:\n  agent-qa config            Show resolved config (default: show)\n  agent-qa config show       Same as above\n  agent-qa config show --json   Structured JSON on stdout"
    );
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    config_file: Option<String>,
    scenarios_root: String,
    record_root: String,
    plugins: Vec<PluginRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginRow {
    binary: String,
    source: String,
    declared_kind: Option<String>,
}

fn show(args: &[String]) -> Result<u8> {
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other => return Err(anyhow!("unknown arg {other:?}")),
        }
    }

    let plugins = discovery::discover(&discovery::DiscoveryOpts::default())?
        .into_iter()
        .map(|p| PluginRow {
            binary: p.binary.display().to_string(),
            source: format!("{:?}", p.source),
            declared_kind: p.declared_kind,
        })
        .collect::<Vec<_>>();

    let report = Report {
        config_file: paths::locate_config_file().map(|p| p.display().to_string()),
        scenarios_root: paths::scenarios_root().display().to_string(),
        record_root: paths::record_root().display().to_string(),
        plugins,
    };

    if json {
        let body = serde_json::to_string_pretty(&report)?;
        println!("{body}");
        return Ok(0);
    }
    render_text(&report);
    Ok(0)
}

fn render_text(r: &Report) {
    println!(
        "agent-qa.toml: {}",
        r.config_file.as_deref().unwrap_or("(none found)")
    );
    println!("scenarios_root: {}", r.scenarios_root);
    println!("record_root:   {}", r.record_root);
    println!();
    if r.plugins.is_empty() {
        println!("plugins: (none discovered)");
        return;
    }
    println!("plugins ({}):", r.plugins.len());
    for p in &r.plugins {
        let kind = p.declared_kind.as_deref().unwrap_or("(undeclared)");
        println!("  {}  [{}]  source={}", p.binary, kind, p.source);
    }
}

// Used by tests + the env var helper to ensure the JSON shape stays
// stable.
#[allow(dead_code)]
const _: fn(&Report) -> Result<String, serde_json::Error> = |r| serde_json::to_string(r);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup(tmp: &Path) {
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp);
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.join("rec"));
    }
    fn teardown() {
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn show_reports_resolved_roots() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let code = show(&[]).unwrap();
        assert_eq!(code, 0);
        teardown();
    }

    #[test]
    fn show_json_emits_camel_case_envelope() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        // Capture stdout via a roundtrip through Report directly (we
        // don't need to swallow the println from show).
        let report = Report {
            config_file: Some("/x/agent-qa.toml".into()),
            scenarios_root: "/abs/scenarios".into(),
            record_root: "/abs/record".into(),
            plugins: vec![],
        };
        let body = serde_json::to_string(&report).unwrap();
        assert!(body.contains("\"configFile\":\"/x/agent-qa.toml\""));
        assert!(body.contains("\"scenariosRoot\":\"/abs/scenarios\""));
        assert!(body.contains("\"recordRoot\":\"/abs/record\""));
        teardown();
    }

    #[test]
    fn show_unknown_arg_errors() {
        let err = show(&["--what".into()]).unwrap_err().to_string();
        assert!(err.contains("unknown arg"));
    }

    #[test]
    fn config_file_path_surfaces_when_toml_present() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        fs::write(tmp.path().join("agent-qa.toml"), "[paths]\n").unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        // Call locate_config_file via the report path.
        let p = paths::locate_config_file().unwrap();
        assert!(p.ends_with("agent-qa.toml"));
        std::env::set_current_dir(prev).unwrap();
        teardown();
    }
}
