//! `fill-unique` verb — fill a uniqueness-constrained field and record
//! the step with the unique-template intact.
//!
//! At record time we substitute `{{vars._unique}}` with a freshly minted
//! 8-hex token, fill the field with the substituted literal, and append a
//! friendly `action`/`fillByLabel` trigger row (the same buffer vocab
//! `record-step` writes) whose value arg carries the **template** (not the
//! substituted value). `flush` translates it into a `do/type` step via the
//! shared `recorder_shape::map_row` path. Replay then re-mints a fresh token
//! each run via `value::substitute_scenario_vars`, so the same scenario can
//! run many times without colliding on uniqueness constraints.
//!
//! CLI shape:
//!
//!   agent-qa fill-unique <Label> --template '<literal-with-{{vars._unique}}>'
//!                              [--save-as <name>] [--no-record]
//!                              [--session <name>]
//!
//! `--save-as <name>` registers a `vars.<name>` binding inside
//! `<record_root>/scenario.fill-unique.bindings.json` so a later
//! `record-step check` referencing `{{vars.<name>}}` resolves to the
//! same minted value at replay time.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::browser;
use crate::paths;
use crate::value::{substitute_scenario_vars, ValueScope};

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    // Resolve the active sid + session from scenario.env.
    let env_file = paths::record_env_file();
    let env_body = fs::read_to_string(&env_file)
        .with_context(|| format!("read {} (was `start` run?)", env_file.display()))?;
    let session = opts
        .session
        .clone()
        .or_else(|| extract(&env_body, "SESSION"))
        .unwrap_or_else(|| "default".to_string());

    // Substitute {{vars._unique}} → freshly minted hex, then fill.
    let mut scope = ValueScope::default();
    let resolved = substitute_scenario_vars(&opts.template, &mut scope);
    browser::find_label_fill(&session, &opts.label, &resolved)
        .with_context(|| format!("agent-browser find label {} fill", opts.label))?;

    // Optional --save-as: stash the resolved value so a later check that
    // references {{vars.<name>}} sees the same token.
    if let Some(name) = &opts.save_as {
        save_binding(name, &resolved)?;
    }

    if opts.record {
        let buffer = paths::record_steps_jsonl();
        let step_index = count_lines(&buffer)? as u32;
        let step_id = format!("s{step_index}");
        // Record a friendly `action`/`fillByLabel` trigger row — the same
        // buffer vocab `record-step` writes — so `flush` translates it through
        // the shared `recorder_shape::map_row` path into the `do/type` step.
        // args[1] keeps the {{vars._unique}} template intact (not the minted
        // value) so replay re-mints; the `--save-as` binding is persisted
        // separately in scenario.fill-unique.bindings.json.
        let payload = json!({
            "method": "fillByLabel",
            "args": [opts.label.clone(), opts.template.clone()],
            "intent": format!("fill-unique {}", opts.label),
        });
        let row = json!({
            "stepIndex": step_index,
            "stepId": step_id,
            "kind": "action",
            "payload": payload,
            "recordedAt": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        });
        append_jsonl(&buffer, &row)?;
        // Keyframe the filled state so this step shows a screenshot in the
        // recording view, just like record-step does.
        if let Some(sid) = extract(&env_body, "SID") {
            crate::record_step::capture_step_sidecars(&sid, &session, &step_id);
        }
        println!(
            "filled {} (step {step_id}) → {}",
            opts.label,
            redact_unique(&resolved)
        );
    } else {
        println!("filled {} → {}", opts.label, redact_unique(&resolved));
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa fill-unique — fill a labelled field with a unique token\n\nUsage:\n  agent-qa fill-unique <Label> --template '<literal-with-{{vars._unique}}>'\n                              [--save-as <name>] [--no-record]\n                              [--session <name>]\n\nAt record time: mints a fresh 8-hex value, fills the labelled field with\nthe substituted literal, appends an action/fillByLabel row carrying the\noriginal template (not the substituted value; flush turns it into a do/type\nstep) so replay re-mints fresh.\n\n--save-as <name> registers a vars.<name> binding so a later record-step\ncheck referencing {{vars.<name>}} sees the same token at replay."
    );
}

