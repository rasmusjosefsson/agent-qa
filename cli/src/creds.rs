//! Credential reference resolution via the `credentials` plugin kind.
//!
//! Persona `credentials.entries` and environment `auth.creds` hold
//! `ENV_VAR → value` maps. A value is either a literal or a provider ref
//! shaped `<scheme>:<rest>` (`vault:kv/qa:x`, `op://…`, `awssm:…` — the
//! scheme is opaque to agent-qa; the plugin that claims it defines the
//! rest). Refs are delegated to discovered `credentials` plugins in
//! discovery order; a plugin returning a value resolves the name, an
//! `unresolved` entry carries its reason forward, and a name the plugin
//! omits is treated as a literal it doesn't own.
//!
//! Escapes:
//!   `literal:<anything>`  — strips the marker, never delegated (a
//!                           password that happens to look like `pa:ss`
//!                           or `scheme:x` stays a literal).
//!   scheme-less values    — always literals.
//!
//! Errors: any name a plugin reports unresolved fails the whole
//! resolution — a mistyped or unreachable secret must be loud up front,
//! not a cryptic downstream auth failure. Scheme-shaped values with no
//! `credentials` plugin installed at all are also an error: the operator
//! either installs a provider plugin or marks the value `literal:`.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::plugin::{discovery, host};

const KIND: &str = "credentials";
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);

/// `agent-qa creds-resolve <json-map | ->` — resolve a {ENV_VAR: value}
/// map through the credentials plugins and print the resolved map as
/// JSON on stdout. The workbench resolves credentials this way so the
/// plugin protocol stays in one place (here). Exits 1 with the reason
/// on stderr when any ref can't be resolved.
pub fn run(args: &[String]) -> Result<u8> {
    let src = match args.first().map(String::as_str) {
        Some("-h" | "--help" | "help") => {
            print_help();
            return Ok(0);
        }
        Some("-") => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("read creds map from stdin")?;
            buf
        }
        Some(s) => s.to_string(),
        None => bail!("usage: creds-resolve <json-map | ->"),
    };
    let parsed: Json = serde_json::from_str(&src).context("parse creds map json")?;
    let obj = parsed
        .as_object()
        .ok_or_else(|| anyhow!("creds map must be a JSON object"))?;
    let mut map = BTreeMap::new();
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            map.insert(k.clone(), s.to_string());
        }
    }
    match resolve_credential_refs(&map) {
        Ok(resolved) => {
            println!("{}", serde_json::to_string(&resolved)?);
            Ok(0)
        }
        Err(e) => {
            eprintln!("{e:#}");
            Ok(1)
        }
    }
}

fn print_help() {
    println!(
        "agent-qa creds-resolve — resolve credential refs via credentials plugins\n\nUsage:\n  agent-qa creds-resolve <json-map>\n  agent-qa creds-resolve -        (read the map from stdin)\n\nInput: a JSON object {{ENV_VAR: value}}. Literal values pass through;\n<scheme>:<rest> values are delegated to discovered `credentials`\nplugins (agent-qa.toml [plugins] credentials = ..., AGENT_QA_PLUGINS,\nor agent-qa-plugin-* on $PATH). `literal:<v>` forces a literal.\nPrints the resolved object on stdout; exits 1 when any ref stays\nunresolved (reason on stderr)."
    );
}

/// Is `v` ref-shaped? `^[a-z][a-z0-9._-]*:` with a non-`/` char after the
/// colon — `vault:kv/x`, `op://…`, `awssm:name` qualify; URLs
/// (`https://…`) and single-letter prefixes (`x:y`) do not.
fn scheme_prefix(v: &str) -> Option<&str> {
    let colon = v.find(':')?;
    let scheme = &v[..colon];
    let rest = &v[colon + 1..];
    if scheme.len() < 2
        || !scheme.chars().next()?.is_ascii_alphabetic()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        || rest.starts_with('/')
    {
        return None;
    }
    Some(scheme)
}

