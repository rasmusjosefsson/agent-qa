//! Screenshot diff — pixel comparison between two replay runs.
//!
//! For each stepId that has a `<run>/screenshots/<stepId>.png` in BOTH
//! runs, we decode both PNGs, compare pixel-by-pixel, and (when the
//! differing fraction exceeds the threshold) write a delta-map PNG to
//! `<out_dir>/screenshots/<stepId>.diff.png` highlighting the diffs in
//! red against a faded version of the baseline.
//!
//! Differing pixel = any channel (R/G/B/A) differs from its counterpart.
//! `--pixel-threshold` lets the caller tolerate up to a given fraction
//! of differing pixels (0 = exact match required).
//!
//! Size mismatch is its own outcome — we don't try to align differently
//! sized screenshots.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::{ImageReader, Rgba, RgbaImage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ShotOutcome {
    Same,
    Changed,
    SizeMismatch,
    OnlyA,
    OnlyB,
}

impl ShotOutcome {
    pub(super) fn label(&self) -> &'static str {
        match self {
            ShotOutcome::Same => "SAME",
            ShotOutcome::Changed => "CHANGED",
            ShotOutcome::SizeMismatch => "SIZE-DIFF",
            ShotOutcome::OnlyA => "ONLY-A",
            ShotOutcome::OnlyB => "ONLY-B",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ShotEntry {
    pub step_id: String,
    pub outcome: ShotOutcome,
    /// Fraction of differing pixels in [0, 1]. None for outcomes where
    /// the comparison wasn't done (Only-A, Only-B, Size-Mismatch).
    pub differing_fraction: Option<f64>,
}

#[derive(Debug, Clone)]
pub(super) struct ShotReport {
    pub entries: Vec<ShotEntry>,
}

pub(super) fn build(
    scenario_dir: &Path,
    run_a: &str,
    run_b: &str,
    out_dir: &Path,
    threshold: f64,
) -> Result<ShotReport> {
    let dir_a = scenario_dir.join("replays").join(run_a).join("screenshots");
    let dir_b = scenario_dir.join("replays").join(run_b).join("screenshots");

    let mut ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if dir_a.is_dir() {
        for e in fs::read_dir(&dir_a)?.flatten() {
            if let Some(stem) = e.path().file_stem().and_then(|s| s.to_str()) {
                ids.insert(stem.to_string());
            }
        }
    }
    if dir_b.is_dir() {
        for e in fs::read_dir(&dir_b)?.flatten() {
            if let Some(stem) = e.path().file_stem().and_then(|s| s.to_str()) {
                ids.insert(stem.to_string());
            }
        }
    }

    let mut entries: Vec<ShotEntry> = Vec::new();
    let mut diff_out_dir: Option<PathBuf> = None;
    for id in ids {
        let pa = dir_a.join(format!("{id}.png"));
        let pb = dir_b.join(format!("{id}.png"));
        match (pa.is_file(), pb.is_file()) {
            (true, true) => {
                let a = decode_png(&pa)?;
                let b = decode_png(&pb)?;
                if a.dimensions() != b.dimensions() {
                    entries.push(ShotEntry {
                        step_id: id,
                        outcome: ShotOutcome::SizeMismatch,
                        differing_fraction: None,
                    });
                    continue;
                }
                let (frac, diff_img) = pixel_diff(&a, &b);
                if frac <= threshold {
                    entries.push(ShotEntry {
                        step_id: id,
                        outcome: ShotOutcome::Same,
                        differing_fraction: Some(frac),
                    });
                } else {
                    // Write the diff PNG.
                    let dod = diff_out_dir.get_or_insert_with(|| {
                        let p = out_dir.join("screenshots");
                        let _ = fs::create_dir_all(&p);
                        p
                    });
                    let out_path = dod.join(format!("{id}.diff.png"));
                    diff_img
                        .save(&out_path)
                        .with_context(|| format!("write diff png {}", out_path.display()))?;
                    entries.push(ShotEntry {
                        step_id: id,
                        outcome: ShotOutcome::Changed,
                        differing_fraction: Some(frac),
                    });
                }
            }
            (true, false) => entries.push(ShotEntry {
                step_id: id,
                outcome: ShotOutcome::OnlyA,
                differing_fraction: None,
            }),
            (false, true) => entries.push(ShotEntry {
                step_id: id,
                outcome: ShotOutcome::OnlyB,
                differing_fraction: None,
            }),
            (false, false) => {}
        }
    }

    Ok(ShotReport { entries })
}

fn decode_png(path: &Path) -> Result<RgbaImage> {
    let img = ImageReader::open(path)
        .with_context(|| format!("open {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("guess format {}", path.display()))?
        .decode()
        .with_context(|| format!("decode {}", path.display()))?;
    Ok(img.to_rgba8())
}

/// Returns (differing-fraction, delta-map image). Delta map shows the
/// baseline (`a`) faded to 50% greyscale; differing pixels are red.
fn pixel_diff(a: &RgbaImage, b: &RgbaImage) -> (f64, RgbaImage) {
    let (w, h) = a.dimensions();
    let total = (w as u64) * (h as u64);
    let mut diff = RgbaImage::new(w, h);
    let mut differing: u64 = 0;
    for y in 0..h {
        for x in 0..w {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            if pa != pb {
                differing += 1;
                diff.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            } else {
                let g = ((pa[0] as u16 + pa[1] as u16 + pa[2] as u16) / 3) as u8;
                let faded = g / 2 + 64;
                diff.put_pixel(x, y, Rgba([faded, faded, faded, 255]));
            }
        }
    }
    let frac = if total == 0 {
        0.0
    } else {
        differing as f64 / total as f64
    };
    (frac, diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use tempfile::TempDir;

    fn write_png(path: &Path, w: u32, h: u32, fill: Rgba<u8>) {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, fill);
            }
        }
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        img.save(path).unwrap();
    }

    #[test]
    fn pixel_diff_identical_returns_zero() {
        let mut a = RgbaImage::new(2, 2);
        let mut b = RgbaImage::new(2, 2);
        for y in 0..2 {
            for x in 0..2 {
                a.put_pixel(x, y, Rgba([10, 20, 30, 255]));
                b.put_pixel(x, y, Rgba([10, 20, 30, 255]));
            }
        }
        let (frac, _) = pixel_diff(&a, &b);
        assert_eq!(frac, 0.0);
    }

    #[test]
    fn pixel_diff_one_different_pixel() {
        let mut a = RgbaImage::new(2, 2);
        let mut b = RgbaImage::new(2, 2);
        for y in 0..2 {
            for x in 0..2 {
                a.put_pixel(x, y, Rgba([0; 4]));
                b.put_pixel(x, y, Rgba([0; 4]));
            }
        }
        b.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let (frac, _) = pixel_diff(&a, &b);
        assert_eq!(frac, 0.25);
    }

    #[test]
    fn build_classifies_size_mismatch() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        write_png(
            &jdir.join("replays/rA/screenshots/s1.png"),
            2,
            2,
            Rgba([255; 4]),
        );
        write_png(
            &jdir.join("replays/rB/screenshots/s1.png"),
            3,
            3,
            Rgba([255; 4]),
        );
        let out_dir = jdir.join("compare").join("test");
        fs::create_dir_all(&out_dir).unwrap();
        let r = build(&jdir, "rA", "rB", &out_dir, 0.0).unwrap();
        assert_eq!(r.entries.len(), 1);
        assert!(matches!(r.entries[0].outcome, ShotOutcome::SizeMismatch));
    }

    #[test]
    fn build_writes_diff_png_when_changed() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        write_png(
            &jdir.join("replays/rA/screenshots/s1.png"),
            4,
            4,
            Rgba([0, 0, 0, 255]),
        );
        write_png(
            &jdir.join("replays/rB/screenshots/s1.png"),
            4,
            4,
            Rgba([255, 255, 255, 255]),
        );
        let out_dir = jdir.join("compare").join("test");
        fs::create_dir_all(&out_dir).unwrap();
        let r = build(&jdir, "rA", "rB", &out_dir, 0.0).unwrap();
        assert_eq!(r.entries.len(), 1);
        assert!(matches!(r.entries[0].outcome, ShotOutcome::Changed));
        assert_eq!(r.entries[0].differing_fraction, Some(1.0));
        assert!(out_dir.join("screenshots").join("s1.diff.png").is_file());
    }

    #[test]
    fn build_respects_pixel_threshold() {
        let tmp = TempDir::new().unwrap();
        let jdir = tmp.path().to_path_buf();
        // 4x4 baseline; flip one pixel in B → 1/16 = 0.0625 differing.
        write_png(
            &jdir.join("replays/rA/screenshots/s1.png"),
            4,
            4,
            Rgba([0, 0, 0, 255]),
        );
        let mut b = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                b.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        b.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        fs::create_dir_all(jdir.join("replays/rB/screenshots")).unwrap();
        b.save(jdir.join("replays/rB/screenshots/s1.png")).unwrap();

        let out_dir = jdir.join("compare").join("test");
        fs::create_dir_all(&out_dir).unwrap();

        // Threshold 0.1 tolerates the 0.0625 difference → Same.
        let r = build(&jdir, "rA", "rB", &out_dir, 0.1).unwrap();
        assert!(matches!(r.entries[0].outcome, ShotOutcome::Same));
        // Threshold 0 → Changed.
        let r2 = build(&jdir, "rA", "rB", &out_dir, 0.0).unwrap();
        assert!(matches!(r2.entries[0].outcome, ShotOutcome::Changed));
    }
}
