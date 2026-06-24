//! scenario/2 replay sidecar tree writer.
//!
//! Implements the path convention spec at
//! [`docs/specs/scenario-sidecar-tree.md`](../../docs/specs/scenario-sidecar-tree.md):
//!
//!   - `<SID>/replays/<runId>/` per run
//!   - `<runId>` = `<isoTimestamp>__<shortHash>[__<profile>]`
//!     (8 random hex chars; profile only when it matches the safe
//!     filename regex)
//!   - per-kind subdirs (`snapshots/`, `screenshots/`, `screenshots/`,
//!     `network/`, `probes/`, `perf/`) with `<stepId>.<ext>` keying
//!     (LITERAL stepId — no zero-padded positional index)
//!   - `heal.jsonl` — one run-scoped JSONL of heal events
//!   - `audit.json` — run-level metadata
//!   - `replays/latest.txt` — one-line pointer to latest `<runId>`
//!   - atomic writes via `<path>.tmp` → rename
//!
//! Per the spec, the contract (`scenario.json`) MUST NOT be mutated by
//! the runner. This module never opens `scenario.json` for writing.
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use sha2::{Digest, Sha256};

// ---------- types ----------

#[derive(Debug, Clone)]
pub struct RunPaths {
    /// Absolute path to the scenario directory (parent of `replays/`).
    pub scenario_dir: PathBuf,
    /// The minted `<runId>`.
    pub run_id: String,
    /// Absolute path to `<scenario_dir>/replays/<run_id>/`.
    pub run_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidecarKind {
    Snapshots,
    Screenshots,
    Network,
    Probes,
    Perf,
}

impl SidecarKind {
    pub fn dir_name(self) -> &'static str {
        match self {
            SidecarKind::Snapshots => "snapshots",
            SidecarKind::Screenshots => "screenshots",
            SidecarKind::Network => "network",
            SidecarKind::Probes => "probes",
            SidecarKind::Perf => "perf",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            SidecarKind::Snapshots => "txt",
            SidecarKind::Screenshots => "png",
            SidecarKind::Network => "json",
            SidecarKind::Probes => "json",
            SidecarKind::Perf => "json",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterSource {
    Default,
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAuditParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: InputType,
    pub sensitive: bool,
    /// Resolved value. Sensitive entries are recorded as the literal
    /// string `"[REDACTED]"` regardless of actual type so the audit
    /// file never carries the secret on disk.
    pub value: Json,
    pub source: ParameterSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAudit {
    pub schema: String, // always "scenario-replay-audit/v1"
    pub run_id: String,
    pub scenario_id: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    /// sha256 of the scenario.json bytes at run start — drift detector.
    pub scenario_content_hash: String,
    /// Resolved input parameters, one row per declared input that
    /// received a value (default or cli). Sensitive values are
    /// `"[REDACTED]"` in this trail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<RunAuditParameter>>,
    /// stepIds whose value was overridden by `--heal-from-run` at
    /// dispatch time. Empty `None` when the flag wasn't set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heal_overrides_applied: Option<Vec<String>>,
    /// Free-form label from `replay --tag`. Useful for grouping runs
    /// across replays (e.g. 'pre-deploy', 'nightly').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl RunAudit {
    pub const SCHEMA_ID: &'static str = "scenario-replay-audit/v1";
}

// ---------- safe-segment ----------

fn is_safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

// ---------- run id ----------

/// Mint a `<runId>` of the form `<isoTimestamp>__<shortHash>[__<profile>]`.
///
/// Timestamp is filesystem-safe (`:` and `.` → `-`). Hash is 8 random
/// lowercase hex chars — independent of the scenario id so two parallel
/// runs against the same scenario can never collide. Profile suffix is
/// appended only when the profile matches the safe-filename regex.
pub fn mint_run_id(profile: Option<&str>) -> String {
    let ts = chrono::Utc::now()
        .format("%Y-%m-%dT%H-%M-%S-%3fZ")
        .to_string();
    let hash = random_hex(4);
    match profile {
        Some(p) if is_safe_segment(p) => format!("{ts}__{hash}__{p}"),
        _ => format!("{ts}__{hash}"),
    }
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes * 2);
    for b in buf {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

// ---------- run root ----------

/// Create the `<scenario_dir>/replays/<run_id>/` directory tree. Per-kind
/// dirs are *not* pre-created; `write_step_sidecar` / `ensure_kind_dir`
/// mkdir lazily so directories that never get written stay absent
/// (matches the spec: absence is meaningful).
pub fn prepare_run_root(scenario_dir: &Path, run_id: &str) -> Result<RunPaths> {
    if !is_safe_segment(run_id) {
        bail!("unsafe runId for sidecar path: {run_id:?}");
    }
    let run_root = scenario_dir.join("replays").join(run_id);
    fs::create_dir_all(&run_root).with_context(|| format!("mkdir -p {}", run_root.display()))?;
    Ok(RunPaths {
        scenario_dir: scenario_dir.to_path_buf(),
        run_id: run_id.to_string(),
        run_root,
    })
}

// ---------- atomic write ----------

/// Atomic write — `<path>.tmp` → rename. Readers ignore `.tmp` files.
pub fn atomic_write_file(abs_path: &Path, contents: &[u8]) -> Result<()> {
    let tmp = with_tmp_suffix(abs_path);
    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        f.write_all(contents)
            .with_context(|| format!("write {}", tmp.display()))?;
        f.sync_all().ok(); // best-effort; not strictly required for correctness
    }
    fs::rename(&tmp, abs_path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), abs_path.display()))?;
    Ok(())
}

fn with_tmp_suffix(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(".tmp");
    PathBuf::from(s)
}

// ---------- per-step sidecars ----------

/// Absolute path for a step-keyed sidecar. Does NOT mkdir.
pub fn step_sidecar_path(run: &RunPaths, kind: SidecarKind, step_id: &str) -> Result<PathBuf> {
    if !is_safe_segment(step_id) {
        bail!("unsafe stepId for sidecar path: {step_id:?}");
    }
    Ok(run
        .run_root
        .join(kind.dir_name())
        .join(format!("{step_id}.{}", kind.extension())))
}

/// Write `<run_root>/<kind>/<step_id>.<ext>` atomically; mkdir the kind dir.
pub fn write_step_sidecar(
    run: &RunPaths,
    kind: SidecarKind,
    step_id: &str,
    contents: &[u8],
) -> Result<()> {
    ensure_kind_dir(run, kind)?;
    atomic_write_file(&step_sidecar_path(run, kind, step_id)?, contents)
}

/// Ensure parent dir exists for a sidecar path (used before binary writes
/// like screenshot capture).
pub fn ensure_kind_dir(run: &RunPaths, kind: SidecarKind) -> Result<PathBuf> {
    let dir = run.run_root.join(kind.dir_name());
    fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    Ok(dir)
}

// ---------- heal jsonl ----------

/// Append one heal event as a JSONL line to `<run_root>/heal.jsonl`.
/// Caller passes the canonical row shape; this helper is a thin
/// formatter so heal pipelines stay free of file I/O glue.
pub fn append_heal_jsonl(run: &RunPaths, row: &Json) -> Result<()> {
    let path = run.run_root.join("heal.jsonl");
    use std::fs::OpenOptions;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    let line = format!("{row}\n");
    f.write_all(line.as_bytes())
        .with_context(|| format!("append {}", path.display()))?;
    Ok(())
}

// ---------- audit ----------

pub fn write_run_audit(run: &RunPaths, audit: &RunAudit) -> Result<()> {
    let body = serde_json::to_string_pretty(audit)?;
    let mut body = body;
    body.push('\n');
    atomic_write_file(&run.run_root.join("audit.json"), body.as_bytes())
}

// ---------- latest pointer ----------

/// Write `<scenario_dir>/replays/latest.txt` carrying the single `runId`.
pub fn update_latest_pointer(scenario_dir: &Path, run_id: &str) -> Result<()> {
    if !is_safe_segment(run_id) {
        bail!("unsafe runId for latest pointer: {run_id:?}");
    }
    let dir = scenario_dir.join("replays");
    fs::create_dir_all(&dir).with_context(|| format!("mkdir -p {}", dir.display()))?;
    atomic_write_file(&dir.join("latest.txt"), format!("{run_id}\n").as_bytes())
}

// ---------- hash ----------

pub fn hash_scenario_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    use std::fmt::Write;
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    // ----- mint_run_id -----

