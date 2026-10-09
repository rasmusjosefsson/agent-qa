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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value as Json;

use crate::paths;
use crate::runner::{RunOptions, ScenarioSource};

pub fn apply(opts: &mut RunOptions) -> Result<()> {
    if opts.persona.is_none() && opts.environment.is_none() {
        return Ok(());
    }
    let root = paths::scenarios_root();
    let env_rec = load_environment(&root, opts.environment.as_deref())?;

    // Environment params → param defaults. An explicit `--param k=v` still
    // wins — env values fill in what the command line didn't name. Only keys
    // the scenario declares in `inputs` merge: an environment record
    // describes ambient run context (baseUrl, deployment, tags…) and a
    // scenario legitimately declares none or a subset, so pushing every env
    // key into input_overrides would trip the declared-inputs check on
    // values nothing can reference.
    if let Some(env) = &env_rec {
        let declared = declared_input_names(opts).unwrap_or_default();
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
            if declared.contains(&k) {
                opts.input_overrides.entry(k).or_insert(v);
            }
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
    if file.is_file() {
        let body = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
        return serde_json::from_str(&body)
            .with_context(|| format!("unparseable persona record {}", file.display()));
    }
    // Package-provided personas (installed extension packages register
    // flat <id>.json records under [personas] extra-dirs — same lookup the
    // workbench performs; local records shadow by id).
    for rec in package_records("personas") {
        if rec.get("id").and_then(|v| v.as_str()) == Some(id) {
            return Ok(rec);
        }
    }
    bail!(
        "no such persona: {id} — available: {}",
        available_persona_ids(root)
    )
}

/// Local persona ids + package-provided ones, for the not-found message.
fn available_persona_ids(root: &Path) -> String {
    let mut ids: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join("_personas")) {
        ids.extend(
            entries
                .flatten()
                .filter(|e| e.path().join("persona.json").is_file())
                .filter_map(|e| e.file_name().to_str().map(str::to_string)),
        );
    }
    for rec in package_records("personas") {
        if let Some(id) = rec.get("id").and_then(|v| v.as_str()) {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        "(none defined)".to_string()
    } else {
        ids.join(", ")
    }
}

/// Resolve the environment record: the named one (local first, then a
/// package-provided record by id — same shadowing the workbench uses), else
/// the default-flagged one, else the sole record. Returns `None` when none
/// are defined.
fn load_environment(root: &Path, id: Option<&str>) -> Result<Option<Json>> {
    if let Some(id) = id {
        safe_record_id(id, "environment")?;
        let file = root.join("_environments").join(id).join("environment.json");
        if file.is_file() {
            let body =
                fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
            return serde_json::from_str(&body)
                .with_context(|| format!("unparseable environment record {}", file.display()))
                .map(Some);
        }
        for rec in package_records("environments") {
            if rec.get("id").and_then(|v| v.as_str()) == Some(id) {
                return Ok(Some(rec));
            }
        }
        bail!(
            "no such environment: {id} — available: {}",
            available_environment_ids(root)
        );
    }
    let mut records: Vec<Json> = Vec::new();
    let dir = root.join("_environments");
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let file = entry.path().join("environment.json");
            if let Ok(body) = fs::read_to_string(&file) {
                if let Ok(rec) = serde_json::from_str::<Json>(&body) {
                    records.push(rec);
                }
            }
        }
    }
    records.extend(package_records("environments"));
    Ok(records
        .iter()
        .find(|e| e.get("default").and_then(|d| d.as_bool()) == Some(true))
        .cloned()
        .or_else(|| (records.len() == 1).then(|| records[0].clone())))
}

/// Local environment ids + package-provided ones, for the not-found message.
fn available_environment_ids(root: &Path) -> String {
    let mut ids: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join("_environments")) {
        ids.extend(
            entries
                .flatten()
                .filter(|e| e.path().join("environment.json").is_file())
                .filter_map(|e| e.file_name().to_str().map(str::to_string)),
        );
    }
    for rec in package_records("environments") {
        if let Some(id) = rec.get("id").and_then(|v| v.as_str()) {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        "(none defined)".to_string()
    } else {
        ids.join(", ")
    }
}

