//! `fill-unique` verb — fill a uniqueness-constrained field and record
//! the step with the unique-template intact.
//!
//! At record time we substitute `{{vars._unique}}` with a freshly minted
//! 8-hex token, fill the field with the substituted literal, and append a
//! replay-native `do/type` draft with the template value intact. Replay then
//! mints a fresh token through `value::substitute_scenario_vars`, so the same
//! scenario can run again without colliding on uniqueness constraints.
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

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value as Json};

use crate::browser;
use crate::paths;
use crate::record_step::StepKind;
use crate::recorder_state::RecorderState;
use crate::value::{substitute_scenario_vars, ValueScope};

pub fn run(args: &[String]) -> Result<u8> {
    let opts = parse_args(args)?;

    let mut state = RecorderState::load_active()?;
    let session = opts
        .session
        .clone()
        .unwrap_or_else(|| state.session.clone());

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
        let payload = json!({
            "intent": format!("fill-unique {}", opts.label),
            "verb": "type",
            "on": { "role": "textbox", "name": opts.label },
            "value": { "from": "literal", "literal": opts.template },
        });
        let row = crate::record_step::record_draft(&mut state, StepKind::Do, &payload, &session)?;
        println!(
            "filled {} (step {}) → {}",
            opts.label,
            row.step_id,
            redact_unique(&resolved)
        );
    } else {
        println!("filled {} → {}", opts.label, redact_unique(&resolved));
    }
    Ok(0)
}

fn print_help() {
    println!(
        "agent-qa fill-unique — fill a labelled field with a unique token\n\nUsage:\n  agent-qa fill-unique <Label> --template '<literal-with-{{vars._unique}}>'\n                              [--save-as <name>] [--no-record]\n                              [--session <name>]\n\nAt record time, this command mints a fresh 8-hex value and fills the field.\nIt records a replay-native do/type draft with the original template, so\nreplay mints a fresh value.\n\n--save-as <name> registers a vars.<name> binding so a later record-step\ncheck referencing {{vars.<name>}} sees the same token at replay."
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
    use crate::browser::{self, BrowserConnection};
    use crate::recorder_state::RecorderBaseline;
    use crate::test_util::lock_env;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn fill_unique_records_a_direct_type_step() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        let binary = tmp.path().join("agent-browser");
        fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, &binary);
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path().join("record"));
        browser::_reset_bin_cache_for_tests();
        RecorderState::new(
            "s1".into(),
            "record".into(),
            "default".into(),
            RecorderBaseline::Fresh,
            None,
            BrowserConnection::default(),
        )
        .save()
        .unwrap();
        run(&[
            "Email".into(),
            "--template".into(),
            "qa-{{vars._unique}}@example.com".into(),
        ])
        .unwrap();
        let step = serde_json::to_value(&RecorderState::load_active().unwrap().steps[0]).unwrap();
        assert_eq!(step["kind"], "do");
        assert_eq!(step["verb"], "type");
        assert_eq!(step["value"]["literal"], "qa-{{vars._unique}}@example.com");
        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }
}
