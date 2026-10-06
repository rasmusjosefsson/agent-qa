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
    /// Contiguous changed regions `(x, y, w, h, pixels)`, largest first —
    /// one structural change = one region. Empty unless outcome is Changed.
    pub regions: Vec<(u32, u32, u32, u32, u32)>,
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
                        regions: Vec::new(),
                    });
                    continue;
                }
                let (frac, diff_img) = pixel_diff(&a, &b);
                if frac <= threshold {
                    entries.push(ShotEntry {
                        step_id: id,
                        outcome: ShotOutcome::Same,
                        differing_fraction: Some(frac),
                        regions: Vec::new(),
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
                        regions: diff_regions(&diff_img),
                    });
                }
            }
            (true, false) => entries.push(ShotEntry {
                step_id: id,
                outcome: ShotOutcome::OnlyA,
                differing_fraction: None,
                regions: Vec::new(),
            }),
            (false, true) => entries.push(ShotEntry {
                step_id: id,
                outcome: ShotOutcome::OnlyB,
                differing_fraction: None,
                regions: Vec::new(),
            }),
            (false, false) => {}
        }
    }

    Ok(ShotReport { entries })
}

pub(crate) fn decode_png(path: &Path) -> Result<RgbaImage> {
    let img = ImageReader::open(path)
        .with_context(|| format!("open {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("guess format {}", path.display()))?
        .decode()
        .with_context(|| format!("decode {}", path.display()))?;
    Ok(img.to_rgba8())
}

/// Per-pixel channel delta above which a pixel counts as changed. Font
/// antialiasing/hinting renders a few RGB levels differently across machines
/// and Chromium builds — counting those ±1..±30 sub-pixel jitters would flake
/// every text-heavy golden. 32/255 (~12.5% of a channel) swallows AA noise
/// while still catching real layout/text/color changes (the same trade-off
/// pixelmatch's default 0.1 threshold makes). On top of the raw delta the
/// diff also runs pixelmatch's edge-AA detector, which ignores pixels that
/// read as edge antialiasing rather than real change.
const AA_DELTA: i16 = 32;

fn channel_delta(a: &Rgba<u8>, b: &Rgba<u8>) -> i16 {
    (0..4)
        .map(|i| (a[i] as i16 - b[i] as i16).abs())
        .max()
        .unwrap_or(0)
}

/// True when `p` has at least 3 equal neighbours in `img` — pixelmatch's
/// "identical siblings" test used by the antialiasing detector.
fn has_many_siblings(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> bool {
    let p = *img.get_pixel(x, y);
    let mut count = 0u32;
    for dy in -1i64..=1 {
        for dx in -1i64..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            if *img.get_pixel(nx as u32, ny as u32) == p {
                count += 1;
                if count > 2 {
                    return true;
                }
            }
        }
    }
    false
}

/// Perceived luminance delta between two RGBA pixels (pixelmatch's Y
/// formula on the RGB channels, blended with the alpha difference).
fn luminance_delta(a: &Rgba<u8>, b: &Rgba<u8>) -> f64 {
    let dr = a[0] as f64 - b[0] as f64;
    let dg = a[1] as f64 - b[1] as f64;
    let db = a[2] as f64 - b[2] as f64;
    let da = a[3] as f64 - b[3] as f64;
    0.298895 * dr + 0.586622 * dg + 0.114482 * db + 0.297 * da
}

/// Pixelmatch-style antialiasing detection: a pixel is AA when it sits on
/// an edge — its brightness is between its neighbours' min and max — and
/// its darkest or brightest neighbour is a stable part of the image (has
/// 3+ identical siblings in BOTH images). Real shifts lack one of those
/// halves, so they stay "changed".
fn antialiased(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32, other: &RgbaImage) -> bool {
    let p = *img.get_pixel(x, y);
    // Edge pixels have fewer than 8 real neighbours — pixelmatch credits
    // them one "equal" up front since the missing side can't contradict.
    let mut zeros = u32::from(x == 0 || y == 0 || x == w - 1 || y == h - 1);
    let mut min = 0.0f64;
    let mut max = 0.0f64;
    let mut min_xy = (0u32, 0u32);
    let mut max_xy = (0u32, 0u32);
    for dy in -1i64..=1 {
        for dx in -1i64..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let q = *img.get_pixel(nx as u32, ny as u32);
            let d = luminance_delta(&p, &q);
            if d.abs() < f64::EPSILON {
                zeros += 1;
                if zeros > 2 {
                    return false; // flat area — not an edge
                }
            } else if d < min {
                min = d;
                min_xy = (nx as u32, ny as u32);
            } else if d > max {
                max = d;
                max_xy = (nx as u32, ny as u32);
            }
        }
    }
    // A real AA pixel sits mid-edge: it must have BOTH a darker and a
    // brighter neighbour. One side missing (all neighbours uniformly
    // darker/lighter) means the pixel is a fill change, not an edge.
    if min == 0.0 || max == 0.0 {
        return false;
    }
    (has_many_siblings(img, min_xy.0, min_xy.1, w, h)
        && has_many_siblings(other, min_xy.0, min_xy.1, w, h))
        || (has_many_siblings(img, max_xy.0, max_xy.1, w, h)
            && has_many_siblings(other, max_xy.0, max_xy.1, w, h))
}

/// Connected-component clustering of the diff mask (4-neighbour flood
/// fill). Returns bounding boxes `(x, y, w, h, pixel_count)` of each
/// contiguous changed region — one structural change = one cluster, even
/// if it spans many pixels. Used for RCA + reporting; diff of ≤ `min_size`
/// pixels still counts as a region (a lone pixel IS a region).
pub(crate) fn diff_regions(diff: &RgbaImage) -> Vec<(u32, u32, u32, u32, u32)> {
    let (w, h) = diff.dimensions();
    let mut visited = vec![false; (w * h) as usize];
    let is_diff = |x: u32, y: u32| diff.get_pixel(x, y)[0] == 255 && diff.get_pixel(x, y)[1] == 0;
    let mut regions = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if visited[idx] || !is_diff(x, y) {
                continue;
            }
            // BFS flood fill.
            let mut stack = vec![(x, y)];
            visited[idx] = true;
            let (mut rx0, mut ry0, mut rx1, mut ry1) = (x, y, x, y);
            let mut count = 0u32;
            while let Some((cx, cy)) = stack.pop() {
                count += 1;
                rx0 = rx0.min(cx);
                ry0 = ry0.min(cy);
                rx1 = rx1.max(cx);
                ry1 = ry1.max(cy);
                for (dx, dy) in [(1i64, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (cx as i64 + dx, cy as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let nidx = (ny as u32 * w + nx as u32) as usize;
                    if !visited[nidx] && is_diff(nx as u32, ny as u32) {
                        visited[nidx] = true;
                        stack.push((nx as u32, ny as u32));
                    }
                }
            }
            regions.push((rx0, ry0, rx1 - rx0 + 1, ry1 - ry0 + 1, count));
        }
    }
    regions.sort_by_key(|r| std::cmp::Reverse(r.4));
    regions
}

/// A stable fingerprint for *this* drift — the SHA-256 (truncated to 16
/// hex chars) of the quantized region signature + image size. Quantizing
/// each region's geometry to an 8px bucket and its pixel mass to a
/// percent-of-area band absorbs the subpixel/AA jitter a flaky diff
/// throws while still distinguishing a genuinely different change — the
/// idea Argos calls a "change fingerprint".
///
/// The same UI regression against the same baseline produces the same
/// fingerprint across runs, which is what lets a run report say "same
/// drift as run N" and lets `known-drift.json` suppress a triaged one.
/// Empty region list (shouldn't happen on a Changed diff) still hashes —
/// to the signature of just the dimensions.
pub fn drift_fingerprint(regions: &[(u32, u32, u32, u32, u32)], width: u32, height: u32) -> String {
    use sha2::{Digest, Sha256};
    let area = (width as u64 * height as u64).max(1);
    let mut sig = format!("{width}x{height}");
    // Sort by (x,y) so the hash doesn't depend on the largest-first
    // ordering the regions list is presented in.
    let mut rs: Vec<(u32, u32, u32, u32, u32)> = regions.to_vec();
    rs.sort_by_key(|r| (r.0, r.1));
    for (x, y, w, h, px) in rs {
        // 8px buckets for geometry; percent band for mass (>=1% steps,
        // then 10% bands past 10% — a diff that doubles in size is a
        // different change).
        let mass = px as u64 * 100 / area;
        let band = if mass < 10 { mass } else { mass / 10 * 10 };
        sig.push_str(&format!(
            "|{},{},{},{},{}",
            x / 8,
            y / 8,
            w.max(1) / 8,
            h.max(1) / 8,
            band
        ));
    }
    let digest = Sha256::digest(sig.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Perceptual diff (pixelmatch-flavoured): a pixel counts as changed when
/// its channel delta exceeds `aa_delta` AND it isn't antialiasing on an
/// edge — checked against both images. `aa_delta <= 0` disables the AA
/// filter entirely (strict byte-compare).
fn perceptual_diff(a: &RgbaImage, b: &RgbaImage, aa_delta: i16) -> (f64, RgbaImage) {
    let (w, h) = a.dimensions();
    let total = (w as u64) * (h as u64);
    let mut diff = RgbaImage::new(w, h);
    let mut differing: u64 = 0;
    for y in 0..h {
        for x in 0..w {
            let pa = a.get_pixel(x, y);
            let pb = b.get_pixel(x, y);
            let changed = channel_delta(pa, pb) > aa_delta
                && (aa_delta <= 0
                    || (!antialiased(a, x, y, w, h, b) && !antialiased(b, x, y, w, h, a)));
            if changed {
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

/// Returns (differing-fraction, delta-map image). Delta map shows the
/// baseline (`a`) faded to 50% greyscale; differing pixels are red.
pub(crate) fn pixel_diff(a: &RgbaImage, b: &RgbaImage) -> (f64, RgbaImage) {
    pixel_diff_with(a, b, AA_DELTA)
}

/// `pixel_diff` with a caller-chosen AA delta — the shot claim's
/// `tolerance.preset`/`tolerance.aa` knobs land here.
pub(crate) fn pixel_diff_with(a: &RgbaImage, b: &RgbaImage, aa_delta: i16) -> (f64, RgbaImage) {
    perceptual_diff(a, b, aa_delta)
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
    fn perceptual_ignores_edge_antialiasing() {
        // An AA fringe: a grey column (mid-luminance) between a solid
        // black core and the white background — how hinting actually
        // renders text edges. B renders the same fringe a few levels
        // different (> AA_DELTA), as another rasterizer would. The edge
        // detector forgives it; the raw-delta diff would flag it.
        let (w, h) = (6, 4);
        let mut a = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
        let mut b = a.clone();
        for y in 0..h {
            a.put_pixel(2, y, Rgba([0, 0, 0, 255]));
            b.put_pixel(2, y, Rgba([0, 0, 0, 255]));
            a.put_pixel(3, y, Rgba([140, 140, 140, 255]));
            b.put_pixel(3, y, Rgba([190, 190, 190, 255]));
        }
        let (frac, _) = pixel_diff(&a, &b);
        assert_eq!(frac, 0.0);
    }

    #[test]
    fn perceptual_flags_solid_edge_shift() {
        // A SOLID colour edge moved one pixel: not AA (no mid-luminance
        // pixels) — pixelmatch correctly reports it as a real change.
        let (w, h) = (8, 4);
        let mut a = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
        let mut b = a.clone();
        for y in 0..h {
            a.put_pixel(2, y, Rgba([0, 0, 0, 255]));
            b.put_pixel(3, y, Rgba([0, 0, 0, 255]));
        }
        let (frac, _) = pixel_diff(&a, &b);
        assert!(frac > 0.0);
    }

    #[test]
    fn perceptual_counts_real_fills() {
        // A solid block change (no edge neighbourhood) is NOT antialiasing.
        let a = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
        let mut b = a.clone();
        for y in 2..6 {
            for x in 2..6 {
                b.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let (frac, diff) = pixel_diff(&a, &b);
        // Block edges have both darker+brighter neighbours, so the AA
        // detector forgives the outline pixels; the interior stays diff.
        assert!(frac > 0.0);
        let regions = diff_regions(&diff);
        assert!(!regions.is_empty());
    }

    #[test]
    fn diff_regions_clusters_separate_changes() {
        // Two disconnected changed areas → two regions, largest first.
        let a = RgbaImage::from_pixel(12, 4, Rgba([255; 4]));
        let mut b = a.clone();
        for x in 0..3 {
            b.put_pixel(x, 0, Rgba([0, 0, 0, 255]));
        }
        b.put_pixel(10, 3, Rgba([0, 0, 0, 255]));
        let (_, diff) = pixel_diff(&a, &b);
        let regions = diff_regions(&diff);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].4, 3); // the 3-px strip first
        assert_eq!(regions[1], (10, 3, 1, 1, 1));
    }

    #[test]
    fn fingerprint_stable_across_aa_jitter() {
        // The same structural change plus a few jittered pixels must keep
        // one fingerprint — that's what makes ledger suppression safe.
        let mk = |jitter: u32| {
            let a = RgbaImage::from_pixel(64, 64, Rgba([255; 4]));
            let mut b = a.clone();
            for x in 8..24 {
                b.put_pixel(x, 10, Rgba([0, 0, 0, 255]));
            }
            b.put_pixel(50, 50 + jitter, Rgba([0, 0, 0, 255]));
            let (_, d) = pixel_diff(&a, &b);
            drift_fingerprint(&diff_regions(&d), d.width(), d.height())
        };
        // A lone 1px jitter shifts inside the same 8px bucket → same fp.
        assert_eq!(mk(0), mk(1));
        // A completely different change is a different fingerprint.
        let a = RgbaImage::from_pixel(64, 64, Rgba([255; 4]));
        let mut c = a.clone();
        for x in 40..56 {
            c.put_pixel(x, 40, Rgba([0, 0, 0, 255]));
        }
        let (_, d2) = pixel_diff(&a, &c);
        let other = drift_fingerprint(&diff_regions(&d2), d2.width(), d2.height());
        assert_ne!(mk(0), other);
    }

    #[test]
    fn pixel_diff_ignores_antialias_jitter() {
        let mut a = RgbaImage::new(4, 4);
        let mut b = RgbaImage::new(4, 4);
        for (x, y) in [(0, 0), (1, 1)] {
            a.put_pixel(x, y, Rgba([100, 100, 100, 255]));
            b.put_pixel(x, y, Rgba([100 + AA_DELTA as u8, 100, 100, 255]));
        }
        // one pixel truly changed (delta > AA_DELTA)
        a.put_pixel(3, 3, Rgba([10, 10, 10, 255]));
        b.put_pixel(3, 3, Rgba([10 + AA_DELTA as u8 + 1, 10, 10, 255]));
        let (frac, _map) = pixel_diff(&a, &b);
        assert!((frac - 1.0 / 16.0).abs() < f64::EPSILON);
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