/// Read-only records installed packages ship: flat `<id>.json` files under
/// each `[personas]`/`[environments]` `extra-dirs` entry of the user config.
/// Mirrors the workbench's packageRecordDirs/readPackageRecords so a
/// packaged persona/environment works for `replay --persona/--environment`
/// without being copied under `<root>/_personas`/`_environments`.
fn package_records(table: &str) -> Vec<Json> {
    let mut out = Vec::new();
    for dir in package_record_dirs(table) {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let file = entry.path();
            if file.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(body) = fs::read_to_string(&file) else {
                continue;
            };
            let Ok(mut rec) = serde_json::from_str::<Json>(&body) else {
                continue;
            };
            if rec.get("id").is_none() {
                if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
                    rec["id"] = Json::String(stem.to_string());
                }
            }
            out.push(rec);
        }
    }
    out
}

/// `[<table>] extra-dirs` from the user-level config files — the global
/// `~/.agent-qa/agent-qa.toml` (or `$AGENT_QA_HOME/agent-qa.toml`) plus the
/// repo `agent-qa.toml` walked up from cwd, matching how `skills` merges its
/// own extra-dirs. Best-effort: unreadable/typo'd files are skipped (the
/// skills loader already warns on those).
fn package_record_dirs(table: &str) -> Vec<PathBuf> {
    #[derive(serde::Deserialize)]
    struct RecordConfig {
        personas: Option<ExtraDirs>,
        environments: Option<ExtraDirs>,
    }
    #[derive(serde::Deserialize)]
    struct ExtraDirs {
        #[serde(rename = "extra-dirs", alias = "extra_dirs")]
        extra_dirs: Option<Vec<String>>,
    }

    let mut files = crate::global_config::existing_global_config_files();
    if let Some(home) = std::env::var_os("AGENT_QA_HOME") {
        let p = PathBuf::from(home).join("agent-qa.toml");
        if p.is_file() && !files.contains(&p) {
            files.insert(0, p);
        }
    }
    // Repo-level config: same walk-up convention `skills` uses.
    if let Ok(cwd) = std::env::current_dir() {
        let mut cur: Option<&Path> = Some(cwd.as_path());
        while let Some(d) = cur {
            for name in ["agent-qa.toml", ".agent-qa.toml"] {
                let candidate = d.join(name);
                if candidate.is_file() && !files.contains(&candidate) {
                    files.push(candidate);
                }
            }
            cur = d.parent();
        }
    }

    let mut dirs = Vec::new();
    for toml_path in files {
        let Ok(bytes) = fs::read_to_string(&toml_path) else {
            continue;
        };
        let Ok(cfg) = toml::from_str::<RecordConfig>(&bytes) else {
            continue;
        };
        let section = match table {
            "personas" => cfg.personas,
            "environments" => cfg.environments,
            _ => None,
        };
        let Some(specs) = section.and_then(|s| s.extra_dirs) else {
            continue;
        };
        let base = toml_path.parent().unwrap_or_else(|| Path::new("."));
        for spec in specs {
            let expanded = crate::global_config::expand_tilde(&spec);
            dirs.push(if expanded.is_absolute() {
                expanded
            } else {
                base.join(expanded)
            });
        }
    }
    dirs
}

