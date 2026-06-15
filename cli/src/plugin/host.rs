//! Invoke a plugin: spawn the subprocess, write the JSON request on stdin,
//! read the JSON response from stdout, parse and surface.
//!
//! Errors are intentionally rich: we want `plugins doctor` to print
//! actionable information when something is wrong with a third-party plugin.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};
use thiserror::Error;

use super::protocol::{PingResponse, PluginRequest, PluginResponse, PROTOCOL_VERSION};

#[derive(Debug, Error)]
pub enum InvokeError {
    #[error("spawn `{binary}`: {source}")]
    Spawn {
        binary: String,
        #[source]
        source: std::io::Error,
    },
    #[error("write stdin to `{binary}`: {source}")]
    Stdin {
        binary: String,
        #[source]
        source: std::io::Error,
    },
    #[error("wait for `{binary}`: {source}")]
    Wait {
        binary: String,
        #[source]
        source: std::io::Error,
    },
    #[error("plugin `{binary}` exited with status {status}; stderr: {stderr}")]
    NonZeroExit {
        binary: String,
        status: i32,
        stderr: String,
    },
    #[error("plugin `{binary}` returned malformed JSON: {source}\nstdout was: {stdout}")]
    BadJson {
        binary: String,
        stdout: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("plugin `{binary}` returned ok=true with no response payload")]
    EmptyOk { binary: String },
    #[error("plugin `{binary}` reported error [{code}]: {message}")]
    PluginReportedError {
        binary: String,
        code: String,
        message: String,
    },
    #[error(
        "plugin `{binary}` speaks protocol version {plugin_version}, but this build of agent-qa supports only up to {host_version}. Upgrade agent-qa or downgrade the plugin."
    )]
    ProtocolVersionTooNew {
        binary: String,
        plugin_version: u32,
        host_version: u32,
    },
}

/// What a successful invocation returned. The caller of `invoke` is expected
/// to know how to interpret `response` for the given `kind`.
#[derive(Debug)]
pub struct InvokeOutcome {
    pub response: Value,
}

