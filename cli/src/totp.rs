//! `totp` — emit the current time-based one-time code for `saveAs`,
//! the 2FA complement to the `mail` verb. RFC 6238 over RFC 4226 HOTP:
//! HMAC over the 30-second (default) window counter, dynamic truncation
//! to `digits` decimal places.
//!
//! Scenario shape:
//!
//!   {"id":"s7","kind":"do","verb":"totp",
//!    "params":{"secret":"{{vars.totpSeed}}","digits":6,"period":30},
//!    "saveAs":"otp"}
//!
//! `secret` is the base32 seed (the string authenticator apps store —
//! spaces/hyphens tolerated, padding optional). Put the real seed in a
//! sensitive input or `inputs.local.json` so the scenario file never
//! carries it. `algorithm` = "sha1" (default, the interoperable choice)
//! or "sha256"; `offset` shifts the evaluated instant in seconds for
//! clock-skewed environments.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use hmac::{Hmac, Mac};
use serde_json::Value as Json;
use sha1::Sha1;
use sha2::Sha256;

use crate::value::{substitute_scenario_vars, ValueScope};

/// `totp` do-step: resolve params, compute the code, return it for
/// `saveAs`. `secret` is required; the rest default to RFC conventions.
pub(crate) fn code_step(
    params: Option<&BTreeMap<String, Json>>,
    scope: &mut ValueScope,
    step_id: &str,
) -> Result<Json> {
    let secret_raw = params
        .and_then(|p| p.get("secret"))
        .and_then(|v| v.as_str())
        .with_context(|| {
            format!("step '{step_id}' totp: params.secret is required (base32 seed)")
        })?;
    let secret = substitute_scenario_vars(secret_raw, scope);
    let digits = params
        .and_then(|p| p.get("digits"))
        .and_then(|v| v.as_u64())
        .unwrap_or(6);
    let period = params
        .and_then(|p| p.get("period"))
        .and_then(|v| v.as_u64())
        .unwrap_or(30);
    let offset = params
        .and_then(|p| p.get("offset"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let algorithm = params
        .and_then(|p| p.get("algorithm"))
        .and_then(|v| v.as_str())
        .unwrap_or("sha1");

    let key = base32_decode(&secret)
        .with_context(|| format!("step '{step_id}' totp: secret is not valid base32"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64
        + offset;
    let code = totp(&key, now, period, digits, algorithm)
        .with_context(|| format!("step '{step_id}' totp"))?;
    Ok(Json::String(code))
}

/// T = floor((unix_secs + applied offset) / period); HOTP(key, T).
pub(crate) fn totp(
    key: &[u8],
    unix_secs: i64,
    period: u64,
    digits: u64,
    algorithm: &str,
) -> Result<String> {
    if key.is_empty() {
        bail!("empty secret");
    }
    if period == 0 {
        bail!("period must be > 0");
    }
    if !(1..=10).contains(&digits) {
        bail!("digits must be 1..=10, got {digits}");
    }
    if unix_secs < 0 {
        bail!("negative time after offset — check params.offset");
    }
    let counter = (unix_secs as u64) / period;
    hotp(key, counter, digits, algorithm)
}

fn hotp(key: &[u8], counter: u64, digits: u64, algorithm: &str) -> Result<String> {
    let msg = counter.to_be_bytes();
    let mac: Vec<u8> = match algorithm.to_ascii_lowercase().as_str() {
        "sha1" => <Hmac<Sha1> as Mac>::new_from_slice(key)?
            .chain_update(msg)
            .finalize()
            .into_bytes()
            .to_vec(),
        "sha256" | "sha-256" => <Hmac<Sha256> as Mac>::new_from_slice(key)?
            .chain_update(msg)
            .finalize()
            .into_bytes()
            .to_vec(),
        other => bail!("unsupported algorithm {other:?} — expected sha1|sha256"),
    };
    // RFC 4226 dynamic truncation.
    let offset = (mac[mac.len() - 1] & 0x0f) as usize;
    let bin = ((mac[offset] as u32 & 0x7f) << 24)
        | ((mac[offset + 1] as u32) << 16)
        | ((mac[offset + 2] as u32) << 8)
        | (mac[offset + 3] as u32);
    let modulus = 10u64.checked_pow(digits as u32).unwrap_or(u64::MAX);
    Ok(format!(
        "{:0>width$}",
        bin as u64 % modulus,
        width = digits as usize
    ))
}

/// RFC 4648 base32 (case-insensitive, spaces/hyphens ignored, `=` padding
/// optional). Errors on a character outside the alphabet.
fn base32_decode(s: &str) -> Result<Vec<u8>> {
    let mut bits: u64 = 0;
    let mut nbits = 0u32;
    let mut out = Vec::new();
    for c in s.chars() {
        let v = match c {
            'A'..='Z' => c as u8 - b'A',
            'a'..='z' => c as u8 - b'a',
            '2'..='7' => c as u8 - b'2' + 26,
            ' ' | '-' | '=' => continue,
            _ => bail!("invalid base32 char {c:?}"),
        };
        bits = (bits << 5) | v as u64;
        nbits += 5;
        if nbits >= 8 {
            out.push((bits >> (nbits - 8)) as u8);
            nbits -= 8;
        }
    }
    Ok(out)
}

/// `agent-qa totp <secret>` — print the current code; `--digits`,
/// `--period`, `--algorithm`, `--offset` mirror the verb's params.
pub fn run(args: &[String]) -> Result<u8> {
    let mut secret: Option<String> = None;
    let mut digits = 6u64;
    let mut period = 30u64;
    let mut offset = 0i64;
    let mut algorithm = "sha1".to_string();
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                println!(
                    "agent-qa totp — print the current TOTP code\n\nUsage:\n  agent-qa totp <base32-secret> [--digits N] [--period SECS]\n                     [--algorithm sha1|sha256] [--offset SECS]\n\nIn a scenario use the `totp` do-verb with params.secret — keep real\nseeds in a sensitive input or inputs.local.json."
                );
                return Ok(0);
            }
            "--digits" => digits = parse_flag(&mut it, "--digits")?,
            "--period" => period = parse_flag(&mut it, "--period")?,
            "--offset" => offset = parse_flag(&mut it, "--offset")?,
            "--algorithm" => {
                algorithm = it
                    .next()
                    .context("--algorithm needs a value")?
                    .to_ascii_lowercase()
            }
            other if secret.is_none() => secret = Some(other.to_string()),
            other => bail!("totp: unexpected argument {other:?}"),
        }
    }
    let secret =
        secret.context("usage: agent-qa totp <base32-secret> [--digits N] [--period SECS]")?;
    let key = base32_decode(&secret).context("totp: secret is not valid base32")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64
        + offset;
    println!("{}", totp(&key, now, period, digits, &algorithm)?);
    Ok(0)
}

fn parse_flag<T: std::str::FromStr>(
    it: &mut std::iter::Peekable<std::slice::Iter<String>>,
    flag: &str,
) -> Result<T> {
    it.next()
        .and_then(|v| v.parse::<T>().ok())
        .with_context(|| format!("{flag} expects a number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 4226 Appendix D HOTP vectors (key = "12345678901234567890").
    #[test]
    fn hotp_rfc4226_vectors() {
        let key = b"12345678901234567890";
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, want) in expected.iter().enumerate() {
            assert_eq!(
                hotp(key, counter as u64, 6, "sha1").unwrap(),
                *want,
                "counter {counter}"
            );
        }
    }

    #[test]
    fn totp_base32_secret() {
        // "12345678901234567890" as base32 = GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ.
        let key = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(key, b"12345678901234567890");
        // 59s window 1 → counter 1 → HOTP counter-1 code 287082.
        assert_eq!(totp(&key, 59, 30, 6, "sha1").unwrap(), "287082");
        // 60s → counter 2 → 359152.
        assert_eq!(totp(&key, 60, 30, 6, "sha1").unwrap(), "359152");
    }

    #[test]
    fn base32_tolerates_spaces_hyphens_lower() {
        let a = base32_decode("gezd gnbv-gy3t qojq").unwrap();
        let b = base32_decode("GEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(a, b);
        assert!(base32_decode("GEZ!").is_err());
        assert!(base32_decode("GEZ1").is_err()); // 1 is not in the alphabet
    }

    #[test]
    fn bad_params_rejected() {
        assert!(totp(b"k", 60, 0, 6, "sha1").is_err());
        assert!(totp(b"k", 60, 30, 11, "sha1").is_err());
        assert!(totp(b"", 60, 30, 6, "sha1").is_err());
        assert!(totp(b"k", -5, 30, 6, "sha1").is_err());
        assert!(totp(b"k", 60, 30, 6, "md5").is_err());
    }
}