    #[test]
    fn mint_run_id_basic_shape() {
        let id = mint_run_id(None);
        // 2026-05-19T16-23-45-123Z__abcd1234
        let parts: Vec<&str> = id.split("__").collect();
        assert_eq!(parts.len(), 2, "expected exactly one __ separator in {id}");
        assert!(parts[0].ends_with('Z'));
        assert_eq!(parts[1].len(), 8);
        assert!(parts[1].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn mint_run_id_with_safe_profile_appends_suffix() {
        let id = mint_run_id(Some("acme-user"));
        let parts: Vec<&str> = id.split("__").collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[2], "acme-user");
    }

    #[test]
    fn mint_run_id_with_unsafe_profile_omits_suffix() {
        let id = mint_run_id(Some("../escape"));
        let parts: Vec<&str> = id.split("__").collect();
        assert_eq!(parts.len(), 2, "unsafe profile should be dropped");
    }

    #[test]
    fn mint_run_id_is_unique_across_calls() {
        let a = mint_run_id(None);
        let b = mint_run_id(None);
        assert_ne!(a, b, "consecutive run ids must differ (random hash)");
    }

    // ----- prepare_run_root -----

    #[test]
    fn prepare_run_root_creates_dirs_lazily() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().join("sid");
        let rp = prepare_run_root(&jdir, "r1").unwrap();
        assert!(rp.run_root.is_dir());
        assert_eq!(rp.run_root, jdir.join("replays").join("r1"));
        // None of the per-kind dirs are pre-created.
        assert!(!rp.run_root.join("snapshots").exists());
    }

