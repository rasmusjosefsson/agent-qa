//! `--persona` / `--environment` resolution for `replay` — the CLI mirror of
//! the workbench server's `resolveRunAuthEnv`, so a credentialed run works
//! from the terminal exactly as it does from the Runs pane.
//!
//! Records live under the scenarios root:
//!   <root>/_personas/<id>/persona.json
//!       { profile, credentials: { entries: { ENV_VAR: "literal|<scheme>:<ref>" } } }
//!   <root>/_environments/<id>/environment.json
//!       { baseUrl, params: {...}, auth: { plugin, loginUrl, config, creds } }
//!
//! `apply()` runs once before dispatch:
//!   - persona `profile` becomes the run's `--profile` (and derives the
//!     `<profile>-session` name when the caller didn't pass `--session`)
//!   - environment `params` (+`baseUrl` → `baseUrl`) merge UNDER explicit
//!     `--param` overrides
//!   - credential entries (literal or `<scheme>:` provider ref) resolve into
//!     process env vars via `credentials` plugins — the auth plugin reads
//!     them during the scenario's `useProfile` op
//!   - `AGENT_QA_ENV_*` vars surface the environment's connection config
//!   - `profile-add` registers the profile↔plugin adapter binding
//!     (idempotent) so `useProfile` can bootstrap the login

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value as Json;

use crate::paths;
use crate::runner::RunOptions;

pub fn apply(opts: &mut RunOptions) -> Result<()> {
    if opts.persona.is_none() && opts.environment.is_none() {
        return Ok(());
    }
    let root = paths::scenarios_root();
    let env_rec = load_environment(&root, opts.environment.as_deref())?;

    // Environment params → param defaults. An explicit `--param k=v` still
    // wins — env values fill in what the command line didn't name.
    if let Some(env) = &env_rec {
        let mut defaults: BTreeMap<String, String> = BTreeMap::new();
        if let Some(params) = env.get("params").and_then(|p| p.as_object()) {
            for (k, v) in params {
                if let Some(s) = v.as_str() {
                    defaults.insert(k.clone(), s.to_string());
                }
            }
        }
        if let Some(base) = env.get("baseUrl").and_then(|b| b.as_str()) {
            if !base.is_empty() {
                defaults.insert("baseUrl".to_string(), base.to_string());
            }
        }
        for (k, v) in defaults {
            opts.input_overrides.entry(k).or_insert(v);
        }
    }

    let Some(persona_id) = opts.persona.as_deref() else {
        return Ok(());
    };
    let persona = load_persona(&root, persona_id)?;
    let profile = persona
        .get("profile")
        .and_then(|p| p.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if profile.is_empty() {
        bail!("persona '{persona_id}' has no profile to authenticate");
    }

    let auth = env_rec
        .as_ref()
        .and_then(|e| e.get("auth"))
        .cloned()
        .unwrap_or(Json::Null);

    // Environment's shared creds (base) + persona's identity creds (override).
    let mut entries: BTreeMap<String, String> = BTreeMap::new();
    if let Some(creds) = auth.get("creds").and_then(|c| c.as_object()) {
        for (k, v) in creds {
            if let Some(s) = v.as_str() {
                entries.insert(k.clone(), s.to_string());
            }
        }
    }
    if let Some(creds) = persona
        .pointer("/credentials/entries")
        .and_then(|c| c.as_object())
    {
        for (k, v) in creds {
            if let Some(s) = v.as_str() {
                entries.insert(k.clone(), s.to_string());
            }
        }
    }
    for (k, v) in crate::creds::resolve_credential_refs(&entries)? {
        std::env::set_var(k, v);
    }

    if let Some(base) = env_rec
        .as_ref()
        .and_then(|e| e.get("baseUrl"))
        .and_then(|b| b.as_str())
        .filter(|b| !b.is_empty())
    {
        std::env::set_var("AGENT_QA_ENV_BASE_URL", base);
    }
    if let Some(login) = auth
        .get("loginUrl")
        .and_then(|l| l.as_str())
        .filter(|l| !l.is_empty())
    {
        std::env::set_var("AGENT_QA_ENV_LOGIN_URL", login);
    }
    if let Some(config) = auth.get("config").and_then(|c| c.as_object()) {
        for (k, v) in config {
            if let Some(s) = v.as_str() {
                let key: String = k
                    .to_uppercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                std::env::set_var(format!("AGENT_QA_ENV_{key}"), s);
            }
        }
    }

    // Register the profile against the env's auth plugin (idempotent) so
    // replay's `useProfile` op finds the adapter binding and runs
    // profile-bootstrap — with these credentials already in env.
    if let Some(plugin) = auth
        .get("plugin")
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
    {
        crate::profile_add::run(&[profile.clone(), "--adapter".to_string(), plugin.to_string()])?;
    }

    // `<profile>-session` — the same derivation --profile triggers at parse
    // time. An explicit --session survives (session_name already differs).
    if opts.session_name == "default" {
        opts.session_name = format!("{profile}-session");
    }
    opts.profile = Some(profile);
    Ok(())
}

fn load_persona(root: &Path, id: &str) -> Result<Json> {
    safe_record_id(id, "persona")?;
    let file = root.join("_personas").join(id).join("persona.json");
    let body = fs::read_to_string(&file).with_context(|| {
        format!(
            "no such persona: {id} — available: {}",
            available_ids(&root.join("_personas"), "persona.json")
        )
    })?;
    serde_json::from_str(&body)
        .with_context(|| format!("unparseable persona record {}", file.display()))
}

/// Subdirectory ids holding a `<marker>` record, comma-joined — the
/// available choices for a "no such <record>" error.
fn available_ids(dir: &Path, marker: &str) -> String {
    let mut ids: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().join(marker).is_file())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    if ids.is_empty() {
        "(none defined)".to_string()
    } else {
        ids.join(", ")
    }
}

