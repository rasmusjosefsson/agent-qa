//! `truncate` drops active steps at and after an index and archives sidecars.

use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

use crate::recorder_state::RecorderState;
use crate::sidecar::atomic_write_file;

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
        "truncated to {} step(s); dropped {} step(s); moved {} sidecar file(s)",
        opts.to_step_index,
        summary.dropped_step_ids.len(),
        summary.moved_files.len()
    );
    println!("archive: {}", summary.archive_dir.display());
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa truncate - drop active steps at and after N

Usage:
  agent-qa truncate <toStepIndex> [--archive-tag <slug>]

Moves matching recording sidecars into a timestamped archive. It never drives the browser."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    to_step_index: usize,
    archive_tag: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema: &'static str,
    sid: String,
    timestamp: String,
    to_step_index: usize,
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
    let mut index = None;
    let mut tag = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--archive-tag" => tag = it.next().cloned(),
            value if value.starts_with("--archive-tag=") => {
                tag = Some(value["--archive-tag=".len()..].to_string())
            }
            value if value.starts_with("--") => bail!("unknown flag {value:?}"),
            value => {
                if index.is_some() {
                    bail!("unexpected positional {value:?}; usage: truncate <toStepIndex>");
                }
                index = Some(value.parse::<usize>().map_err(|_| {
                    anyhow!("toStepIndex must be a non-negative integer; got {value:?}")
                })?);
            }
        }
    }
    let to_step_index =
        index.ok_or_else(|| anyhow!("usage: truncate <toStepIndex> [--archive-tag <slug>]"))?;
    if tag.as_deref().is_some_and(|value| !is_safe_segment(value)) {
        bail!("--archive-tag must match [A-Za-z0-9._-]+");
    }
    Ok(Opts {
        to_step_index,
        archive_tag: tag,
    })
}

fn is_safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn truncate(opts: &Opts) -> Result<Summary> {
    let mut state = RecorderState::load_active()?;
    let split_at = opts.to_step_index.min(state.steps.len());
    let dropped = state.steps.split_off(split_at);
    let dropped_step_ids: Vec<_> = dropped.iter().map(|step| step.id().to_string()).collect();
    let scenario_dir = crate::paths::scenario_dir(&state.sid)?;
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%dT%H-%M-%S-%3fZ")
        .to_string();
    let suffix = opts
        .archive_tag
        .as_deref()
        .map(|tag| format!("-{tag}"))
        .unwrap_or_default();
    let archive_dir = scenario_dir
        .join("recording/failed")
        .join(format!("truncate-{timestamp}{suffix}"));
    let mut moved_files = Vec::new();
    for step_id in &dropped_step_ids {
        for (kind, ext) in ARCHIVE_EXTENSIONS {
            let source = scenario_dir
                .join("recording")
                .join(kind)
                .join(format!("{step_id}.{ext}"));
            if source.is_file() {
                let destination = archive_dir.join(kind).join(format!("{step_id}.{ext}"));
                fs::create_dir_all(destination.parent().unwrap())?;
                fs::rename(&source, &destination).with_context(|| {
                    format!("rename {} -> {}", source.display(), destination.display())
                })?;
                moved_files.push(format!("{kind}/{step_id}.{ext}"));
            }
        }
    }
    fs::create_dir_all(&archive_dir)?;
    let manifest = Manifest {
        schema: "truncate-archive/v1",
        sid: state.sid.clone(),
        timestamp,
        to_step_index: opts.to_step_index,
        dropped_step_ids: dropped_step_ids.clone(),
        moved_files: moved_files.clone(),
        archive_tag: opts.archive_tag.clone(),
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    atomic_write_file(&archive_dir.join("manifest.json"), &bytes)?;
    state.save()?;
    Ok(Summary {
        dropped_step_ids,
        moved_files,
        archive_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserConnection;
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    #[test]
    fn truncate_updates_state_and_moves_sidecars() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(crate::paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path().join("record"));
        let mut state = RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        );
        state.steps = serde_json::from_value(serde_json::json!([
            {"id":"s0","intent":"first","kind":"do","verb":"reload"},
            {"id":"s1","intent":"second","kind":"do","verb":"reload"}
        ]))
        .unwrap();
        state.save().unwrap();
        let screenshot = tmp.path().join("s1/recording/screenshots/s1.png");
        fs::create_dir_all(screenshot.parent().unwrap()).unwrap();
        fs::write(&screenshot, b"png").unwrap();
        let summary = truncate(&Opts {
            to_step_index: 1,
            archive_tag: None,
        })
        .unwrap();
        assert_eq!(summary.dropped_step_ids, ["s1"]);
        assert_eq!(RecorderState::load_active().unwrap().steps.len(), 1);
        assert!(!screenshot.exists());
        assert!(summary.archive_dir.join("screenshots/s1.png").is_file());
        std::env::remove_var(crate::paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(crate::paths::RECORD_DIR_ENV);
    }
}
