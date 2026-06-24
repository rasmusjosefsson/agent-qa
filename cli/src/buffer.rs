//! `buffer` verb — inspect / reorder / delete rows in the in-flight buffer.
//!
//! The authoring editor (L3) renders the recording buffer as a list of
//! step cards and needs to reorder and delete them. Those operations
//! re-index `stepIndex` / `stepId` so the buffer stays a clean, ordered
//! `s0, s1, …` sequence that `flush` can translate unchanged. Keeping the
//! re-indexing here (not in the Node launcher) keeps all contract-shaping
//! logic in Rust; the buffer itself is ephemeral recording scratch under
//! `<record_root>/scenario.steps.jsonl`.
//!
//! Pure disk bookkeeping: never drives the live tab. Sidecars captured by
//! `record-step` under `<sid>/recording/<kind>/<stepId>.<ext>` are NOT
//! moved — they are keyed by the original stepId and the editor only uses
//! them for author-time preview, so a reindex intentionally leaves them in
//! place rather than risk clobbering. (`truncate` is the verb that archives
//! sidecars when dropping a tail.)
//!
//! CLI shape:
//!
//!   agent-qa buffer list [--json]
//!   agent-qa buffer delete <index>
//!   agent-qa buffer move <from> <to>
//!   agent-qa buffer clear

use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::paths;

pub fn run(args: &[String]) -> Result<u8> {
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return Ok(0);
    }
    match args[0].as_str() {
        "list" => cmd_list(&args[1..]),
        "delete" | "rm" => cmd_delete(&args[1..]),
        "move" | "mv" => cmd_move(&args[1..]),
        "clear" => cmd_clear(),
        other => bail!("buffer: unknown subcommand {other:?}; try list|delete|move|clear"),
    }
}

fn print_help() {
    println!(
        "agent-qa buffer — inspect / reorder / delete the in-flight buffer

Usage:
  agent-qa buffer list [--json]      List buffered rows (id, kind, summary)
  agent-qa buffer delete <index>     Drop one row; re-index the rest
  agent-qa buffer move <from> <to>   Move a row to a new position; re-index
  agent-qa buffer clear              Empty the buffer

Indices are 0-based and match the row's current position. After delete or
move, every row's stepIndex / stepId is renumbered s0, s1, … so `flush`
emits a clean, ordered scenario.json.

Pure disk bookkeeping; never drives the live tab."
    );
}

fn read_rows() -> Result<Vec<Json>> {
    let path = paths::record_steps_jsonl();
    let body = match fs::read_to_string(&path) {
        Ok(b) => b,
        Err(_) => return Ok(Vec::new()),
    };
    let mut rows = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Json = serde_json::from_str(line)
            .with_context(|| format!("parse buffer row at line {}", i + 1))?;
        rows.push(row);
    }
    Ok(rows)
}

/// Renumber rows in place so stepIndex/stepId are a dense `s0..sN` run,
/// then write them back to the buffer atomically-ish (truncate + write).
fn write_rows(rows: &[Json]) -> Result<()> {
    let path = paths::record_steps_jsonl();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        let mut row = row.clone();
        if let Some(obj) = row.as_object_mut() {
            obj.insert("stepIndex".into(), json!(i));
            obj.insert("stepId".into(), json!(format!("s{i}")));
        }
        out.push_str(&serde_json::to_string(&row)?);
        out.push('\n');
    }
    fs::write(&path, out.as_bytes()).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn row_summary(row: &Json) -> Json {
    let kind = row.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
    let payload = row.get("payload").cloned().unwrap_or(Json::Null);
    json!({
        "stepId": row.get("stepId").and_then(|v| v.as_str()).unwrap_or(""),
        "stepIndex": row.get("stepIndex").and_then(|v| v.as_i64()),
        "kind": kind,
        "payload": payload,
    })
}

fn cmd_list(args: &[String]) -> Result<u8> {
    let json_out = args.iter().any(|a| a == "--json");
    let rows = read_rows()?;
    if json_out {
        let summaries: Vec<Json> = rows.iter().map(row_summary).collect();
        println!("{}", serde_json::to_string(&json!({ "rows": summaries }))?);
    } else if rows.is_empty() {
        println!("(buffer is empty)");
    } else {
        for (i, row) in rows.iter().enumerate() {
            let kind = row.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            let payload = row.get("payload").cloned().unwrap_or(Json::Null);
            println!("[{i}] {kind}: {}", compact(&payload));
        }
    }
    Ok(0)
}

