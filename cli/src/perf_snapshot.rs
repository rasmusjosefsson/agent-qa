//! `perf-snapshot` verb — capture an optional performance sidecar.
//!
//! Drives the live agent-browser session to read `window.performance`
//! into a small JSON payload (navigation timings + paint entries) and
//! writes the result under `<sid>/perf/<TS>.json` (or stdout when no
//! sid is in scope).
//!
//! The surface is intentionally minimal: navigation + paint timings
//! cover the everyday 'is the page loading at the speed I expect?'
//! check. `--record-renders <ms>` for React-render profiling lands
//! later when agent-browser's react-renders subverb is wired in.

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use serde_json::Value as Json;

use crate::browser;
use crate::paths;
use crate::sidecar::atomic_write_file;

const PERF_EVAL: &str = r#"
(() => {
  const out = { navigation: null, paint: [], collectedAt: new Date().toISOString() };
  try {
    const navs = performance.getEntriesByType('navigation');
    if (navs && navs.length > 0) {
      const n = navs[0];
      out.navigation = {
        startTime: n.startTime,
        domContentLoadedEventEnd: n.domContentLoadedEventEnd,
        loadEventEnd: n.loadEventEnd,
        responseStart: n.responseStart,
        responseEnd: n.responseEnd,
        requestStart: n.requestStart,
        domComplete: n.domComplete,
        transferSize: n.transferSize,
        type: n.type,
        redirectCount: n.redirectCount,
      };
    }
    const paints = performance.getEntriesByType('paint') || [];
    for (const p of paints) {
      out.paint.push({ name: p.name, startTime: p.startTime });
    }
  } catch (e) { out.error = String(e); }
  return JSON.stringify(out);
})()
"#;

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let captured = capture(&opts.session)?;
    let body = serde_json::to_string_pretty(&captured)?;

    if let Some(sid) = &opts.sid {
        let dir = paths::perf_dir(sid)?;
        fs::create_dir_all(&dir).map_err(|e| anyhow!("mkdir -p {}: {e}", dir.display()))?;
        let ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H-%M-%S-%3fZ")
            .to_string();
        let path = dir.join(format!("{ts}.json"));
        let mut bytes = body.clone().into_bytes();
        bytes.push(b'\n');
        atomic_write_file(&path, &bytes)?;
        println!("perf snapshot → {}", path.display());
        if !opts.quiet {
            println!("{body}");
        }
    } else {
        println!("{body}");
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa perf-snapshot — capture a performance sidecar\n\nUsage:\n  agent-qa perf-snapshot [--sid <sid>] [--session <name>] [--quiet]\n\nReads window.performance from the live agent-browser session and emits\nnavigation + paint timings as JSON. With --sid, writes the snapshot\nto <sid>/perf/<TS>.json; otherwise prints to stdout."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    sid: Option<String>,
    session: String,
    quiet: bool,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut sid: Option<String> = None;
    let mut session: Option<String> = None;
    let mut quiet = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--sid" => sid = it.next().cloned(),
            s if s.starts_with("--sid=") => sid = Some(s["--sid=".len()..].to_string()),
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            "--quiet" => quiet = true,
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => bail!("unexpected positional {other:?}"),
        }
    }
    Ok(Opts {
        sid,
        session: session.unwrap_or_else(|| "default".to_string()),
        quiet,
    })
}

fn capture(session: &str) -> Result<Json> {
    let raw = browser::eval_expression(session, PERF_EVAL)
        .map_err(|e| anyhow!("agent-browser eval: {e}"))?;
    decode_eval(&raw)
}

/// agent-browser's eval returns the JS expression's value pretty-printed
/// as JSON. Our IIFE returns a JSON STRING (so it survives the bridge
/// transport intact), so we get back a double-encoded string. Unwrap once
/// then parse.
fn decode_eval(raw: &str) -> Result<Json> {
    let trimmed = raw.trim();
    let outer: Result<Json, _> = serde_json::from_str(trimmed);
    let inner_str = match outer {
        Ok(Json::String(s)) => s,
        Ok(other) => return Ok(other),
        Err(_) => return Ok(Json::String(trimmed.to_string())),
    };
    serde_json::from_str(&inner_str).or_else(|_| Ok(Json::String(inner_str)))
}

#[allow(dead_code)]
const _PERF_DIR_OK: fn(&str) -> anyhow::Result<PathBuf> = paths::perf_dir;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser as ab;
    use crate::test_util::lock_env;
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

    fn install_fake_perf(dir: &Path, inner_payload: &str) {
        // Write the desired response to a sibling file; have the fake
        // script just `cat` that file when invoked with eval. This
        // sidesteps shell-escaping pain inside the script body.
        let resp_path = dir.join("resp.txt");
        // agent-browser's eval double-encodes the in-page string, so we
        // serialize the inner payload into a JSON string literal.
        let outer = serde_json::to_string(inner_payload).unwrap();
        fs::write(&resp_path, outer).unwrap();
        let body = format!(
            "#!/bin/sh\nif [ \"$3\" = eval ]; then cat '{}'; exit 0; fi\nexit 0\n",
            resp_path.display()
        );
        let bin = write_exec(dir, &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn clear_fake() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    #[test]
    fn decode_eval_unwraps_double_encoded_json_string() {
        // Outer level: a JSON string. Inner: real JSON.
        let raw = r#""{\"navigation\":null,\"paint\":[]}""#;
        let v = decode_eval(raw).unwrap();
        assert!(v.is_object());
        assert!(v["paint"].is_array());
    }

    #[test]
    fn decode_eval_passes_through_object() {
        let raw = r#"{"x":1}"#;
        let v = decode_eval(raw).unwrap();
        assert_eq!(v["x"], 1);
    }

    #[test]
    fn parse_args_session_default() {
        let opts = parse_args(&[]).unwrap();
        assert_eq!(opts.session, "default");
        assert!(opts.sid.is_none());
        assert!(!opts.quiet);
    }

    #[test]
    fn parse_args_unknown_flag_errors() {
        parse_args(&["--what".into()]).unwrap_err();
    }

    #[test]
    fn capture_decodes_fake_browser_response() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        // Fake returns the JSON string carrying the perf payload.
        install_fake_perf(
            tmp.path(),
            r#"{"navigation":{"loadEventEnd":1234},"paint":[],"collectedAt":"now"}"#,
        );
        let v = capture("sess").unwrap();
        assert!(v.is_object(), "got: {v:?}");
        assert_eq!(v["navigation"]["loadEventEnd"], 1234);
        clear_fake();
    }
}