#[derive(Debug, Clone)]
struct Opts {
    label: String,
    template: String,
    save_as: Option<String>,
    record: bool,
    session: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Opts> {
    let mut label: Option<String> = None;
    let mut template: Option<String> = None;
    let mut save_as: Option<String> = None;
    let mut record = true;
    let mut session: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                std::process::exit(0);
            }
            "--template" => template = it.next().cloned(),
            s if s.starts_with("--template=") => {
                template = Some(s["--template=".len()..].to_string())
            }
            "--save-as" => save_as = it.next().cloned(),
            s if s.starts_with("--save-as=") => save_as = Some(s["--save-as=".len()..].to_string()),
            "--no-record" => record = false,
            "--session" => session = it.next().cloned(),
            s if s.starts_with("--session=") => session = Some(s["--session=".len()..].to_string()),
            other if other.starts_with("--") => bail!("unknown flag {other:?}"),
            other => {
                if label.is_some() {
                    bail!("unexpected positional {other:?}; usage: fill-unique <Label> --template <…>");
                }
                label = Some(other.to_string());
            }
        }
    }
    let label = label.ok_or_else(|| anyhow!("usage: fill-unique <Label> --template <…>"))?;
    let template = template.ok_or_else(|| anyhow!("--template is required"))?;
    if !template.contains("{{vars._unique}}") {
        bail!("--template must contain '{{{{vars._unique}}}}'; got {template:?}");
    }
    Ok(Opts {
        label,
        template,
        save_as,
        record,
        session,
    })
}

fn extract(env: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    env.lines()
        .find_map(|l| l.strip_prefix(&prefix).map(|v| v.trim().to_string()))
}

fn count_lines(path: &Path) -> Result<u64> {
    let body = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return Ok(0),
    };
    Ok(body.iter().filter(|&&b| b == b'\n').count() as u64)
}

