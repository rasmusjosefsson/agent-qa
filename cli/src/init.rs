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
        "value": "example.com"
      }
    }
  ]
}
"#;

const GITIGNORE_BLOCK: &str = "# agent-qa run artifacts (scenario.json + baselines/ stay tracked)\ntmp/agent-qa-scenarios/\ntmp/agent-qa-record/\nscenarios/*/replays/\nscenarios/*/shots-diff/\nscenarios/*/inputs.local.json\n";

/// A PR gate that replays the whole suite and comments the verdict. Uses the
/// published package, so this repo only needs scenarios + baselines committed.
const CI_WORKFLOW: &str = r#"name: agent-qa
# Replays every scenario under scenarios/ on each pull request and posts a
# sticky verdict comment. Sharding: uncomment the matrix to split the suite.
on:
  pull_request:
  workflow_dispatch:

permissions:
  contents: read
  pull-requests: write

jobs:
  replay:
    name: replay goldens
    runs-on: ubuntu-latest
    timeout-minutes: 30
    # strategy:
    #   matrix:
    #     shard: [1, 2]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - name: install agent-qa + browser
        run: |
          npm install --no-audit --no-fund agent-qa agent-browser
          ./node_modules/.bin/agent-browser install
      - name: lint scenarios
        run: ./node_modules/.bin/agent-qa scenario check-all
      - name: replay suite
        id: gate
        working-directory: .
        env:
          AGENT_BROWSER_BIN: ${{ github.workspace }}/node_modules/.bin/agent-browser
        run: ./node_modules/.bin/agent-qa replay --all --quiet --report /tmp/qa-report.md
        # sharded: replay --all --shard ${{ matrix.shard }}/2 --quiet --report /tmp/qa-report.md
      - name: upload run evidence
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: agent-qa-runs
          path: scenarios/*/replays/
          retention-days: 14
      - name: comment verdict on the PR
        if: always() && github.event_name == 'pull_request'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          RUN_URL: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}
        run: |
          printf '\n\n[run artifacts](%s/artifacts)\n' "$RUN_URL" >> /tmp/qa-report.md
          gh pr comment ${{ github.event.pull_request.number }} --body-file /tmp/qa-report.md
"#;

pub fn cli(args: &[String]) -> Result<u8> {
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        println!(
            "agent-qa init — bootstrap an agent-qa scenario directory\n\nUsage:\n  agent-qa init [dir] [--force] [--ci]\n\n  dir      Target directory (default: cwd)\n  --force  Overwrite existing files (scenario.json, agent-qa.toml)\n  --ci     Also write .github/workflows/agent-qa.yml — a replay-on-PR gate"
        );
        return Ok(0);
    }
    let mut root: Option<PathBuf> = None;
    let mut force = false;
    let mut ci = false;
    for a in args {
        match a.as_str() {
            "--force" => force = true,
            "--ci" => ci = true,
            v if v.starts_with("--") => bail!("init: unknown flag {v:?}"),
            v => {
                if root.is_some() {
                    bail!(
                        "unexpected positional {v:?}; usage: agent-qa init [dir] [--force] [--ci]"
                    );
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
    init_at(&root, force, ci, &mut |line| println!("{line}"))?;
    Ok(0)
}

fn init_at(root: &Path, force: bool, ci: bool, note: &mut dyn FnMut(&str)) -> Result<()> {
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

    // 4. --ci — a PR gate that replays the suite.
    if ci {
        let wf = root.join(".github/workflows/agent-qa.yml");
        if !wf.exists() || force {
            fs::create_dir_all(wf.parent().unwrap())?;
            fs::write(&wf, CI_WORKFLOW).with_context(|| format!("write {}", wf.display()))?;
            note("created .github/workflows/agent-qa.yml  (PR gate: replay --all)");
        } else {
            note("kept    .github/workflows/agent-qa.yml  (already present)");
        }
    }

    note("");
    note("next: agent-qa replay hello          # smoke-check the setup");
    note("      agent-qa start \"my flow\"      # record your first scenario");
    if ci {
        note("      # .github/workflows/agent-qa.yml replays the suite on every PR");
    }
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
        init_at(tmp.path(), false, false, &mut |l| lines.push(l.to_string())).unwrap();
        assert!(tmp.path().join("agent-qa.toml").exists());
        assert!(tmp.path().join("scenarios/hello/scenario.json").exists());
        let gi = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(gi.contains("scenarios/*/replays/"));
        // second run keeps everything
        let mut lines2 = Vec::new();
        init_at(tmp.path(), false, false, &mut |l| {
            lines2.push(l.to_string())
        })
        .unwrap();
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
        init_at(tmp.path(), true, false, &mut |l| lines2.push(l.to_string())).unwrap();
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("[paths]") && toml.contains("scenarios_root"));
    }

    #[test]
    fn init_ci_writes_pr_gate_workflow() {
        let tmp = TempDir::new().unwrap();
        let mut lines = Vec::new();
        init_at(tmp.path(), false, true, &mut |l| lines.push(l.to_string())).unwrap();
        let wf = tmp.path().join(".github/workflows/agent-qa.yml");
        let body = fs::read_to_string(&wf).unwrap();
        assert!(body.contains("pull_request:") && body.contains("replay --all"));
        // second run leaves it alone
        fs::write(&wf, "custom").unwrap();
        init_at(tmp.path(), false, true, &mut |_| {}).unwrap();
        assert_eq!(fs::read_to_string(&wf).unwrap(), "custom");
    }

    /// `init --help` prints usage (it used to fail "unknown flag").
    #[test]
    fn help_flag_prints_usage() {
        for args in [vec!["--help".to_string()], vec!["-h".to_string()]] {
            assert_eq!(cli(&args).unwrap(), 0, "args {args:?}");
        }
    }
}
