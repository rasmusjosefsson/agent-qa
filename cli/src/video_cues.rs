//! Step timestamps for the replay scrubber.
//!
//! When `--record-video` actually starts, the runner writes
//! `<run>/video-cues.json`: one cue per step, in milliseconds since the
//! recording began. The workbench drops a marker on the timeline at that
//! moment and seeks there. Missing or unreadable cues never fail a run —
//! the scrubber then estimates positions from step durations.

use anyhow::Result;
use serde::Serialize;
use std::path::Path;

use crate::sidecar::atomic_write_file;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoCue {
    pub id: String,
    pub idx: u32,
    pub t_ms: u64,
}

pub fn write(path: &Path, cues: &[VideoCue]) -> Result<()> {
    let body = serde_json::json!({ "cues": cues });
    let mut bytes = serde_json::to_vec_pretty(&body)?;
    bytes.push(b'\n');
    atomic_write_file(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_roundtrips_step_offsets() {
        let dir = std::env::temp_dir().join(format!(
            "agent-qa-cues-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("video-cues.json");
        write(
            &path,
            &[
                VideoCue {
                    id: "s1".into(),
                    idx: 1,
                    t_ms: 400,
                },
                VideoCue {
                    id: "s2".into(),
                    idx: 2,
                    t_ms: 1800,
                },
            ],
        )
        .unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed["cues"][0]["id"], "s1");
        assert_eq!(parsed["cues"][0]["idx"], 1);
        assert_eq!(parsed["cues"][0]["tMs"], 400);
        assert_eq!(parsed["cues"][1]["tMs"], 1800);
        std::fs::remove_dir_all(&dir).ok();
    }
}
