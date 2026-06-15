//! `truncate` verb — drop in-flight steps ≥ N and archive their sidecars.
//!
//! Pure disk bookkeeping: never drives the live tab. Useful when a
//! recording goes off the rails — fix the live tab yourself with
//! agent-browser primitives, then `truncate <N>` so the next
//! `record-step` appends cleanly at index N.
//!
//! Behaviour:
//!
//!   1. Read `<record_root>/scenario.steps.jsonl`. Keep the first N rows;
//!      collect the dropped rows' `stepId`s.
//!   2. mkdir `<sid>/recording/failed/truncate-<isoTs>[-<tag>]/{snapshots,
//!      screenshots,probes,network}/`
//!   3. Move `<sid>/recording/<kind>/<stepId>.<ext>` into the archive for
//!      every dropped stepId × kind that exists. (Move, not copy — the
//!      live recording must not refer to dropped step ids.)
//!   4. Write `manifest.json` with the SID, timestamp, toStepIndex,
//!      droppedStepIds[], movedFiles[].
//!   5. Truncate `scenario.steps.jsonl` in place.

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::Value as Json;

use crate::paths;
use crate::sidecar::atomic_write_file;

const ARCHIVE_KINDS: &[&str] = &["snapshots", "screenshots", "probes", "network"];
const ARCHIVE_EXTENSIONS: &[(&str, &str)] = &[
    ("snapshots", "txt"),
    ("screenshots", "png"),
    ("probes", "json"),
    ("network", "json"),
];

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;
    let summary = truncate(&opts)?;
    println!(
        "truncated to {} step(s); dropped {} row(s); moved {} sidecar file(s)",
        opts.to_step_index,
        summary.dropped_step_ids.len(),
        summary.moved_files.len(),
    );
    println!("archive: {}", summary.archive_dir.display());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa truncate — drop steps ≥ N and archive their sidecars

Usage:
  agent-qa truncate <toStepIndex> [--archive-tag <slug>]

Effect (pure disk; never touches the live tab):
  - Drops rows ≥ N from <record_root>/scenario.steps.jsonl
  - Moves matching <sid>/recording/<kind>/<stepId>.<ext> files to
    <sid>/recording/failed/truncate-<isoTs>[-<tag>]/<kind>/<stepId>.<ext>
  - Writes a manifest.json under the archive dir"
    );
}

#[derive(Debug, Clone)]
struct Opts {
    to_step_index: u32,
    archive_tag: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema: &'static str,
    sid: String,
    timestamp: String,
    to_step_index: u32,
    dropped_step_ids: Vec<String>,
    moved_files: Vec<String>,
    archive_tag: Option<String>,
}

#[derive(Debug)]
struct Summary {
    dropped_step_ids: Vec<String>,
    moved_files: Vec<String>,
    archive_dir: PathBuf,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut to_step_index: Option<u32> = None;
    let mut archive_tag: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--archive-tag" => archive_tag = it.next().cloned(),
            s if s.starts_with("--archive-tag=") => {
                archive_tag = Some(s["--archive-tag=".len()..].to_string())
            }
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if to_step_index.is_some() {
                    bail!("unexpected positional {other:?}; usage: truncate <toStepIndex>");
                }
                let n: i64 = other.parse().map_err(|_| {
                    anyhow!("toStepIndex must be a non-negative integer; got {other:?}")
                })?;
                if n < 0 {
                    bail!("toStepIndex must be non-negative; got {n}");
                }
                to_step_index = Some(n as u32);
            }
        }
    }
    let to_step_index = to_step_index
        .ok_or_else(|| anyhow!("usage: truncate <toStepIndex> [--archive-tag <slug>]"))?;
    if let Some(tag) = &archive_tag {
        if !is_safe_segment(tag) {
            bail!("--archive-tag must match [A-Za-z0-9._-]+; got {tag:?}");
        }
    }
    Ok(Opts {
        to_step_index,
        archive_tag,
    })
}