/// All discovered plugins serving `credentials`, in priority order:
/// `[plugins] credentials = …` declarations first, then binaries that
/// ping as serving the kind. Multiple provider plugins compose — each
/// gets the still-unresolved names.
fn credential_plugins() -> Result<Vec<PathBuf>> {
    let plugins = discovery::discover(&discovery::DiscoveryOpts::default())?;
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in &plugins {
        if p.declared_kind.as_deref() == Some(KIND) && seen.insert(p.binary.clone()) {
            out.push(p.binary.clone());
        }
    }
    for p in &plugins {
        if p.declared_kind.is_some() || seen.contains(&p.binary) {
            continue;
        }
        if let Ok(pong) = host::ping(&p.binary) {
            if pong.kinds.iter().any(|k| k == KIND) && seen.insert(p.binary.clone()) {
                out.push(p.binary.clone());
            }
        }
    }
    Ok(out)
}

/// Resolve every ref-shaped value in `map` through the credentials
/// plugins. Literal values (and `literal:`-escaped ones) pass through.
/// Values delegated to but unclaimed by every plugin pass through too —
/// a plugin omits names whose scheme it doesn't own. Values a plugin
/// reports unresolved fail the whole call with the plugin's reason.
pub(crate) fn resolve_credential_refs(
    map: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut remaining: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in map {
        if let Some(lit) = value.strip_prefix("literal:") {
            out.insert(name.clone(), lit.to_string());
        } else if scheme_prefix(value).is_some() {
            remaining.insert(name.clone(), value.clone());
        } else {
            out.insert(name.clone(), value.clone());
        }
    }
    if remaining.is_empty() {
        return Ok(out);
    }

    let plugins = credential_plugins()?;
    if plugins.is_empty() {
        let schemes: Vec<String> = remaining
            .values()
            .filter_map(|v| scheme_prefix(v).map(str::to_string))
            .collect();
        bail!(
            "could not resolve credential refs: {} — no credentials plugin serves scheme(s) {}. Install one (agent-qa install …, [plugins] credentials = \"…\", or agent-qa-plugin-* on $PATH), or mark the values literal:…",
            remaining.keys().cloned().collect::<Vec<_>>().join(", "),
            schemes.join(", ")
        );
    }

    // Last unresolved reason per name — a later plugin that resolves the
    // name clears it; a later failure overwrites it.
    let mut reasons: BTreeMap<String, String> = BTreeMap::new();
    for binary in plugins {
        if remaining.is_empty() {
            break;
        }
        let outcome = host::invoke(
            &binary,
            KIND,
            Some("resolve"),
            json!({ "refs": remaining }),
            RESOLVE_TIMEOUT,
        )
        .map_err(|e| anyhow!("credentials resolve via {}: {e}", binary.display()))?;
        let resp = &outcome.response;
        if let Some(values) = resp.get("values").and_then(|v| v.as_object()) {
            for (name, v) in values {
                if let Some(s) = v.as_str() {
                    if remaining.remove(name).is_some() {
                        out.insert(name.clone(), s.to_string());
                        reasons.remove(name);
                    }
                }
            }
        }
        if let Some(unresolved) = resp.get("unresolved").and_then(|v| v.as_object()) {
            for (name, v) in unresolved {
                if remaining.contains_key(name) {
                    if let Some(s) = v.as_str() {
                        reasons.insert(name.clone(), s.to_string());
                    }
                }
            }
        }
    }

    // Names no plugin claimed at all stay literal — the value wasn't a
    // ref the installed providers understand (a colon-literal that
    // slipped past scheme detection).
    for (name, value) in remaining {
        if reasons.contains_key(&name) {
            continue;
        }
        eprintln!(
            "note: no credentials plugin claimed {name} (scheme {:?}) — passing through as a literal",
            scheme_prefix(&value).unwrap_or("?")
        );
        out.insert(name, value);
    }
    if !reasons.is_empty() {
        bail!(
            "could not resolve credential refs: {}",
            reasons
                .iter()
                .map(|(n, r)| format!("{n} — {r}"))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    Ok(out)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    // A credentials plugin that resolves scheme `acme` and reports
    // `broken` refs as unresolved; every other name is omitted (→ literal).
    fn write_acme_plugin(dir: &Path) -> PathBuf {
        write_exec(
            dir,
            "agent-qa-plugin-acme-creds",
            "#!/bin/sh\nif [ \"$1\" = ping ]; then\n  echo '{\"ok\":true,\"response\":{\"protocolVersion\":1,\"name\":\"acme-creds\",\"kinds\":[\"credentials\"]}}'\n  exit 0\nfi\nif [ \"$1\" = credentials ]; then\n  python3 -c 'import json,sys\nreq=json.load(sys.stdin)[\"request\"][\"refs\"]\nvals={};un={}\nfor n,v in req.items():\n  if v.startswith(\"acme:\"): vals[n]=v[5:]+\"-resolved\"\n  elif v.startswith(\"broken:\"): un[n]=\"provider says no\"\nprint(json.dumps({\"ok\":True,\"response\":{\"values\":vals,\"unresolved\":un}}))'\n  exit 0\nfi\nexit 0\n",
        )
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // Isolate plugin discovery: empty HOME kills the global
    // ~/.agent-qa/agent-qa.toml, PATH is replaced by <dir> plus the bare
    // system dirs (sh/python3 shebangs still resolve, no real plugins).
    // Returns the previous env values for restoration.
    fn isolate_env(
        dir: &Path,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) {
        let prev_home = std::env::var("HOME").ok();
        let prev_path = std::env::var("PATH").ok();
        let prev_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        let prev_plugins = std::env::var("AGENT_QA_PLUGINS").ok();
        std::env::set_var("HOME", dir);
        std::env::set_var("PATH", format!("{}:/usr/bin:/bin", dir.display()));
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("AGENT_QA_PLUGINS");
        (prev_home, prev_path, prev_xdg, prev_plugins)
    }

    fn restore_env(
        saved: (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    ) {
        let (home, path, xdg, plugins) = saved;
        for (k, v) in [
            ("HOME", home),
            ("PATH", path),
            ("XDG_CONFIG_HOME", xdg),
            ("AGENT_QA_PLUGINS", plugins),
        ] {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }

    #[test]
    fn scheme_prefix_recognizes_refs_not_urls() {
        assert_eq!(scheme_prefix("vault:kv/x:y"), Some("vault"));
        assert_eq!(scheme_prefix("awssm:prod/db"), Some("awssm"));
        assert_eq!(scheme_prefix("pa:ss"), Some("pa"));
        assert_eq!(scheme_prefix("https://example.com"), None);
        assert_eq!(scheme_prefix("x:y"), None);
        assert_eq!(scheme_prefix("plain"), None);
        assert_eq!(scheme_prefix("user@host:22"), None); // '@' breaks the scheme
    }

    #[test]
    fn literals_pass_through_and_literal_escape_works() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let saved = isolate_env(tmp.path());
        let m = map(&[
            ("A", "plain"),
            ("B", "literal:pa:ss"),
            ("C", "https://example.com"),
        ]);
        let out = resolve_credential_refs(&m).unwrap();
        assert_eq!(out["A"], "plain");
        assert_eq!(out["B"], "pa:ss");
        assert_eq!(out["C"], "https://example.com");
        restore_env(saved);
    }

    #[test]
    fn schemed_value_without_any_credentials_plugin_errors() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        let saved = isolate_env(tmp.path());
        let m = map(&[("A", "vault:kv/x:y")]);
        let err = resolve_credential_refs(&m).unwrap_err().to_string();
        assert!(err.contains("no credentials plugin"), "{err}");
        assert!(err.contains("vault"), "{err}");
        restore_env(saved);
    }

    #[test]
    fn plugin_resolves_claimed_refs_and_leaves_others_literal() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_acme_plugin(tmp.path());
        let saved = isolate_env(tmp.path());
        let m = map(&[
            ("A", "acme:secret"),
            ("B", "pa:ss"), // ref-shaped but unclaimed → literal
            ("C", "just-a-value"),
        ]);
        let out = resolve_credential_refs(&m).unwrap();
        assert_eq!(out["A"], "secret-resolved");
        assert_eq!(out["B"], "pa:ss");
        assert_eq!(out["C"], "just-a-value");
        restore_env(saved);
    }

    #[test]
    fn plugin_reported_unresolved_fails_with_reason() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_acme_plugin(tmp.path());
        let saved = isolate_env(tmp.path());
        let m = map(&[("A", "broken:thing")]);
        let err = resolve_credential_refs(&m).unwrap_err().to_string();
        assert!(err.contains("could not resolve credential refs"), "{err}");
        assert!(err.contains("A — provider says no"), "{err}");
        restore_env(saved);
    }
}