/// Names the scenario declares in `inputs` — used to scope environment
/// params to references the scenario can actually resolve.
fn declared_input_names(opts: &RunOptions) -> Option<BTreeSet<String>> {
    let file = match &opts.source {
        ScenarioSource::Sid(sid) => paths::scenario_dir(sid).ok()?.join("scenario.json"),
        ScenarioSource::Path(p) => p.clone(),
    };
    let body = fs::read_to_string(&file).ok()?;
    let doc: Json = serde_json::from_str(&body).ok()?;
    let keys = doc.get("inputs")?.as_object()?;
    Some(keys.keys().cloned().collect())
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

    /// The sid `t` scenario `base_opts()` points at — writes a scenario.json
    /// declaring the given input names so environment params can merge.
    fn write_scenario_with_inputs(root: &Path, input_names: &[&str]) {
        let dir = root.join("t");
        fs::create_dir_all(&dir).unwrap();
        let inputs: serde_json::Map<String, Json> = input_names
            .iter()
            .map(|n| (n.to_string(), json!({"type": "string"})))
            .collect();
        fs::write(
            dir.join("scenario.json"),
            json!({"schema": "scenario/1", "title": "t", "steps": [], "inputs": Json::Object(inputs)})
                .to_string(),
        )
        .unwrap();
    }

    /// A package-style record dir (flat `<id>.json` files) plus a global
    /// config file registering it under `[<table>] extra-dirs`.
    fn write_package_dir(root: &Path, table: &str, record: Json) -> PathBuf {
        let home = root.join("qa-home");
        let pkg = root.join("pkg-records");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&pkg).unwrap();
        let id = record
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        fs::write(pkg.join(format!("{id}.json")), record.to_string()).unwrap();
        fs::write(
            home.join("agent-qa.toml"),
            // Forward slashes so the path stays a valid TOML string on Windows
            // (backslashes would read as escapes and break the parse).
            format!(
                "[{table}]\nextra-dirs = [\"{}\"]\n",
                pkg.display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();
        std::env::set_var("AGENT_QA_HOME", &home);
        home
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
        write_scenario_with_inputs(tmp.path(), &["baseUrl", "tenant", "locale"]);
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
    fn environment_params_skip_undeclared_inputs() {
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
        // The scenario declares only `baseUrl` — the environment's other
        // params are ambient context, not errors.
        write_scenario_with_inputs(tmp.path(), &["baseUrl"]);
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let mut opts = opts_with_env("staging");
        apply(&mut opts).unwrap();
        assert_eq!(
            opts.input_overrides.get("baseUrl").map(String::as_str),
            Some("https://staging.example.com")
        );
        assert!(!opts.input_overrides.contains_key("tenant"));
        assert!(!opts.input_overrides.contains_key("locale"));
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
    }

    #[test]
    fn environment_params_merge_nothing_for_inputless_scenario() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        write_environment(
            tmp.path(),
            "staging",
            json!({
                "schema": "environment/1", "id": "staging",
                "baseUrl": "https://staging.example.com",
                "params": { "tenant": "acme" },
                "auth": {}
            }),
        );
        // Scenario with no inputs key at all — env params merge nothing
        // rather than tripping the declared-inputs check downstream.
        let dir = tmp.path().join("t");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("scenario.json"),
            json!({"schema": "scenario/1", "title": "t", "steps": []}).to_string(),
        )
        .unwrap();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let mut opts = opts_with_env("staging");
        apply(&mut opts).unwrap();
        assert!(opts.input_overrides.is_empty());
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
    }

    #[test]
    fn package_persona_resolves_via_extra_dirs() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var("HOME", tmp.path()); // isolate from real ~/.agent-qa
        write_package_dir(
            tmp.path(),
            "personas",
            json!({
                "schema": "persona/1", "id": "pack-admin", "profile": "pack-p",
                "credentials": { "entries": {} }
            }),
        );
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let rec = load_persona(tmp.path(), "pack-admin").unwrap();
        assert_eq!(rec.get("profile").and_then(|v| v.as_str()), Some("pack-p"));
        let mut opts = opts_with_persona("pack-admin");
        apply(&mut opts).unwrap();
        assert_eq!(opts.profile.as_deref(), Some("pack-p"));
        assert_eq!(opts.session_name, "pack-p-session");
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
        std::env::remove_var("AGENT_QA_HOME");
    }

    #[test]
    fn package_environment_resolves_via_extra_dirs() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var("HOME", tmp.path());
        write_package_dir(
            tmp.path(),
            "environments",
            json!({
                "schema": "environment/1", "id": "pack-env",
                "baseUrl": "https://pack.example.com", "params": {}, "auth": {}
            }),
        );
        write_scenario_with_inputs(tmp.path(), &["baseUrl"]);
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let mut opts = opts_with_env("pack-env");
        apply(&mut opts).unwrap();
        assert_eq!(
            opts.input_overrides.get("baseUrl").map(String::as_str),
            Some("https://pack.example.com")
        );
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
        std::env::remove_var("AGENT_QA_HOME");
    }

    #[test]
    fn local_persona_shadows_package_record() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var("HOME", tmp.path());
        write_package_dir(
            tmp.path(),
            "personas",
            json!({
                "schema": "persona/1", "id": "admin", "profile": "pack-p",
                "credentials": { "entries": {} }
            }),
        );
        write_persona(
            tmp.path(),
            "admin",
            json!({"schema": "persona/1", "id": "admin", "profile": "local-p",
                   "credentials": { "entries": {} }}),
        );
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let mut opts = opts_with_persona("admin");
        apply(&mut opts).unwrap();
        assert_eq!(opts.profile.as_deref(), Some("local-p"));
        std::env::remove_var("AGENT_QA_SCENARIOS_DIR");
        std::env::remove_var("AGENT_QA_HOME");
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