fn append_jsonl(path: &Path, row: &Json) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    use std::fs::OpenOptions;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let line = serde_json::to_string(row)?;
    f.write_all(line.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

fn save_binding(name: &str, value: &str) -> Result<()> {
    let path = paths::record_root().join("scenario.fill-unique.bindings.json");
    let mut map: serde_json::Map<String, Json> = match fs::read(&path) {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => serde_json::Map::new(),
    };
    map.insert(name.to_string(), Json::String(value.to_string()));
    let body = serde_json::to_string_pretty(&Json::Object(map))?;
    fs::create_dir_all(path.parent().unwrap()).ok();
    fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Mask a minted unique-token in human-readable output (8 hex → `<hex>`).
fn redact_unique(s: &str) -> String {
    let re = regex::Regex::new(r"[a-f0-9]{8}").unwrap();
    re.replace_all(s, "<hex>").into_owned()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::browser as ab;
    use crate::test_util::lock_env;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn write_exec(dir: &Path, body: &str) -> std::path::PathBuf {
        let p = dir.join("agent-browser");
        fs::write(&p, body).unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&p, perm).unwrap();
        p
    }

    fn install_fake_logging(dir: &Path, log: &Path) {
        let body = format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", log.display());
        let bin = write_exec(dir, &body);
        std::env::set_var(ab::BIN_ENV, &bin);
        ab::_reset_bin_cache_for_tests();
    }

    fn clear_fake() {
        std::env::remove_var(ab::BIN_ENV);
        ab::_reset_bin_cache_for_tests();
    }

    fn write_env(rec: &Path, sid: &str) {
        fs::create_dir_all(rec).unwrap();
        fs::write(
            rec.join("scenario.env"),
            format!("SID={sid}\nSESSION=default\n"),
        )
        .unwrap();
    }

    fn opts(label: &str, template: &str) -> Opts {
        Opts {
            label: label.into(),
            template: template.into(),
            save_as: None,
            record: true,
            session: None,
        }
    }

    #[test]
    fn parse_args_requires_unique_token_in_template() {
        let err = parse_args(&["Email".into(), "--template".into(), "no-token-here".into()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("vars._unique"));
    }

    #[test]
    fn parse_args_no_record_flag() {
        let opts = parse_args(&[
            "Email".into(),
            "--template".into(),
            "qa-{{vars._unique}}@e.com".into(),
            "--no-record".into(),
        ])
        .unwrap();
        assert!(!opts.record);
    }

    #[test]
    fn redact_unique_masks_8_hex_tokens() {
        assert_eq!(redact_unique("qa-deadbeef@e.com"), "qa-<hex>@e.com");
        assert_eq!(redact_unique("plain text"), "plain text");
    }

    #[test]
    fn run_substitutes_unique_invokes_fill_and_appends_row() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        write_env(&rec, "j1");

        run_inner(&opts("Email", "qa-{{vars._unique}}@e.com")).unwrap();

        let invocation = fs::read_to_string(&log).unwrap();
        assert!(
            regex::Regex::new(r"--session default find label Email fill qa-[a-f0-9]{8}@e\.com")
                .unwrap()
                .is_match(&invocation),
            "got: {invocation}"
        );

        let buffer = fs::read_to_string(rec.join("scenario.steps.jsonl")).unwrap();
        assert_eq!(buffer.lines().count(), 1);
        let row: Json = serde_json::from_str(buffer.lines().next().unwrap()).unwrap();
        // Recorded as a friendly trigger row that `flush` can translate
        // (the bug was emitting a raw `do` step `flush`/`verify` reject).
        assert_eq!(row["kind"], "action");
        assert_eq!(row["payload"]["method"], "fillByLabel");
        assert_eq!(row["payload"]["args"][0], "Email");
        assert_eq!(row["payload"]["args"][1], "qa-{{vars._unique}}@e.com");
        // And it must flush into a do/type step that keeps the template
        // (replay re-mints {{vars._unique}} from it).
        let step = crate::recorder_shape::map_row("action", &row["payload"], "s0").unwrap();
        assert_eq!(step["verb"], "type");
        assert_eq!(step["on"]["role"], "textbox");
        assert_eq!(step["on"]["name"], "Email");
        assert_eq!(step["value"]["literal"], "qa-{{vars._unique}}@e.com");

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake();
    }

    #[test]
    fn save_as_writes_bindings_file() {
        let _g = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        let rec = tmp.path().join("rec");
        std::env::set_var(paths::RECORD_DIR_ENV, &rec);
        let log = tmp.path().join("ab.log");
        install_fake_logging(tmp.path(), &log);
        write_env(&rec, "j1");

        let mut o = opts("Email", "qa-{{vars._unique}}@e.com");
        o.save_as = Some("my_email".into());
        run_inner(&o).unwrap();

        let bindings: Json = serde_json::from_slice(
            &fs::read(rec.join("scenario.fill-unique.bindings.json")).unwrap(),
        )
        .unwrap();
        let v = bindings["my_email"].as_str().unwrap();
        assert!(regex::Regex::new(r"^qa-[a-f0-9]{8}@e\.com$")
            .unwrap()
            .is_match(v));

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        clear_fake();
    }

    fn run_inner(opts: &Opts) -> Result<()> {
        let env_file = paths::record_env_file();
        let env_body = fs::read_to_string(&env_file)?;
        let session = opts
            .session
            .clone()
            .or_else(|| extract(&env_body, "SESSION"))
            .unwrap_or_else(|| "default".to_string());
        let mut scope = ValueScope::default();
        let resolved = substitute_scenario_vars(&opts.template, &mut scope);
        browser::find_label_fill(&session, &opts.label, &resolved)?;
        if let Some(name) = &opts.save_as {
            save_binding(name, &resolved)?;
        }
        if opts.record {
            let buffer = paths::record_steps_jsonl();
            let step_index = count_lines(&buffer)? as u32;
            let step_id = format!("s{step_index}");
            let payload = serde_json::json!({
                "method": "fillByLabel",
                "args": [opts.label.clone(), opts.template.clone()],
                "intent": format!("fill-unique {}", opts.label),
            });
            let row = serde_json::json!({
                "stepIndex": step_index,
                "stepId": step_id,
                "kind": "action",
                "payload": payload,
                "recordedAt": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            });
            append_jsonl(&buffer, &row)?;
        }
        Ok(())
    }
}