    #[test]
    fn prepare_run_root_rejects_unsafe_run_id() {
        let tmp = TempDir::new().unwrap();
        prepare_run_root(tmp.path(), "../x").unwrap_err();
    }

    // ----- atomic_write_file -----

    #[test]
    fn atomic_write_leaves_no_tmp_behind() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a/b/c.txt");
        atomic_write_file(&p, b"hello").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "hello");
        let tmp_path = with_tmp_suffix(&p);
        assert!(!tmp_path.exists(), ".tmp must be renamed away");
    }

    #[test]
    fn atomic_write_overwrites_existing() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.txt");
        atomic_write_file(&p, b"first").unwrap();
        atomic_write_file(&p, b"second").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "second");
    }

    // ----- step_sidecar_path -----

    #[test]
    fn step_sidecar_path_combines_safely() {
        let tmp = TempDir::new().unwrap();
        let rp = prepare_run_root(tmp.path(), "r1").unwrap();
        let p = step_sidecar_path(&rp, SidecarKind::Probes, "step.42").unwrap();
        assert!(p.ends_with("replays/r1/probes/step.42.json"));
    }

    #[test]
    fn step_sidecar_path_rejects_unsafe_step_id() {
        let tmp = TempDir::new().unwrap();
        let rp = prepare_run_root(tmp.path(), "r1").unwrap();
        step_sidecar_path(&rp, SidecarKind::Probes, "../escape").unwrap_err();
    }

    #[test]
    fn write_step_sidecar_creates_kind_dir() {
        let tmp = TempDir::new().unwrap();
        let rp = prepare_run_root(tmp.path(), "r1").unwrap();
        write_step_sidecar(&rp, SidecarKind::Snapshots, "s1", b"snap-text").unwrap();
        let path = rp.run_root.join("snapshots").join("s1.txt");
        assert_eq!(fs::read_to_string(path).unwrap(), "snap-text");
    }

    #[test]
    fn ensure_kind_dir_creates_dir() {
        let tmp = TempDir::new().unwrap();
        let rp = prepare_run_root(tmp.path(), "r1").unwrap();
        let dir = ensure_kind_dir(&rp, SidecarKind::Network).unwrap();
        assert!(dir.is_dir());
        assert_eq!(dir, rp.run_root.join("network"));
    }

    // ----- heal jsonl -----

    #[test]
    fn append_heal_jsonl_creates_and_appends() {
        let tmp = TempDir::new().unwrap();
        let rp = prepare_run_root(tmp.path(), "r1").unwrap();
        append_heal_jsonl(&rp, &json!({ "step": "s1", "outcome": "absorb" })).unwrap();
        append_heal_jsonl(&rp, &json!({ "step": "s2", "outcome": "patch" })).unwrap();
        let body = fs::read_to_string(rp.run_root.join("heal.jsonl")).unwrap();
        let lines: Vec<&str> = body.trim_end().split('\n').collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"step\":\"s1\""));
        assert!(lines[1].contains("\"step\":\"s2\""));
    }

    // ----- audit -----

    #[test]
    fn write_run_audit_serializes_camel_case() {
        let tmp = TempDir::new().unwrap();
        let rp = prepare_run_root(tmp.path(), "r1").unwrap();
        let audit = RunAudit {
            schema: RunAudit::SCHEMA_ID.to_string(),
            run_id: "r1".into(),
            scenario_id: "j1".into(),
            started_at: "2026-01-01T00:00:00-000Z".into(),
            finished_at: Some("2026-01-01T00:01:00-000Z".into()),
            exit_code: Some(0),
            summary: Some("PASS".into()),
            profile: Some("acme-user".into()),
            session_name: None,
            scenario_content_hash: "deadbeef".into(),
            parameters: None,
            heal_overrides_applied: None,
            tag: None,
        };
        write_run_audit(&rp, &audit).unwrap();
        let body = fs::read_to_string(rp.run_root.join("audit.json")).unwrap();
        assert!(body.ends_with('\n'));
        let parsed: Json = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["runId"], "r1");
        assert_eq!(parsed["scenarioContentHash"], "deadbeef");
        assert_eq!(parsed["scenarioId"], "j1");
        assert!(
            parsed.get("sessionName").is_none(),
            "None field must be skipped"
        );
    }

    #[test]
    fn audit_parameter_redacted_secret_serializes() {
        let p = RunAuditParameter {
            name: "password".into(),
            ty: InputType::String,
            sensitive: true,
            value: json!("[REDACTED]"),
            source: ParameterSource::Cli,
        };
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"sensitive\":true"));
        assert!(s.contains("\"value\":\"[REDACTED]\""));
        assert!(s.contains("\"source\":\"cli\""));
    }

    // ----- latest pointer -----

    #[test]
    fn update_latest_pointer_writes_single_line() {
        let tmp = TempDir::new().unwrap();
        update_latest_pointer(tmp.path(), "r-abc").unwrap();
        let body = fs::read_to_string(tmp.path().join("replays").join("latest.txt")).unwrap();
        assert_eq!(body, "r-abc\n");
    }

    #[test]
    fn update_latest_pointer_rejects_unsafe_run_id() {
        let tmp = TempDir::new().unwrap();
        update_latest_pointer(tmp.path(), "../x").unwrap_err();
    }

    // ----- hash -----

    #[test]
    fn hash_scenario_bytes_returns_sha256_hex() {
        // SHA-256("") is e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let empty = hash_scenario_bytes(b"");
        assert_eq!(
            empty,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let hello = hash_scenario_bytes(b"hello");
        assert_eq!(hello.len(), 64);
        assert!(hello.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
