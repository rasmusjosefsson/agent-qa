//! `agent-qa init` — bootstrap a directory for scenario authoring.
//!
//! Writes `agent-qa.toml` (pinning `scenarios_root = "./scenarios"`),
//! creates the scenarios dir with a `hello` smoke scenario, and adds the
//! replay/record artifact paths to `.gitignore` so `scenario.json` +
//! `baselines/` commit while runs stay untracked. Idempotent: existing
//! files are left alone unless `--force` is passed (and even then the
//! toml is only rewritten if it lacks a `[paths]` table).

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const TOML: &str = r#"# agent-qa configuration — see docs/configuration.md.
[paths]
scenarios_root = "./scenarios"
"#;

const HELLO: &str = r#"{
  "schema": "scenario/2",
  "id": "hello",
  "intent": "Smoke check — the example page loads and its URL is right",
  "env": { "open": [{ "kind": "fresh" }] },
  "steps": [
    {
      "id": "s0",
      "intent": "Open the example page",
      "kind": "do",
      "verb": "goto",
      "value": { "from": "literal", "literal": "https://example.com/" }
    },
    {
      "id": "s1",
      "intent": "URL is the example page",
      "kind": "check",
      "claim": {
        "subject": { "url": true },
        "predicate": "contains",
        "value": { "from": "literal", "literal": "example.com" }
      }
    }
  ]
}
"#;

const GITIGNORE_BLOCK: &str = "# agent-qa run artifacts (scenario.json + baselines/ stay tracked)\ntmp/agent-qa-scenarios/\ntmp/agent-qa-record/\nscenarios/*/replays/\nscenarios/*/shots-diff/\n";

pub fn cli(args: &[String]) -> Result<u8> {
    let mut root: Option<PathBuf> = None;
    let mut force = false;
    for a in args {
        match a.as_str() {
            "--force" => force = true,
            v if v.starts_with("--") => bail!("init: unknown flag {v:?}"),
            v => {
                if root.is_some() {
                    bail!("unexpected positional {v:?}; usage: agent-qa init [dir] [--force]");
                }
                root = Some(PathBuf::from(v));
            }
        }
    }
    let root = match root {
        Some(r) if r.is_absolute() => r,
        Some(r) => std::env::current_dir().context("cwd")?.join(r),
        None => std::env::current_dir().context("cwd")?,
    };
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    init_at(&root, force, &mut |line| println!("{line}"))?;
    Ok(0)
}

fn init_at(root: &Path, force: bool, note: &mut dyn FnMut(&str)) -> Result<()> {
    // 1. agent-qa.toml — pin scenarios under ./scenarios so goldens commit.
    let toml_path = root.join("agent-qa.toml");
    let toml_exists = toml_path.exists();
    if !toml_exists {
        fs::write(&toml_path, TOML).with_context(|| format!("write {}", toml_path.display()))?;
        note("created agent-qa.toml     (scenarios_root = ./scenarios)");
    } else {
        let body = fs::read_to_string(&toml_path)?;
        if force && !body.contains("[paths]") {
            let mut out = body;
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("\n[paths]\nscenarios_root = \"./scenarios\"\n");
            fs::write(&toml_path, out).with_context(|| format!("write {}", toml_path.display()))?;
            note("updated agent-qa.toml     (added [paths].scenarios_root)");
        } else {
            note("kept    agent-qa.toml     (already present)");
        }
    }

    // 2. scenarios/ + hello scenario.
    let scenarios_dir = root.join("scenarios");
    fs::create_dir_all(&scenarios_dir)
        .with_context(|| format!("create {}", scenarios_dir.display()))?;
    let hello = scenarios_dir.join("hello").join("scenario.json");
    if !hello.exists() || force {
        fs::create_dir_all(hello.parent().unwrap())?;
        fs::write(&hello, HELLO).with_context(|| format!("write {}", hello.display()))?;
        note("created scenarios/hello/scenario.json");
    } else {
        note("kept    scenarios/hello/scenario.json");
    }

    // 3. .gitignore — replay/record artifacts stay untracked.
    let gitignore = root.join(".gitignore");
    let existing = fs::read_to_string(&gitignore).unwrap_or_default();
    if !existing.contains("agent-qa run artifacts") {
        let mut out = existing;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(GITIGNORE_BLOCK);
        fs::write(&gitignore, out).with_context(|| format!("write {}", gitignore.display()))?;
        note("updated .gitignore        (replays, shots-diff, record state)");
    } else {
        note("kept    .gitignore        (agent-qa block already present)");
    }

    note("");
    note("next: agent-qa replay hello          # smoke-check the setup");
    note("      agent-qa start \"my flow\"      # record your first scenario");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn init_writes_toml_hello_and_gitignore_idempotently() {
        let tmp = TempDir::new().unwrap();
        let mut lines = Vec::new();
        init_at(tmp.path(), false, &mut |l| lines.push(l.to_string())).unwrap();
        assert!(tmp.path().join("agent-qa.toml").exists());
        assert!(tmp.path().join("scenarios/hello/scenario.json").exists());
        let gi = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(gi.contains("scenarios/*/replays/"));
        // second run keeps everything
        let mut lines2 = Vec::new();
        init_at(tmp.path(), false, &mut |l| lines2.push(l.to_string())).unwrap();
        assert!(lines2
            .iter()
            .all(|l| !l.starts_with("created") && !l.starts_with("updated") || l.is_empty()));
        // hello scenario validates + parses
        let bytes = fs::read(tmp.path().join("scenarios/hello/scenario.json")).unwrap();
        let v = crate::schema::validate_bytes(&bytes).unwrap();
        let sc: crate::scenario::Scenario = serde_json::from_value(v).unwrap();
        assert_eq!(sc.steps.len(), 2);
        // --force appends [paths] to an existing toml lacking it
        fs::write(tmp.path().join("agent-qa.toml"), "[plugins]\n").unwrap();
        init_at(tmp.path(), true, &mut |l| lines2.push(l.to_string())).unwrap();
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("[paths]") && toml.contains("scenarios_root"));
    }
}
