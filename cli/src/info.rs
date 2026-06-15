//! `info` verb — top-level status one-liner: version, paths, counts.
//!
//! Different from `doctor`: doctor probes the install (agent-browser,
//! plugins, run-paths). `info` just reports what we know without any
//! external probes. Useful for shell prompts / dashboards.

use std::fs;

use anyhow::{bail, Result};

pub fn run(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    let mut check = false;
    for a in args {
        match a.as_str() {
            "--json" => json_out = true,
            "--check" => check = true,
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa info \u{2014} version + paths + scenario/plugin counts.\n\nUsage:\n  agent-qa info               Human-readable summary\n  agent-qa info --json        Structured JSON on stdout\n  agent-qa info --check       Exit non-zero if scenarios/profiles roots can't be created/written"
                );
                return Ok(0);
            }
            other => bail!("unknown flag {other:?}"),
        }
    }

    let version = env!("CARGO_PKG_VERSION");
    let scenarios_root = crate::paths::scenarios_root();
    let profiles_root = crate::paths::profiles_root();
    let config_file = crate::paths::locate_config_file().map(|p| p.display().to_string());
    let scenarios = fs::read_dir(&scenarios_root)
        .map(|it| {
            it.flatten()
                .filter(|e| e.path().join("scenario.json").is_file())
                .count()
        })
        .unwrap_or(0);
    let profiles = fs::read_dir(&profiles_root)
        .map(|it| {
            it.flatten()
                .filter(|e| e.path().join("profile.json").is_file())
                .count()
        })
        .unwrap_or(0);

    if json_out {
        let body = serde_json::json!({
            "name": "agent-qa",
            "version": version,
            "scenariosRoot": scenarios_root.display().to_string(),
            "profilesRoot": profiles_root.display().to_string(),
            "configFile": config_file,
            "scenarios": scenarios,
            "profiles": profiles,
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        println!("agent-qa {version}");
        println!("  scenarios root : {}", scenarios_root.display());
        println!("  scenarios      : {scenarios}");
        println!("  profiles root : {}", profiles_root.display());
        println!("  profiles      : {profiles}");
        if let Some(cfg) = &config_file {
            println!("  config file   : {cfg}");
        }
    }
    if check {
        let mut failed = 0u8;
        for (label, root) in [
            ("scenarios_root", &scenarios_root),
            ("profiles_root", &profiles_root),
        ] {
            if let Err(e) = fs::create_dir_all(root) {
                eprintln!("check FAIL {label}={}: {e}", root.display());
                failed = failed.saturating_add(1);
                continue;
            }
            // Probe write by trying to create + remove a hidden file.
            let probe = root.join(".agent-qa-write-probe");
            let probe_ok = fs::write(&probe, b"").is_ok();
            let _ = fs::remove_file(&probe);
            if !probe_ok {
                eprintln!("check FAIL {label}={}: not writable", root.display());
                failed = failed.saturating_add(1);
            }
        }
        if failed != 0 {
            return Ok(1);
        }
        if !json_out {
            println!("check OK");
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_text_mode_exits_zero() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        assert_eq!(run(&[]).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn run_json_mode_exits_zero() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        assert_eq!(run(&["--json".to_string()]).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn run_unknown_flag_errors() {
        let err = run(&["--nope".to_string()]).unwrap_err().to_string();
        assert!(err.contains("unknown flag"));
    }

    #[test]
    fn run_check_passes_for_writable_root() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        assert_eq!(run(&["--check".to_string()]).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }
}
