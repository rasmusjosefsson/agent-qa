//! Spawn a surface provider: one JSON request on stdin, one JSON
//! response on stdout. Same envelope as plugins, separate binary, and
//! never invoked for a web scenario.

use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Serialize)]
struct Envelope {
    #[serde(rename = "protocolVersion")]
    protocol_version: u32,
    request: Json,
}

#[derive(Deserialize)]
struct Response {
    ok: bool,
    #[serde(default)]
    response: Option<Json>,
    #[serde(default)]
    error: Option<Reported>,
}

#[derive(Deserialize)]
struct Reported {
    code: String,
    message: String,
}

/// Run `<binary> <op>` and return the `response` object.
pub fn invoke(binary: &Path, op: &str, request: Json, timeout: Duration) -> Result<Json> {
    let bin_str = binary.display().to_string();
    let mut cmd = Command::new(binary);
    cmd.arg(op)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut spawn = || cmd.spawn();
    let mut child = match spawn() {
        Ok(c) => c,
        Err(e) if e.raw_os_error() == Some(26) => {
            std::thread::sleep(Duration::from_millis(50));
            spawn().with_context(|| format!("spawn surface provider `{bin_str}`"))?
        }
        Err(e) => {
            return Err(e).with_context(|| format!("spawn surface provider `{bin_str}`"));
        }
    };

    let body = serde_json::to_vec(&Envelope {
        protocol_version: PROTOCOL_VERSION,
        request,
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(&body) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(e).with_context(|| format!("write stdin to `{bin_str}`"));
            }
        }
    }

    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                return Err(e).with_context(|| format!("wait for surface provider `{bin_str}`"));
            }
        }
    };
    let stdout_bytes = out_reader.join().unwrap_or_default();
    let stderr_bytes = err_reader.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let stderr_one = stderr.trim().lines().next().unwrap_or("").to_string();

    let Some(status) = status else {
        bail!(
            "surface provider `{bin_str}` {op} did not exit within {:.0}s{}",
            timeout.as_secs_f64(),
            if stderr_one.is_empty() {
                String::new()
            } else {
                format!(" — {stderr_one}")
            }
        );
    };
    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let parsed: Response = serde_json::from_str(stdout.trim()).with_context(|| {
        format!(
            "surface provider `{bin_str}` {op} returned malformed JSON (exit {}): {}",
            status.code().unwrap_or(-1),
            stdout.trim().chars().take(400).collect::<String>()
        )
    })?;
    if !status.success() && parsed.ok {
        bail!(
            "surface provider `{bin_str}` {op} exited {} with ok=true",
            status.code().unwrap_or(-1)
        );
    }
    if !parsed.ok {
        let err = parsed.error.unwrap_or(Reported {
            code: "missing-error".into(),
            message: "provider returned ok=false without an error object".into(),
        });
        bail!(
            "surface provider `{bin_str}` {op} [{code}]: {message}",
            code = err.code,
            message = err.message
        );
    }
    parsed.response.ok_or_else(|| {
        anyhow::anyhow!("surface provider `{bin_str}` {op} returned ok with no response")
    })
}
