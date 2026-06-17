//! Thin Rust wrapper around the `agent-browser` binary.
//!
//! agent-browser is a separate process that drives Chromium via CDP. Every
//! gesture the recorder / runner emits is a `spawn` of the binary with
//! verb-specific arguments. This module is the single chokepoint:
//!
//!   - Binary resolution: `AGENT_BROWSER_BIN` env var first (injected by
//!     the Node launcher), then
//!     `$PATH` lookup of `agent-browser`. Resolution result is cached.
//!   - Typed errors: every helper returns `Result<_, AgentBrowserError>`
//!     with the verb, exit code, and stderr captured for actionable
//!     surfacing.
//!   - Orphan-daemon auto-recovery: when the binary fails with the
//!     canonical "All CDP discovery methods failed / Auto-launch failed"
//!     pattern, we run `agent-browser close --session <s>` and retry
//!     once. One-shot, opt-out via `AGENT_QA_NO_AUTO_RECOVER=1`.
//!   - Socket-stall detection: surfaced with an actionable hint, NOT
//!     retried (the wedge persists across requests).
//!
//! This module ships only the cross-cutting infrastructure plus the most
//! foundational helper (`open`). Each later verb adds the helpers it needs
//! (click, fill, snapshot, screenshot, eval, …) — no megablob of unused
//! API surface lands ahead of demand.
#![allow(dead_code)]

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use thiserror::Error;
use which::which;

// ---------- error type ----------

#[derive(Debug, Error)]
pub enum AgentBrowserError {
    #[error(
        "agent-browser binary not found.\n\
         Looked up:\n  AGENT_BROWSER_BIN (env): {env_value}\n  $PATH lookup of `agent-browser`: not found\n\
         The Node launcher (npm/agent-qa/bin/agent-qa.js) is expected to set\n\
         AGENT_BROWSER_BIN to the resolved sibling npm dependency. Running\n\
         the binary directly? `npm i -g agent-browser` or set AGENT_BROWSER_BIN\n\
         to the absolute path."
    )]
    BinaryNotFound { env_value: String },

    #[error("agent-browser preflight failed (binary {binary}): {detail}")]
    PreflightFailed { binary: String, detail: String },

    #[error("spawn `agent-browser`: {source}")]
    Spawn {
        #[source]
        source: std::io::Error,
    },

    #[error("agent-browser {verb} exited {exit_code}: {stderr}{hint}")]
    NonZero {
        verb: String,
        exit_code: i32,
        stderr: String,
        /// Optional appended hint for known failure shapes
        /// (e.g. socket stall). Empty string if no hint.
        hint: String,
    },
}

impl AgentBrowserError {
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            AgentBrowserError::NonZero { exit_code, .. } => Some(*exit_code),
            _ => None,
        }
    }
}

// ---------- binary resolution ----------

pub const BIN_ENV: &str = "AGENT_BROWSER_BIN";
pub const NO_AUTO_RECOVER_ENV: &str = "AGENT_QA_NO_AUTO_RECOVER";

fn cached_bin() -> &'static Mutex<Option<PathBuf>> {
    static CELL: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

/// Test-only: reset the cached binary path.
#[doc(hidden)]
pub fn _reset_bin_cache_for_tests() {
    *cached_bin().lock().unwrap() = None;
}

/// Locate the agent-browser binary. Successful resolution is cached;
/// failures are re-attempted on every call (cheap, and lets a missing
/// binary become a working one without process restart).
pub fn resolve_bin() -> Result<PathBuf, AgentBrowserError> {
    {
        let guard = cached_bin().lock().unwrap();
        if let Some(cached) = guard.as_ref() {
            return Ok(cached.clone());
        }
    }
    let resolved = resolve_bin_uncached()?;
    *cached_bin().lock().unwrap() = Some(resolved.clone());
    Ok(resolved)
}

fn resolve_bin_uncached() -> Result<PathBuf, AgentBrowserError> {
    if let Ok(p) = env::var(BIN_ENV) {
        if !p.is_empty() {
            let path = PathBuf::from(&p);
            if path.is_file() {
                return Ok(path);
            }
            return Err(AgentBrowserError::BinaryNotFound { env_value: p });
        }
    }
    match which("agent-browser") {
        Ok(p) => Ok(p),
        Err(_) => Err(AgentBrowserError::BinaryNotFound {
            env_value: env::var(BIN_ENV).unwrap_or_else(|_| "(unset)".into()),
        }),
    }
}