fn compact(v: &Json) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "<unprintable>".into())
}

fn parse_index(s: &str, what: &str) -> Result<usize> {
    s.parse::<usize>()
        .map_err(|_| anyhow!("{what} must be a non-negative integer; got {s:?}"))
}

fn cmd_delete(args: &[String]) -> Result<u8> {
    let idx = args
        .first()
        .ok_or_else(|| anyhow!("usage: buffer delete <index>"))?;
    let idx = parse_index(idx, "index")?;
    let mut rows = read_rows()?;
    if idx >= rows.len() {
        bail!(
            "index {idx} out of range (buffer has {} row(s))",
            rows.len()
        );
    }
    rows.remove(idx);
    write_rows(&rows)?;
    println!("deleted row {idx}; {} row(s) remain", rows.len());
    Ok(0)
}

fn cmd_move(args: &[String]) -> Result<u8> {
    if args.len() < 2 {
        bail!("usage: buffer move <from> <to>");
    }
    let from = parse_index(&args[0], "from")?;
    let to = parse_index(&args[1], "to")?;
    let mut rows = read_rows()?;
    let n = rows.len();
    if from >= n {
        bail!("from index {from} out of range (buffer has {n} row(s))");
    }
    if to >= n {
        bail!("to index {to} out of range (buffer has {n} row(s))");
    }
    let row = rows.remove(from);
    rows.insert(to, row);
    write_rows(&rows)?;
    println!("moved row {from} → {to}; {n} row(s)");
    Ok(0)
}

fn cmd_clear() -> Result<u8> {
    let path = paths::record_steps_jsonl();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    write_empty(&path)?;
    println!("buffer cleared");
    Ok(0)
}

fn write_empty(path: &Path) -> Result<()> {
    fs::write(path, b"").with_context(|| format!("truncate {}", path.display()))?;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    fn seed(rec: &Path, kinds: &[&str]) {
        fs::create_dir_all(rec).unwrap();
        let mut body = String::new();
        for (i, k) in kinds.iter().enumerate() {
            body.push_str(
                &serde_json::to_string(&json!({
                    "stepIndex": i,
                    "stepId": format!("s{i}"),
                    "kind": k,
                    "payload": { "n": i },
                    "recordedAt": "now",
                }))
                .unwrap(),
            );
            body.push('\n');
        }
        fs::write(rec.join("scenario.steps.jsonl"), body).unwrap();
    }

    fn rows_now() -> Vec<Json> {
        read_rows().unwrap()
    }

    #[test]
    fn delete_reindexes_remaining_rows() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        seed(&rec, &["navigation", "action", "assert"]);

        cmd_delete(&["1".into()]).unwrap();
        let rows = rows_now();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["kind"], "navigation");
        assert_eq!(rows[1]["kind"], "assert");
        // re-indexed s0, s1.
        assert_eq!(rows[0]["stepId"], "s0");
        assert_eq!(rows[1]["stepId"], "s1");
        assert_eq!(rows[1]["stepIndex"], 1);

        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn move_reorders_and_reindexes() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        seed(&rec, &["navigation", "action", "assert"]);

        // Move the last row to the front.
        cmd_move(&["2".into(), "0".into()]).unwrap();
        let rows = rows_now();
        assert_eq!(rows[0]["kind"], "assert");
        assert_eq!(rows[1]["kind"], "navigation");
        assert_eq!(rows[2]["kind"], "action");
        assert_eq!(rows[0]["stepId"], "s0");
        assert_eq!(rows[2]["stepId"], "s2");

        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn delete_out_of_range_errors() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        seed(&rec, &["navigation"]);
        cmd_delete(&["5".into()]).unwrap_err();
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn clear_empties_buffer() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        seed(&rec, &["navigation", "action"]);
        cmd_clear().unwrap();
        assert!(rows_now().is_empty());
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }

    #[test]
    fn list_json_emits_rows() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        seed(&rec, &["navigation", "assert"]);
        // Just assert it doesn't error + parses the buffer.
        let rows = read_rows().unwrap();
        assert_eq!(rows.len(), 2);
        let summaries: Vec<Json> = rows.iter().map(row_summary).collect();
        assert_eq!(summaries[0]["kind"], "navigation");
        assert_eq!(summaries[1]["kind"], "assert");
        std::env::remove_var(paths::RECORD_DIR_ENV);
    }
}