fn is_safe_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn truncate(opts: &Opts) -> Result<Summary> {
    let env_file = paths::record_env_file();
    let env_body = fs::read_to_string(&env_file)
        .with_context(|| format!("read {} (was `start` run?)", env_file.display()))?;
    let sid = env_body
        .lines()
        .find_map(|l| l.strip_prefix("SID=").map(|v| v.trim().to_string()))
        .ok_or_else(|| anyhow!("{} has no SID= line", env_file.display()))?;

    let buffer_path = paths::record_steps_jsonl();
    let body = fs::read_to_string(&buffer_path).unwrap_or_default();
    let mut keep_lines: Vec<String> = Vec::new();
    let mut dropped_rows: Vec<Json> = Vec::new();
    for (idx, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if (idx as u32) < opts.to_step_index {
            keep_lines.push(line.to_string());
            continue;
        }
        match serde_json::from_str::<Json>(line) {
            Ok(row) => dropped_rows.push(row),
            Err(_) => dropped_rows.push(Json::Null), // tolerate malformed; archive intent
        }
    }

    let dropped_step_ids: Vec<String> = dropped_rows
        .iter()
        .filter_map(|r| r.get("stepId").and_then(|v| v.as_str()).map(str::to_string))
        .collect();

    let scenario_dir = paths::scenario_dir(&sid)?;
    let ts = chrono::Utc::now()
        .format("%Y-%m-%dT%H-%M-%S-%3fZ")
        .to_string();
    let folder = match &opts.archive_tag {
        Some(t) => format!("truncate-{ts}-{t}"),
        None => format!("truncate-{ts}"),
    };
    let archive_dir = scenario_dir.join("recording").join("failed").join(&folder);

    let mut moved_files: Vec<String> = Vec::new();
    if !dropped_step_ids.is_empty() {
        for kind in ARCHIVE_KINDS {
            fs::create_dir_all(archive_dir.join(kind)).ok();
        }
        for step_id in &dropped_step_ids {
            for (kind, ext) in ARCHIVE_EXTENSIONS {
                let src = scenario_dir
                    .join("recording")
                    .join(kind)
                    .join(format!("{step_id}.{ext}"));
                if src.is_file() {
                    let dst = archive_dir.join(kind).join(format!("{step_id}.{ext}"));
                    fs::rename(&src, &dst).with_context(|| {
                        format!("rename {} -> {}", src.display(), dst.display())
                    })?;
                    moved_files.push(format!("{kind}/{step_id}.{ext}"));
                }
            }
        }
    }

    // Manifest under the archive dir (even if empty — provides forensic
    // record that a truncate ran).
    fs::create_dir_all(&archive_dir).ok();
    let manifest = Manifest {
        schema: "truncate-archive/v1",
        sid: sid.clone(),
        timestamp: ts.clone(),
        to_step_index: opts.to_step_index,
        dropped_step_ids: dropped_step_ids.clone(),
        moved_files: moved_files.clone(),
        archive_tag: opts.archive_tag.clone(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let mut manifest_bytes = manifest_bytes;
    manifest_bytes.push(b'\n');
    atomic_write_file(&archive_dir.join("manifest.json"), &manifest_bytes)?;

    // Truncate the buffer in place.
    let mut new_body = keep_lines.join("\n");
    if !new_body.is_empty() {
        new_body.push('\n');
    }
    fs::write(&buffer_path, new_body.as_bytes())
        .with_context(|| format!("write {}", buffer_path.display()))?;

    Ok(Summary {
        dropped_step_ids,
        moved_files,
        archive_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
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

    fn write_env(rec: &Path, sid: &str) {
        fs::create_dir_all(rec).unwrap();
        fs::write(rec.join("scenario.env"), format!("SID={sid}\n")).unwrap();
    }
    fn append_row(rec: &Path, row: serde_json::Value) {
        let p = rec.join("scenario.steps.jsonl");
        let mut body = fs::read_to_string(&p).unwrap_or_default();
        body.push_str(&serde_json::to_string(&row).unwrap());
        body.push('\n');
        fs::write(&p, body).unwrap();
    }
    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"").unwrap();
    }

    #[test]
    fn truncate_drops_rows_and_moves_sidecars() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "j1");
        for i in 0..3u32 {
            append_row(
                &rec,
                json!({
                    "stepIndex": i, "stepId": format!("s{i}"), "kind": "do",
                    "payload": { "intent": "x", "verb": "reload" },
                    "recordedAt": "now"
                }),
            );
        }
        let jdir = tmp.path().join("j1");
        for i in 0..3u32 {
            touch(&jdir.join("recording/snapshots").join(format!("s{i}.txt")));
            touch(&jdir.join("recording/screenshots").join(format!("s{i}.png")));
        }

        let opts = Opts {
            to_step_index: 1,
            archive_tag: None,
        };
        let sum = truncate(&opts).unwrap();
        assert_eq!(
            sum.dropped_step_ids,
            vec!["s1".to_string(), "s2".to_string()]
        );

        // Buffer keeps only s0.
        let body = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\"stepId\":\"s0\""));

        // Sidecars moved.
        assert!(!jdir.join("recording/snapshots/s1.txt").exists());
        assert!(sum.archive_dir.join("snapshots/s1.txt").is_file());
        assert!(!jdir.join("recording/screenshots/s2.png").exists());
        assert!(sum.archive_dir.join("screenshots/s2.png").is_file());

        // Manifest written.
        let m: Json =
            serde_json::from_slice(&fs::read(sum.archive_dir.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(m["schema"], "truncate-archive/v1");
        assert_eq!(m["toStepIndex"], 1);
        assert_eq!(m["droppedStepIds"].as_array().unwrap().len(), 2);

        teardown();
    }

    #[test]
    fn truncate_with_archive_tag_appears_in_folder_and_manifest() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "j1");
        append_row(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "do",
                "payload": { "intent": "x", "verb": "reload" },
                "recordedAt": "now"
            }),
        );
        let opts = Opts {
            to_step_index: 0,
            archive_tag: Some("attempt2".into()),
        };
        let sum = truncate(&opts).unwrap();
        assert!(
            sum.archive_dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("attempt2"),
            "got: {}",
            sum.archive_dir.display()
        );
        let m: Json =
            serde_json::from_slice(&fs::read(sum.archive_dir.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(m["archiveTag"], "attempt2");
        teardown();
    }

    #[test]
    fn truncate_idempotent_when_index_matches_buffer_length() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        setup(tmp.path());
        let rec = tmp.path().join("rec");
        write_env(&rec, "j1");
        append_row(
            &rec,
            json!({
                "stepIndex": 0, "stepId": "s0", "kind": "do",
                "payload": { "intent": "x", "verb": "reload" },
                "recordedAt": "now"
            }),
        );
        let opts = Opts {
            to_step_index: 1,
            archive_tag: None,
        };
        let sum = truncate(&opts).unwrap();
        assert!(sum.dropped_step_ids.is_empty());
        // Manifest still written (forensic).
        assert!(sum.archive_dir.join("manifest.json").is_file());
        teardown();
    }

    #[test]
    fn parse_args_rejects_negative() {
        parse_args(&["-1".into()]).unwrap_err();
    }

    #[test]
    fn parse_args_rejects_unsafe_archive_tag() {
        parse_args(&["1".into(), "--archive-tag".into(), "../escape".into()]).unwrap_err();
    }
}
