//! Pure-stdlib date/time helpers. Currently a single ISO-8601 parser
//! used by `audit duration` / `audit stats` for run wall-clock math.
//! Lives here so other verbs that want timestamp math (e.g. a future
//! `replay --since <ts>` filter) don't need to import anyhow into the
//! call site or copy the algorithm.

use anyhow::{anyhow, Result};

/// Parse an ISO-8601 timestamp like `2026-01-01T12:34:56.789Z` into
/// milliseconds since the Unix epoch. Returns an error for any other
/// format. Pure-stdlib; doesn't pull in chrono.
///
/// Accepts:
///   - `YYYY-MM-DDThh:mm:ss`
///   - `YYYY-MM-DDThh:mm:ss.fff`
///   - Either form with a trailing `Z`
pub fn parse_iso_ms(s: &str) -> Result<u64> {
    let s = s.trim_end_matches('Z');
    let (date, rest) = s
        .split_once('T')
        .ok_or_else(|| anyhow!("bad timestamp {s:?}: expected YYYY-MM-DDThh:mm:ss[.fff]"))?;
    let mut date_parts = date.split('-');
    let y: i64 = date_parts
        .next()
        .ok_or_else(|| anyhow!("bad timestamp {s:?}"))?
        .parse()?;
    let m: u32 = date_parts
        .next()
        .ok_or_else(|| anyhow!("bad timestamp {s:?}"))?
        .parse()?;
    let d: u32 = date_parts
        .next()
        .ok_or_else(|| anyhow!("bad timestamp {s:?}"))?
        .parse()?;
    let (hms, frac) = rest.split_once('.').unwrap_or((rest, "0"));
    let mut hms_parts = hms.split(':');
    let h: u32 = hms_parts
        .next()
        .ok_or_else(|| anyhow!("bad timestamp {s:?}"))?
        .parse()?;
    let mi: u32 = hms_parts
        .next()
        .ok_or_else(|| anyhow!("bad timestamp {s:?}"))?
        .parse()?;
    let sec: u32 = hms_parts
        .next()
        .ok_or_else(|| anyhow!("bad timestamp {s:?}"))?
        .parse()?;
    let ms: u32 = frac
        .chars()
        .take(3)
        .collect::<String>()
        .parse()
        .unwrap_or(0);
    // Days from civil (Howard Hinnant). Public domain algorithm.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_since_epoch = era * 146097 + doe as i64 - 719468;
    let total_secs = days_since_epoch * 86400 + h as i64 * 3600 + mi as i64 * 60 + sec as i64;
    Ok((total_secs as u64).saturating_mul(1000) + ms as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_iso_ms_basic() {
        let a = parse_iso_ms("2026-01-01T00:00:00.000").unwrap();
        let b = parse_iso_ms("2026-01-01T00:00:01.500").unwrap();
        assert_eq!(b - a, 1500);
        let c = parse_iso_ms("2026-01-01T01:00:00.000").unwrap();
        assert_eq!(c - a, 3_600_000);
        assert!(parse_iso_ms("not-a-timestamp").is_err());
    }

    #[test]
    fn parse_iso_ms_handles_trailing_z() {
        let a = parse_iso_ms("2026-01-01T00:00:00Z").unwrap();
        let b = parse_iso_ms("2026-01-01T00:00:00").unwrap();
        assert_eq!(a, b);
    }
}
