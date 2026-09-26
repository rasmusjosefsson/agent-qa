//! `locator.name.i18nKey` resolution.
//!
//! A recorded accessible name may be an app translation key rather than literal
//! text. Replay resolves the key through `i18n.json` beside the scenario — a
//! flat `{ "key": "accessible name" }` map shipped with the scenario, so the
//! same locator replays under whichever locale text the scenario carries.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// Look `key` up in `<scenario_dir>/i18n.json`. Errors are actionable: a
/// missing file names where to put it, a missing key lists what exists.
pub fn resolve(scenario_dir: &Path, key: &str) -> Result<String> {
    let path = scenario_dir.join("i18n.json");
    let text = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "locator.name.i18nKey {key:?} needs a dictionary at {}",
            path.display()
        )
    })?;
    let map: BTreeMap<String, String> = serde_json::from_str(&text).with_context(|| {
        format!(
            "{}: i18n.json must be a flat object of key → accessible name",
            path.display()
        )
    })?;
    match map.get(key) {
        Some(v) if !v.is_empty() => Ok(v.clone()),
        _ => {
            let keys: Vec<&str> = map.keys().map(String::as_str).collect();
            let shown: Vec<&str> = keys.iter().take(20).copied().collect();
            let more = if keys.len() > 20 { ", …" } else { "" };
            bail!(
                "i18n.json has no entry for {key:?} (has: {}{})",
                shown.join(", "),
                more
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolves_a_key() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(
            dir.path().join("i18n.json"),
            r#"{"form.submit": "Save changes"}"#,
        )
        .unwrap();
        assert_eq!(resolve(dir.path(), "form.submit").unwrap(), "Save changes");
    }

    #[test]
    fn missing_file_names_the_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let e = resolve(dir.path(), "k").unwrap_err().to_string();
        assert!(e.contains("i18n.json"), "{e}");
    }

    #[test]
    fn missing_key_lists_available() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(dir.path().join("i18n.json"), r#"{"a": "A", "b": "B"}"#).unwrap();
        let e = resolve(dir.path(), "zzz").unwrap_err().to_string();
        assert!(
            e.contains("\"zzz\"") && e.contains('a') && e.contains('b'),
            "{e}"
        );
    }

    #[test]
    fn non_object_file_errors() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(dir.path().join("i18n.json"), r#"["not", "an", "object"]"#).unwrap();
        assert!(resolve(dir.path(), "k")
            .unwrap_err()
            .to_string()
            .contains("flat object"));
    }
}