/// Run `<binary> <kind> [<op>]` with the given request payload on stdin.
/// Timeout is approximate (we don't kill the child) — agent-qa plugins are
/// expected to be fast or to handle their own timeouts internally.
pub fn invoke(
    binary: &Path,
    kind: &str,
    op: Option<&str>,
    request_payload: Value,
    _timeout: Duration,
) -> Result<InvokeOutcome, InvokeError> {
    let bin_str = binary.display().to_string();

    let mut cmd = Command::new(binary);
    cmd.arg(kind);
    if let Some(op) = op {
        cmd.arg(op);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Retry once on ETXTBSY (errno 26, ExecutableFileBusy). On Linux
    // with parallel cargo tests, a sibling thread can hold a writable
    // fd to the binary at exec time — even though the writer thread
    // already closed its handle, fork() in another thread inherits all
    // open fds and the kernel refuses to exec a file open-for-write.
    // The window is microseconds; one short backoff clears it. This is
    // a pre-existing test flake that bit CI on PR #2.
    let mut spawn_attempt = || cmd.spawn();
    let mut child = match spawn_attempt() {
        Ok(c) => c,
        Err(e) if e.raw_os_error() == Some(26) => {
            std::thread::sleep(std::time::Duration::from_millis(50));
            spawn_attempt().map_err(|e| InvokeError::Spawn {
                binary: bin_str.clone(),
                source: e,
            })?
        }
        Err(e) => {
            return Err(InvokeError::Spawn {
                binary: bin_str.clone(),
                source: e,
            })
        }
    };

    let envelope = PluginRequest {
        protocol_version: PROTOCOL_VERSION,
        request: request_payload,
    };
    let stdin_bytes = serde_json::to_vec(&envelope).expect("serialize PluginRequest");

    if let Some(mut stdin) = child.stdin.take() {
        // A plugin that doesn't need its request (e.g. it errors before
        // reading) may close stdin before we finish writing. Treat that
        // as a non-fatal signal and proceed to read whatever stdout/stderr
        // it produced — the parse step will surface BadJson, the exit
        // step will surface NonZeroExit, etc.
        if let Err(e) = stdin.write_all(&stdin_bytes) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(InvokeError::Stdin {
                    binary: bin_str.clone(),
                    source: e,
                });
            }
        }
        // stdin dropped here → EOF → plugin can stop reading.
    }

    let output = child.wait_with_output().map_err(|e| InvokeError::Wait {
        binary: bin_str.clone(),
        source: e,
    })?;

    if !output.status.success() {
        return Err(InvokeError::NonZeroExit {
            binary: bin_str,
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let parsed: PluginResponse =
        serde_json::from_str(stdout.trim()).map_err(|e| InvokeError::BadJson {
            binary: bin_str.clone(),
            stdout: stdout.clone(),
            source: e,
        })?;

    if !parsed.ok {
        let err = parsed
            .error
            .unwrap_or_else(|| super::protocol::PluginError {
                code: "missing-error".into(),
                message: "plugin returned ok=false without an error object".into(),
            });
        return Err(InvokeError::PluginReportedError {
            binary: bin_str,
            code: err.code,
            message: err.message,
        });
    }

    let response = parsed
        .response
        .ok_or(InvokeError::EmptyOk { binary: bin_str })?;
    Ok(InvokeOutcome { response })
}

/// Convenience: invoke a plugin's universal `ping` kind and decode the
/// canonical [`PingResponse`]. Enforces protocol-version negotiation:
/// a plugin claiming a `protocolVersion` higher than [`PROTOCOL_VERSION`]
/// is refused with [`InvokeError::ProtocolVersionTooNew`].
pub fn ping(binary: &Path) -> Result<PingResponse, InvokeError> {
    let outcome = invoke(binary, "ping", None, json!({}), Duration::from_secs(5))?;
    let pong: PingResponse =
        serde_json::from_value(outcome.response.clone()).map_err(|e| InvokeError::BadJson {
            binary: binary.display().to_string(),
            stdout: outcome.response.to_string(),
            source: e,
        })?;
    if pong.protocol_version > PROTOCOL_VERSION {
        return Err(InvokeError::ProtocolVersionTooNew {
            binary: binary.display().to_string(),
            plugin_version: pong.protocol_version,
            host_version: PROTOCOL_VERSION,
        });
    }
    Ok(pong)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn write_exec(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    #[test]
    fn ping_against_noop_plugin_works() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(
            tmp.path(),
            "agent-qa-plugin-noop",
            "#!/bin/sh\nif [ \"$1\" = ping ]; then\n  echo '{\"ok\":true,\"response\":{\"protocolVersion\":1,\"name\":\"noop\",\"kinds\":[\"auth\"]}}'\n  exit 0\nfi\nexit 0\n",
        );
        let pr = ping(&plugin).unwrap();
        assert_eq!(pr.name, "noop");
        assert_eq!(pr.kinds, vec!["auth"]);
        assert_eq!(pr.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn plugin_reported_error_surfaces() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(
            tmp.path(),
            "agent-qa-plugin-bad",
            "#!/bin/sh\necho '{\"ok\":false,\"error\":{\"code\":\"bad\",\"message\":\"boom\"}}'\nexit 0\n",
        );
        let err = ping(&plugin).unwrap_err();
        match err {
            InvokeError::PluginReportedError { code, message, .. } => {
                assert_eq!(code, "bad");
                assert_eq!(message, "boom");
            }
            other => panic!("expected PluginReportedError, got {other:?}"),
        }
    }

    #[test]
    fn malformed_json_is_surfaced_with_stdout() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(
            tmp.path(),
            "agent-qa-plugin-malformed",
            "#!/bin/sh\necho 'not json'\nexit 0\n",
        );
        let err = ping(&plugin).unwrap_err();
        match err {
            InvokeError::BadJson { stdout, .. } => assert!(stdout.contains("not json")),
            other => panic!("expected BadJson, got {other:?}"),
        }
    }

    #[test]
    fn non_zero_exit_is_surfaced() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(
            tmp.path(),
            "agent-qa-plugin-fail",
            "#!/bin/sh\necho 'oops' 1>&2\nexit 7\n",
        );
        let err = ping(&plugin).unwrap_err();
        match err {
            InvokeError::NonZeroExit { status, stderr, .. } => {
                assert_eq!(status, 7);
                assert!(stderr.contains("oops"));
            }
            other => panic!("expected NonZeroExit, got {other:?}"),
        }
    }

    #[test]
    fn ping_refuses_plugin_with_future_protocol_version() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(
            tmp.path(),
            "agent-qa-plugin-future",
            "#!/bin/sh\necho '{\"ok\":true,\"response\":{\"protocolVersion\":99,\"name\":\"future\",\"kinds\":[\"auth\"]}}'\nexit 0\n",
        );
        let err = ping(&plugin).unwrap_err();
        match err {
            InvokeError::ProtocolVersionTooNew {
                plugin_version,
                host_version,
                ..
            } => {
                assert_eq!(plugin_version, 99);
                assert_eq!(host_version, PROTOCOL_VERSION);
            }
            other => panic!("expected ProtocolVersionTooNew, got {other:?}"),
        }
    }

    #[test]
    fn ping_accepts_plugin_with_current_protocol_version() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_exec(
            tmp.path(),
            "agent-qa-plugin-current",
            "#!/bin/sh\necho '{\"ok\":true,\"response\":{\"protocolVersion\":1,\"name\":\"cur\",\"kinds\":[\"auth\"]}}'\nexit 0\n",
        );
        let pong = ping(&plugin).unwrap();
        assert_eq!(pong.protocol_version, 1);
    }
}
