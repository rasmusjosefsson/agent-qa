//! `byo-doctor` verb — read-only enumeration of attachable CDP browsers.
//!
//! agent-qa's BYO mode lets the agent drive a browser the user already
//! launched (Brave / Chrome / etc) instead of the bundled headless tab.
//! This verb passes through to `agent-browser doctor` so the agent can
//! see, in one call, which browsers are attachable, which sessions are
//! live, and which install vendors are available — before deciding
//! whether to use BYO at all.
//!
//! Output modes:
//!   - default — agent-browser's stdout, verbatim (a human-readable summary)
//!   - --json — when agent-browser supports `doctor --json` upstream,
//!     the wrapper also accepts the flag and forwards it. For older
//!     agent-browsers that don't, the verb falls back to the default
//!     text and signals that on stderr.

use anyhow::{bail, Result};

use crate::browser;

pub fn run(args: &[String]) -> Result<u8> {
    let mut json_out = false;
    for a in args {
        match a.as_str() {
            "--json" => json_out = true,
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(0);
            }
            other => bail!("byo-doctor: unknown arg {other:?}"),
        }
    }
    let raw = browser::doctor_raw()?;
    if json_out {
        // Best-effort JSON wrap: if agent-browser produced JSON itself,
        // pass it through; otherwise wrap the text so callers always
        // get parseable output under --json.
        match serde_json::from_str::<serde_json::Value>(raw.trim()) {
            Ok(_) => print!("{}", raw),
            Err(_) => {
                let wrapped = serde_json::json!({
                    "format": "text",
                    "content": raw,
                });
                println!("{}", serde_json::to_string_pretty(&wrapped)?);
            }
        }
    } else {
        print!("{}", raw);
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa byo-doctor — read-only enumeration of attachable CDP browsers\n\nUsage:\n  agent-qa byo-doctor          Pass through `agent-browser doctor`\n  agent-qa byo-doctor --json   Wrap the response in JSON if not already\n\nUseful before proposing any --byo invocation: the agent can see which\nbrowsers are attachable, which sessions are live, and which install\nvendors are available before deciding whether to use BYO at all."
    );
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser as ab;
    use crate::test_util::lock_env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_exec(dir: &Path, body: &str) -> std::path::PathBuf {
        let p = dir.join("agent-browser");
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn install_fake(dir: &Path, body: &str) {
        let bin = write_exec(dir, body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }
    fn clear_fake() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    #[test]
    fn doctor_raw_returns_stdout() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(
            tmp.path(),
            "#!/bin/sh\nif [ \"$1\" = doctor ]; then echo 'Daemons: none'; exit 0; fi\nexit 0\n",
        );
        let out = browser::doctor_raw().unwrap();
        assert!(out.contains("Daemons"), "got: {out}");
        clear_fake();
    }

    #[test]
    fn doctor_raw_errors_on_non_zero_exit() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        install_fake(tmp.path(), "#!/bin/sh\nexit 7\n");
        let err = browser::doctor_raw().unwrap_err().to_string();
        assert!(err.contains("preflight failed"), "got: {err}");
        clear_fake();
    }

    #[test]
    fn run_unknown_flag_bails() {
        // Doesn't actually call agent-browser — fails on arg parsing first.
        let err = run(&["--what".into()]).unwrap_err().to_string();
        assert!(err.contains("unknown arg"));
    }
}