/// Verify the binary is present AND runs `--version`. Cached on success.
/// Call before any verb that would otherwise crash with a confusing error.
pub fn ensure_installed() -> Result<(), AgentBrowserError> {
    static OK: OnceLock<()> = OnceLock::new();
    if OK.get().is_some() {
        return Ok(());
    }
    let bin = resolve_bin()?;
    let out = Command::new(&bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| AgentBrowserError::Spawn { source: e })?;
    if !out.status.success() {
        return Err(AgentBrowserError::PreflightFailed {
            binary: bin.display().to_string(),
            detail: format!(
                "exit {} / stderr: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    let _ = OK.set(());
    Ok(())
}

// ---------- failure-shape detection ----------

/// Detect the orphan-daemon footgun: agent-browser's named-session daemon
/// outlives its child Chrome (laptop sleep, OOM, manual kill). The
/// daemon stays bound to a stale port and refuses to relaunch Chrome,
/// so every subsequent CDP request fails with this canonical pair.
pub fn is_orphan_daemon_failure(stderr: &str, stdout: &str) -> bool {
    let s = format!("{stderr}\n{stdout}");
    s.contains("All CDP discovery methods failed") && s.contains("Auto-launch failed:")
}

/// Detect a wedged daemon socket: `Failed to read … os error 35` (EAGAIN).
/// Distinct from the orphan shape — the daemon IS answering, but a
/// specific request hangs. Not auto-retried; auto-retry would mask the
/// wedge and preserve it for the next request.
pub fn is_socket_stall_failure(stderr: &str, stdout: &str) -> bool {
    let s = format!("{stderr}\n{stdout}");
    s.contains("Failed to read") && s.contains("os error 35")
}

const NON_RECOVERABLE_VERBS: &[&str] = &["close", "doctor"];

fn auto_recovery_disabled() -> bool {
    env::var(NO_AUTO_RECOVER_ENV)
        .map(|v| v == "1")
        .unwrap_or(false)
}

// ---------- typed spawn wrapper ----------

#[derive(Debug, Default, Clone, Copy)]
pub struct RunOpts {
    /// When true (default), non-zero exit raises `AgentBrowserError::NonZero`.
    pub throw_on_error: bool,
    /// When true, stderr is captured into the returned struct. Default false
    /// (stderr is inherited so the user sees agent-browser's own logs live).
    pub capture_stderr: bool,
    /// When true, stdout is captured into the returned struct. Default false
    /// so daemonised agent-browser grandchildren cannot keep our pipe open.
    pub capture_stdout: bool,
}

impl RunOpts {
    pub fn new() -> Self {
        Self {
            throw_on_error: true,
            capture_stderr: false,
            capture_stdout: false,
        }
    }
    pub fn capture(mut self) -> Self {
        self.capture_stderr = true;
        self.capture_stdout = true;
        self
    }
    pub fn lenient(mut self) -> Self {
        self.throw_on_error = false;
        self
    }
}

#[derive(Debug)]
pub struct RunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Spawn `agent-browser --session <s> <args…>`, with one-shot orphan
/// recovery, optional throw-on-error, optional stderr capture.
pub fn run<I, S>(session: &str, args: I, opts: RunOpts) -> Result<RunResult, AgentBrowserError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let bin = resolve_bin()?;
    let args_vec: Vec<std::ffi::OsString> = args
        .into_iter()
        .map(|a| a.as_ref().to_os_string())
        .collect();
    run_with_bin(&bin, session, &args_vec, opts)
}

fn run_with_bin(
    bin: &Path,
    session: &str,
    args: &[std::ffi::OsString],
    opts: RunOpts,
) -> Result<RunResult, AgentBrowserError> {
    let primary_verb = args
        .first()
        .map(|a| a.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut first = spawn_once(bin, session, args, opts.capture_stdout, opts.capture_stderr)?;

    if first.exit_code != 0
        && !auto_recovery_disabled()
        && !primary_verb.is_empty()
        && !NON_RECOVERABLE_VERBS.contains(&primary_verb.as_str())
        && is_orphan_daemon_failure(&first.stderr, &first.stdout)
    {
        eprintln!(
            "[agent-browser] orphan daemon detected for session={session}; running 'agent-browser close --session {session}' and retrying once"
        );
        let _ = Command::new(bin)
            .args(["close", "--session", session])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        first = spawn_once(bin, session, args, opts.capture_stdout, opts.capture_stderr)?;
    }

    if opts.throw_on_error && first.exit_code != 0 {
        let hint = socket_stall_hint(&first.stderr, &first.stdout, &primary_verb, session);
        return Err(AgentBrowserError::NonZero {
            verb: primary_verb,
            exit_code: first.exit_code,
            stderr: if first.stderr.is_empty() {
                first.stdout.clone()
            } else {
                first.stderr.clone()
            },
            hint,
        });
    }
    Ok(first)
}

fn spawn_once(
    bin: &Path,
    session: &str,
    args: &[std::ffi::OsString],
    capture_stdout: bool,
    capture_stderr: bool,
) -> Result<RunResult, AgentBrowserError> {
    let timeout = agent_browser_timeout();
    let mut cmd = Command::new(bin);
    cmd.arg("--session").arg(session).args(args);
    cmd.stdin(Stdio::null());
    let stdout_path = capture_stdout.then(|| temp_capture_path("stdout"));
    let stderr_path = capture_stderr.then(|| temp_capture_path("stderr"));
    if capture_stdout {
        let path = stdout_path.as_ref().expect("stdout path exists");
        let file = fs::File::create(path).map_err(|e| AgentBrowserError::Spawn { source: e })?;
        cmd.stdout(Stdio::from(file));
    } else {
        cmd.stdout(Stdio::inherit());
    }
    if capture_stderr {
        let path = stderr_path.as_ref().expect("stderr path exists");
        let file = fs::File::create(path).map_err(|e| AgentBrowserError::Spawn { source: e })?;
        cmd.stderr(Stdio::from(file));
    } else {
        cmd.stderr(Stdio::inherit());
    }
    let mut child = cmd.spawn().map_err(|e| AgentBrowserError::Spawn { source: e })?;
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|e| AgentBrowserError::Spawn { source: e })?
            .is_some()
        {
            break;
        }
        if let Some(limit) = timeout {
            if started.elapsed() >= limit {
                let _ = child.kill();
                let _ = child.wait();
                let stdout = read_capture_file(stdout_path.as_deref());
                let verb = args
                    .first()
                    .map(|a| a.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "<unknown>".into());
                let mut stderr = read_capture_file(stderr_path.as_deref());
                if !stderr.is_empty() {
                    stderr.push('\n');
                }
                stderr.push_str(&format!(
                    "[agent-browser] timed out after {}ms running verb={verb} session={session}",
                    limit.as_millis()
                ));
                cleanup_capture_file(stdout_path.as_deref());
                cleanup_capture_file(stderr_path.as_deref());
                return Ok(RunResult {
                    stdout,
                    stderr,
                    exit_code: 124,
                });
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let status = child.wait().map_err(|e| AgentBrowserError::Spawn { source: e })?;
    let stdout = read_capture_file(stdout_path.as_deref());
    let stderr = read_capture_file(stderr_path.as_deref());
    cleanup_capture_file(stdout_path.as_deref());
    cleanup_capture_file(stderr_path.as_deref());
    Ok(RunResult {
        stdout,
        stderr,
        exit_code: exit_code(status),
    })
}

fn temp_capture_path(label: &str) -> PathBuf {
    let stamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    env::temp_dir().join(format!(
        "agent-qa-agent-browser-{label}-{}-{stamp}.log",
        std::process::id()
    ))
}

fn read_capture_file(path: Option<&Path>) -> String {
    path.and_then(|p| fs::read(p).ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

fn cleanup_capture_file(path: Option<&Path>) {
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
}

fn exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

fn agent_browser_timeout() -> Option<Duration> {
    let raw = env::var("AGENT_QA_AGENT_BROWSER_TIMEOUT_MS").ok()?;
    let ms = raw.parse::<u64>().ok()?;
    (ms > 0).then(|| Duration::from_millis(ms))
}

fn socket_stall_hint(stderr: &str, stdout: &str, verb: &str, session: &str) -> String {
    if NON_RECOVERABLE_VERBS.contains(&verb) || !is_socket_stall_failure(stderr, stdout) {
        return String::new();
    }
    format!(
        "\n[agent-browser] daemon socket stalled (EAGAIN / os error 35) for session={session}.\n\
        \x20               Likely cause: a large Runtime.evaluate payload the daemon could not drain.\n\
        \x20               Auto-retry is disabled for this failure shape (the wedge persists across requests).\n\
        \x20               Recover manually:\n\
        \x20                 agent-browser close --session {session}\n\
        \x20                 agent-browser open --session {session} <url>\n"
    )
}

// ---------- public helpers ----------

/// Open a URL in the named session's live tab.
pub fn open(session: &str, url: &str) -> Result<(), AgentBrowserError> {
    run(session, ["open", url], RunOpts::new())?;
    Ok(())
}

/// Wait `ms` milliseconds.
pub fn wait_ms(session: &str, ms: u64) -> Result<(), AgentBrowserError> {
    let arg = ms.to_string();
    run(session, ["wait", arg.as_str()], RunOpts::new())?;
    Ok(())
}

/// Wait until the page reaches a load state. Soft-fail (no throw):
/// agent-browser often returns non-zero when a page is already idle.
pub fn wait_for_load(session: &str, state: &str) -> Result<(), AgentBrowserError> {
    run(
        session,
        ["wait", "--load", state],
        RunOpts::new().lenient().capture(),
    )?;
    Ok(())
}

/// Run a JS expression in the page. Returns the binary's stdout
/// (typically a JSON-encoded representation of the in-page value).
pub fn eval_expression(session: &str, expression: &str) -> Result<String, AgentBrowserError> {
    let r = run(session, ["eval", expression], RunOpts::new().capture())?;
    Ok(r.stdout)
}

/// Press a key (e.g. "Enter", "Escape").
pub fn press_key(session: &str, key: &str) -> Result<(), AgentBrowserError> {
    run(session, ["press", key], RunOpts::new())?;
    Ok(())
}

/// Verb shape for [`find_role_act`]. Mirrors the agent-browser CLI:
///   `agent-browser --session <s> find role <role> <verb> [--name <n>] [<positional…>]`
#[derive(Debug, Clone, Copy)]
pub enum RoleAct {
    Click,
    Hover,
    Focus,
    Fill,
}

impl RoleAct {
    fn as_str(self) -> &'static str {
        match self {
            RoleAct::Click => "click",
            RoleAct::Hover => "hover",
            RoleAct::Focus => "focus",
            RoleAct::Fill => "fill",
        }
    }
}

/// Act on an element by ARIA role + accessible name.
/// For `Fill`, pass `value` as the positional argument.
pub fn find_role_act(
    session: &str,
    role: &str,
    name: &str,
    act: RoleAct,
    value: Option<&str>,
) -> Result<(), AgentBrowserError> {
    let mut args: Vec<&str> = vec!["find", "role", role, act.as_str(), "--name", name];
    if let Some(v) = value {
        args.push(v);
    }
    run(session, args, RunOpts::new())?;
    Ok(())
}

/// Quiet variant of [`find_role_act`] used by fallback probes. Captures
/// stderr (so the agent-browser "✗ Element not found" line doesn't
/// pollute the user-facing console) while still returning the same
/// `Err(AgentBrowserError)` shape on miss. Use this when calling the
/// role engine speculatively — i.e. when a follow-up fallback may
/// recover.
pub fn find_role_act_quiet(
    session: &str,
    role: &str,
    name: &str,
    act: RoleAct,
    value: Option<&str>,
) -> Result<(), AgentBrowserError> {
    let mut args: Vec<&str> = vec!["find", "role", role, act.as_str(), "--name", name];
    if let Some(v) = value {
        args.push(v);
    }
    run(session, args, RunOpts::new().capture())?;
    Ok(())
}

/// Act on an element matched by a CSS selector.
pub fn find_css_act(
    session: &str,
    selector: &str,
    act: RoleAct,
    value: Option<&str>,
) -> Result<(), AgentBrowserError> {
    let mut args: Vec<&str> = vec!["find", "css", selector, act.as_str()];
    if let Some(v) = value {
        args.push(v);
    }
    run(session, args, RunOpts::new())?;
    Ok(())
}

/// Act on an element using agent-browser's native selector verbs. Newer
/// agent-browser versions do not expose `find css`; the top-level verbs
/// accept CSS selectors directly.
pub fn selector_act(
    session: &str,
    selector: &str,
    act: RoleAct,
    value: Option<&str>,
) -> Result<(), AgentBrowserError> {
    match act {
        RoleAct::Click => run(session, ["click", selector], RunOpts::new()).map(|_| ()),
        RoleAct::Hover => run(session, ["hover", selector], RunOpts::new()).map(|_| ()),
        RoleAct::Focus => run(session, ["focus", selector], RunOpts::new()).map(|_| ()),
        RoleAct::Fill => run(
            session,
            ["fill", selector, value.unwrap_or("")],
            RunOpts::new(),
        )
        .map(|_| ()),
    }
}

/// Act on an element matched by an XPath expression.
pub fn find_xpath_act(
    session: &str,
    expr: &str,
    act: RoleAct,
    value: Option<&str>,
) -> Result<(), AgentBrowserError> {
    let mut args: Vec<&str> = vec!["find", "xpath", expr, act.as_str()];
    if let Some(v) = value {
        args.push(v);
    }
    run(session, args, RunOpts::new())?;
    Ok(())
}

/// Fill a labelled input. Mirrors the agent-browser one-shot:
///   `agent-browser --session <s> find label <label> fill <value>`
pub fn find_label_fill(session: &str, label: &str, value: &str) -> Result<(), AgentBrowserError> {
    let args: Vec<&str> = vec!["find", "label", label, "fill", value];
    run(session, args, RunOpts::new())?;
    Ok(())
}

/// Act on an element matched by visible text content. Mirrors:
///   `agent-browser --session <s> find text <text> <act> [<value>]`
///
/// agent-browser's `find text` locator currently supports `click`,
/// `hover`, `fill` actions; it does NOT support `focus`. Callers that
/// need focus (e.g. presence asserts) should fall back to a DOM eval.
pub fn find_text_act(
    session: &str,
    text: &str,
    act: RoleAct,
    value: Option<&str>,
) -> Result<(), AgentBrowserError> {
    let mut args: Vec<&str> = vec!["find", "text", text, act.as_str()];
    if let Some(v) = value {
        args.push(v);
    }
    run(session, args, RunOpts::new())?;
    Ok(())
}

/// Quiet variant of [`find_text_act`] for fallback probes — captures
/// stderr to keep the console clean when called speculatively.
pub fn find_text_act_quiet(
    session: &str,
    text: &str,
    act: RoleAct,
    value: Option<&str>,
) -> Result<(), AgentBrowserError> {
    let mut args: Vec<&str> = vec!["find", "text", text, act.as_str()];
    if let Some(v) = value {
        args.push(v);
    }
    run(session, args, RunOpts::new().capture())?;
    Ok(())
}

/// Click an element by its agent-browser snapshot ref (`@eN`). This is
/// the lowest-level locator agent-browser exposes — it bypasses all
/// accessible-name calc, role inference, and visibility heuristics by
/// resolving the backend node id directly from the snapshot. Use this
/// as the final rung of a fallback ladder: when the snapshot promises
/// `<role> "<name>" [ref=eN]` but `find role --name` and `find text`
/// both miss, clicking `@eN` is guaranteed to land on that exact node.
pub fn click_ref(session: &str, snapshot_ref: &str) -> Result<(), AgentBrowserError> {
    // agent-browser accepts both `@eN` and the bare `eN` form for the
    // `click` verb's selector argument; we always pass the `@`-prefixed
    // shape so a future tightening of the parser doesn't break us.
    let with_at: String = if snapshot_ref.starts_with('@') {
        snapshot_ref.to_string()
    } else {
        format!("@{snapshot_ref}")
    };
    run(session, ["click", with_at.as_str()], RunOpts::new())?;
    Ok(())
}

/// Parse an agent-browser ARIA snapshot for the first line that matches
/// `<role> "<name>" [... ref=(eN)]`. Returns the ref string (e.g.
/// `"e54"`) or None.
///
/// Snapshot lines look like:
///   `- button "Resources" [expanded=false, ref=e54]`
///   `- link "Docs Vercel documentation" [ref=e69]`
///
/// We match `"<role>\s+\"<name>\""` followed by anything up to a
/// `ref=(eN)` token. Name match is exact (post-trim). If the same
/// role+name appears twice we return the first — callers concerned
/// about ambiguity should narrow the name first.
pub fn find_ref_in_snapshot(snapshot: &str, role: &str, name: &str) -> Option<String> {
    let needle = format!("{role} \"{name}\"");
    for line in snapshot.lines() {
        let trimmed = line.trim_start_matches([' ', '-', '\t']);
        if !trimmed.starts_with(&needle) {
            continue;
        }
        // Ensure the match is on a word boundary: the next char after
        // the closing quote must be a space, `[`, or end-of-line. This
        // prevents "link \"Docs\"" from matching when we asked for
        // "link \"Doc\"".
        let after = &trimmed[needle.len()..];
        if !after.is_empty()
            && !after.starts_with(' ')
            && !after.starts_with('[')
            && !after.starts_with('\t')
        {
            continue;
        }
        // Find ref=eN within this line.
        if let Some(idx) = line.find("ref=") {
            let rest = &line[idx + 4..];
            let end = rest
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            let r = &rest[..end];
            if !r.is_empty() {
                return Some(r.to_string());
            }
        }
    }
    None
}

/// Upload one or more files to a `<input type="file">`. Mirrors the
/// agent-browser one-shot:
///   `agent-browser --session <s> upload <selector> <file>...`
///
/// `selector` is a CSS selector or `@ref` from a snapshot. `files` are
/// passed through as-is — callers are responsible for resolving relative
/// paths against whatever base they prefer (cwd, scenario dir, etc.).
pub fn upload(session: &str, selector: &str, files: &[String]) -> Result<(), AgentBrowserError> {
    if files.is_empty() {
        return Err(AgentBrowserError::PreflightFailed {
            binary: "agent-browser".into(),
            detail: "upload: at least one file path is required".into(),
        });
    }
    let mut args: Vec<&str> = vec!["upload", selector];
    for f in files {
        args.push(f.as_str());
    }
    run(session, args, RunOpts::new())?;
    Ok(())
}

/// Run `agent-browser doctor` and return its stdout.
pub fn doctor_raw() -> Result<String, AgentBrowserError> {
    let bin = resolve_bin()?;
    let out = Command::new(&bin)
        .arg("doctor")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| AgentBrowserError::Spawn { source: e })?;
    if !out.status.success() {
        return Err(AgentBrowserError::PreflightFailed {
            binary: bin.display().to_string(),
            detail: format!(
                "doctor exit {} / stderr: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Take a screenshot to `out_path`. `full_page` enables the agent-browser
/// `--full` flag. Lenient — failures don't abort the run.
pub fn screenshot(
    session: &str,
    out_path: &std::path::Path,
    full_page: bool,
) -> Result<bool, AgentBrowserError> {
    let mut args: Vec<&str> = vec!["screenshot"];
    if full_page {
        args.push("--full");
    }
    let p = out_path.display().to_string();
    args.push(p.as_str());
    let r = run(session, args, RunOpts::new().lenient().capture())?;
    Ok(r.exit_code == 0)
}

/// Take an interactive a11y snapshot — returns the human-readable text
/// agent-browser prints to stdout.
pub fn snapshot_interactive(session: &str) -> Result<String, AgentBrowserError> {
    let r = run(session, ["snapshot", "-i"], RunOpts::new().capture())?;
    Ok(r.stdout)
}

/// Take a FULL a11y snapshot, including non-interactive nodes such as
/// `StaticText` — required when a check or compare needs to match text
/// the interactive snapshot drops.
pub fn snapshot_full(session: &str) -> Result<String, AgentBrowserError> {
    let r = run(session, ["snapshot"], RunOpts::new().capture())?;
    Ok(r.stdout)
}

/// Close the live tab + daemon for a session. Best-effort: a missing
/// session returns `false` rather than erroring.
pub fn close_session(session: &str) -> bool {
    let bin = match resolve_bin() {
        Ok(b) => b,
        Err(_) => return false,
    };
    Command::new(&bin)
        .args(["close", "--session", session])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn find_ref_extracts_simple_match() {
        let snap = r#"- generic
  - banner
    - button "Resources" [expanded=false, ref=e54]
      - StaticText "Resources"
"#;
        assert_eq!(
            find_ref_in_snapshot(snap, "button", "Resources"),
            Some("e54".to_string())
        );
    }

    #[test]
    fn find_ref_handles_link_with_compound_name() {
        let snap = r#"  - link "Docs Vercel documentation" [ref=e69]
    - image
    - StaticText "Docs"
"#;
        assert_eq!(
            find_ref_in_snapshot(snap, "link", "Docs Vercel documentation"),
            Some("e69".to_string())
        );
    }

    #[test]
    fn find_ref_returns_none_when_no_match() {
        let snap = "  - button \"Submit\" [ref=e1]\n";
        assert_eq!(find_ref_in_snapshot(snap, "button", "Cancel"), None);
        assert_eq!(find_ref_in_snapshot(snap, "link", "Submit"), None);
    }

    #[test]
    fn find_ref_skips_lines_with_no_ref_token() {
        // The button is mentioned in body copy without a ref=, then
        // again later with a ref=. We should pick the one that has it.
        let snap = r#"  - paragraph "button Resources is helpful"
  - button "Resources" [expanded=false, ref=e54]
"#;
        assert_eq!(
            find_ref_in_snapshot(snap, "button", "Resources"),
            Some("e54".to_string())
        );
    }

    #[test]
    fn find_ref_word_boundary_prevents_partial_name_match() {
        // Searching for `link "Doc"` must NOT match `link "Docs"`.
        let snap = "  - link \"Docs\" [ref=e7]\n";
        assert_eq!(find_ref_in_snapshot(snap, "link", "Doc"), None);
        assert_eq!(
            find_ref_in_snapshot(snap, "link", "Docs"),
            Some("e7".to_string())
        );
    }

    #[test]
    fn find_ref_first_match_wins() {
        let snap = r#"  - button "Save" [ref=e1]
  - button "Save" [ref=e2]
"#;
        assert_eq!(
            find_ref_in_snapshot(snap, "button", "Save"),
            Some("e1".to_string())
        );
    }

    // Env mutation isn't thread-safe; serialize via the shared lock.
    use crate::test_util::lock_env;

    fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn fake_browser(dir: &Path, body: &str) -> PathBuf {
        write_exec(dir, "agent-browser", body)
    }

    fn set_bin(p: &Path) {
        env::set_var(BIN_ENV, p);
        _reset_bin_cache_for_tests();
    }

    fn clear_bin() {
        env::remove_var(BIN_ENV);
        _reset_bin_cache_for_tests();
    }

    // ----- bin resolution -----

    #[test]
    fn bin_env_var_wins_when_present() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let bin = fake_browser(tmp.path(), "#!/bin/sh\nexit 0\n");
        set_bin(&bin);
        assert_eq!(resolve_bin().unwrap(), bin);
        clear_bin();
    }

    #[test]
    fn bin_env_var_missing_file_errors() {
        let _g = lock_env();
        env::set_var(BIN_ENV, "/nope/agent-browser-missing");
        _reset_bin_cache_for_tests();
        let err = resolve_bin().unwrap_err();
        assert!(matches!(err, AgentBrowserError::BinaryNotFound { .. }));
        clear_bin();
    }

    // ----- failure-shape detection (pure data) -----

    #[test]
    fn orphan_daemon_pattern_matches() {
        let stderr = "Auto-launch failed: All CDP discovery methods failed for 127.0.0.1";
        assert!(is_orphan_daemon_failure(stderr, ""));
        assert!(is_orphan_daemon_failure("", stderr));
    }

    #[test]
    fn orphan_daemon_requires_both_strings() {
        assert!(!is_orphan_daemon_failure(
            "All CDP discovery methods failed",
            ""
        ));
        assert!(!is_orphan_daemon_failure("Auto-launch failed: x", ""));
    }

    #[test]
    fn socket_stall_pattern_matches() {
        let stderr = "Failed to read: Resource temporarily unavailable (os error 35)";
        assert!(is_socket_stall_failure(stderr, ""));
    }

    #[test]
    fn socket_stall_requires_both_strings() {
        assert!(!is_socket_stall_failure("Failed to read", ""));
        assert!(!is_socket_stall_failure("os error 35", ""));
    }

    // ----- run wrapper -----

    #[test]
    fn run_success_returns_stdout() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        // Echo the args back so we can assert wire format.
        let bin = fake_browser(tmp.path(), "#!/bin/sh\necho \"args:$*\"\nexit 0\n");
        set_bin(&bin);
        let r = run(
            "default",
            ["open", "https://example.com"],
            RunOpts::new().capture(),
        )
        .unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(
            r.stdout
                .contains("--session default open https://example.com"),
            "got {:?}",
            r.stdout
        );
        clear_bin();
    }

    #[test]
    fn run_nonzero_errors_with_verb_and_stderr() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let bin = fake_browser(
            tmp.path(),
            "#!/bin/sh\necho 'something broke' 1>&2\nexit 3\n",
        );
        set_bin(&bin);
        let err = run("s", ["click", "@e22"], RunOpts::new().capture()).unwrap_err();
        match err {
            AgentBrowserError::NonZero {
                verb,
                exit_code,
                stderr,
                ..
            } => {
                assert_eq!(verb, "click");
                assert_eq!(exit_code, 3);
                assert!(stderr.contains("something broke"));
            }
            other => panic!("expected NonZero, got {other:?}"),
        }
        clear_bin();
    }

    #[test]
    fn run_lenient_returns_nonzero_result() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let bin = fake_browser(tmp.path(), "#!/bin/sh\nexit 5\n");
        set_bin(&bin);
        let r = run("s", ["get", "url"], RunOpts::new().lenient().capture()).unwrap();
        assert_eq!(r.exit_code, 5);
        clear_bin();
    }

    #[test]
    fn orphan_daemon_triggers_one_retry() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        // State file the fake binary uses to alternate failure→success.
        let state = tmp.path().join("count");
        let body = format!(
            "#!/bin/sh\nCOUNT_FILE='{}'\n\
            if [ \"$1\" = 'close' ]; then exit 0; fi\n\
            N=$( ([ -f \"$COUNT_FILE\" ] && cat \"$COUNT_FILE\") || echo 0)\n\
            N=$((N+1))\n\
            echo \"$N\" > \"$COUNT_FILE\"\n\
            if [ \"$N\" -eq 1 ]; then\n  echo 'Auto-launch failed: All CDP discovery methods failed' 1>&2\n  exit 1\nfi\n\
            echo 'ok'\nexit 0\n",
            state.display()
        );
        let bin = fake_browser(tmp.path(), &body);
        set_bin(&bin);
        let r = run(
            "s",
            ["open", "https://example.com"],
            RunOpts::new().capture(),
        )
        .unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("ok"));
        // First call attempted; close ran; retry succeeded → count file holds 2.
        let n: u32 = fs::read_to_string(&state).unwrap().trim().parse().unwrap();
        assert_eq!(n, 2, "expected exactly one retry");
        clear_bin();
    }

    #[test]
    fn orphan_recovery_opted_out_via_env() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let body =
            "#!/bin/sh\necho 'Auto-launch failed: All CDP discovery methods failed' 1>&2\nexit 1\n";
        let bin = fake_browser(tmp.path(), body);
        set_bin(&bin);
        env::set_var(NO_AUTO_RECOVER_ENV, "1");
        let err = run("s", ["open", "x"], RunOpts::new().capture()).unwrap_err();
        env::remove_var(NO_AUTO_RECOVER_ENV);
        assert!(matches!(err, AgentBrowserError::NonZero { .. }));
        clear_bin();
    }

    #[test]
    fn socket_stall_emits_actionable_hint() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let body = "#!/bin/sh\necho 'Failed to read: Resource temporarily unavailable (os error 35)' 1>&2\nexit 4\n";
        let bin = fake_browser(tmp.path(), body);
        set_bin(&bin);
        let err = run("s", ["eval", "x"], RunOpts::new().capture()).unwrap_err();
        let s = format!("{err}");
        assert!(
            s.contains("socket stalled"),
            "expected stall hint, got: {s}"
        );
        assert!(
            s.contains("close --session s"),
            "expected recovery hint, got: {s}"
        );
        clear_bin();
    }

    #[test]
    fn open_uses_correct_args() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let log = tmp.path().join("log");
        let body = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let bin = fake_browser(tmp.path(), &body);
        set_bin(&bin);
        open("default", "https://example.com").unwrap();
        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            invocation.contains("--session default open https://example.com"),
            "got {invocation:?}"
        );
        clear_bin();
    }

    #[test]
    fn close_session_returns_true_on_zero_exit() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let bin = fake_browser(tmp.path(), "#!/bin/sh\nexit 0\n");
        set_bin(&bin);
        assert!(close_session("default"));
        clear_bin();
    }

    #[test]
    fn close_session_returns_false_on_missing_binary() {
        let _g = lock_env();
        env::set_var(BIN_ENV, "/nope/missing");
        _reset_bin_cache_for_tests();
        assert!(!close_session("default"));
        clear_bin();
    }
}