/// Resolve the environment record: the named one, else the default-flagged
/// one, else the sole record. Returns `None` when none are defined.
fn load_environment(root: &Path, id: Option<&str>) -> Result<Option<Json>> {
    if let Some(id) = id {
        safe_record_id(id, "environment")?;
        let file = root.join("_environments").join(id).join("environment.json");
        let body = fs::read_to_string(&file).with_context(|| {
            format!(
                "no such environment: {id} — available: {}",
                available_ids(&root.join("_environments"), "environment.json")
            )
        })?;
        return serde_json::from_str(&body)
            .with_context(|| format!("unparseable environment record {}", file.display()))
            .map(Some);
    }
    let dir = root.join("_environments");
    let mut records: Vec<Json> = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    for entry in entries.flatten() {
        let file = entry.path().join("environment.json");
        if let Ok(body) = fs::read_to_string(&file) {
            if let Ok(rec) = serde_json::from_str::<Json>(&body) {
                records.push(rec);
            }
        }
    }
    Ok(records
        .iter()
        .find(|e| e.get("default").and_then(|d| d.as_bool()) == Some(true))
        .cloned()
        .or_else(|| (records.len() == 1).then(|| records[0].clone())))
}

fn safe_record_id(id: &str, label: &str) -> Result<()> {
    if !paths::is_safe(id) {
        bail!("unsafe {label} id: {id:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use serde_json::json;
    use tempfile::TempDir;

    fn write_persona(root: &Path, id: &str, rec: Json) {
        let dir = root.join("_personas").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("persona.json"), rec.to_string()).unwrap();
    }

    fn write_environment(root: &Path, id: &str, rec: Json) {
        let dir = root.join("_environments").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("environment.json"), rec.to_string()).unwrap();
    }

    fn base_opts() -> RunOptions {
        RunOptions {
            source: crate::runner::ScenarioSource::Sid("t".into()),
            profile: None,
            persona: None,
            environment: None,
            session_name: "default".into(),
            heal_from_run: None,
            headed: false,
            browser_profile: None,
            input_overrides: BTreeMap::new(),
            dry_run: false,
            no_sidecars: false,
            quiet: false,
            plain: false,
            tag: None,
            output_audit: None,
            from_step: None,
            until_step: None,
            update_baselines: false,
            keep_going: false,
            run_for: Vec::new(),
            baseline_sync: false,
            record_video: None,
            junit: None,
            base_url: None,
            auto_promote: false,
            freeze: None,
            har: false,
            mock_from: None,
            offline: false,
            fresh_browser: false,
        }
    }

    fn opts_with_env(environment: &str) -> RunOptions {
        RunOptions {
            environment: Some(environment.to_string()),
            ..base_opts()
        }
    }

    fn opts_with_persona(persona: &str) -> RunOptions {
        RunOptions {
            persona: Some(persona.to_string()),
            ..base_opts()
        }
    }

    #[test]
    fn environment_params_merge_under_param_overrides() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_environment(
            tmp.path(),
            "staging",
            json!({
                "schema": "environment/1", "id": "staging",
                "baseUrl": "https://staging.example.com",
                "params": { "tenant": "acme", "locale": "sv" },
                "auth": {}
            }),
        );
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let mut opts = opts_with_env("staging");
        opts.input_overrides
            .insert("locale".to_string(), "en".to_string());
        apply(&mut opts).unwrap();
        assert_eq!(
            opts.input_overrides.get("tenant").map(String::as_str),
            Some("acme")
        );
        assert_eq!(
            opts.input_overrides.get("baseUrl").map(String::as_str),
            Some("https://staging.example.com")
        );
        // explicit --param beats the environment value
        assert_eq!(
            opts.input_overrides.get("locale").map(String::as_str),
            Some("en")
        );
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
    }

    #[test]
    fn persona_sets_profile_and_session() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_persona(
            tmp.path(),
            "admin",
            json!({
                "schema": "persona/1", "id": "admin", "profile": "admin-p",
                "credentials": { "entries": { "ADMIN_EMAIL": "a@x.io" } }
            }),
        );
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        std::env::remove_var("ADMIN_EMAIL");
        let mut opts = opts_with_persona("admin");
        apply(&mut opts).unwrap();
        assert_eq!(opts.profile.as_deref(), Some("admin-p"));
        assert_eq!(opts.session_name, "admin-p-session");
        assert_eq!(std::env::var("ADMIN_EMAIL").unwrap(), "a@x.io");
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
        std::env::remove_var("ADMIN_EMAIL");
    }

    #[test]
    fn missing_persona_errors() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let mut opts = opts_with_persona("ghost");
        assert!(apply(&mut opts)
            .unwrap_err()
            .to_string()
            .contains("no such persona"));
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
    }

    #[test]
    fn unknown_persona_names_the_available_ids() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_persona(tmp.path(), "admin-user", json!({"schema": "persona/1"}));
        write_persona(tmp.path(), "viewer", json!({"schema": "persona/1"}));
        let err = load_persona(tmp.path(), "admni").unwrap_err().to_string();
        assert!(err.contains("no such persona: admni"), "{err}");
        assert!(err.contains("admin-user, viewer"), "{err}");
    }

    #[test]
    fn unknown_environment_names_the_available_ids() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_environment(tmp.path(), "staging", json!({"schema": "env/1"}));
        let err = load_environment(tmp.path(), Some("prod"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("no such environment: prod"), "{err}");
        assert!(err.contains("staging"), "{err}");
    }

    #[test]
    fn unresolved_credential_ref_errors() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_persona(
            tmp.path(),
            "v",
            json!({
                "schema": "persona/1", "id": "v", "profile": "v-p",
                "credentials": { "entries": { "V_PASS": "vault:secret/x:pw" } }
            }),
        );
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        // Isolate discovery: no real plugins must leak in.
        let prev_home = std::env::var("HOME").ok();
        let prev_path = std::env::var("PATH").ok();
        let prev_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        let prev_plugins = std::env::var("AGENT_QA_PLUGINS").ok();
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("PATH", "/usr/bin:/bin");
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("AGENT_QA_PLUGINS");
        let mut opts = opts_with_persona("v");
        let err = apply(&mut opts).unwrap_err();
        assert!(err.to_string().contains("credential refs"), "{err}");
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
        for (k, v) in [
            ("HOME", prev_home),
            ("PATH", prev_path),
            ("XDG_CONFIG_HOME", prev_xdg),
            ("AGENT_QA_PLUGINS", prev_plugins),
        ] {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }
}
