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
use std::process::Command;

const TOML: &str = r#"# agent-qa configuration — see docs/configuration.md.
[paths]
scenarios_root = "./scenarios"

# Golden storage: where shot/domshot baselines live. The default is
# local — PNGs committed inside <sid>/baselines/ with the repo.
# Remote stores keep binary goldens out of the source repo's history;
# `agent-qa baselines push|pull|status` syncs them, replay pulls before
# running, and minting verbs push after accepting.
#
# [baselines]
# store = "github"                          # a second GitHub repo (free)
# repo  = "owner/agent-qa-goldens"
# token_env = "GITHUB_TOKEN"                # env var holding a contents:write token
#
# [baselines]
# store = "turso"                           # a libSQL db (Turso free tier)
# url   = "libsql://<db>-<org>.turso.io"
# token_env = "TURSO_API_KEY"               # env var holding the db JWT
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

/// The full golden loop as a PR gate: replay the suite, publish drift
/// images + video to a `shot-diffs` branch and comment them inline, then
/// either approve the waiting `goldens apply` job or comment
/// `goldens apply` on the PR to re-mint baselines into the store.
/// Uses the published package, so the repo only needs scenarios +
/// baselines committed. For a real approve-gate create a `goldens`
/// environment in repo Settings → Environments with a required reviewer.
const CI_WORKFLOW: &str = r####"name: agent-qa
# Replays every scenario under scenarios/ on each pull request. A golden
# failure publishes baseline/actual/diff images + the replay video to the
# shot-diffs branch and comments them inline; re-mint by approving the
# `goldens apply` job on the PR checks, or by commenting `goldens apply`.
on:
  push:
    branches: [main]   # mint_on_push only — branch pushes run via pull_request
  pull_request:
  issue_comment:
    types: [created]
  workflow_dispatch:

permissions:
  contents: write       # drift images + local-store baselines commits
  pull-requests: write  # drift + verdict comments
  issues: write         # `goldens apply` confirmation comment

jobs:
  replay:
    name: replay goldens
    if: github.event_name == 'push' || github.event_name == 'pull_request'
    runs-on: ubuntu-latest
    timeout-minutes: 30
    outputs:
      drifted: ${{ steps.publish.outputs.drifted }}
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
          npm install --no-audit --no-fund @rasmusjosefsson/agent-qa
          sudo apt-get install -y ffmpeg >/dev/null
          ./node_modules/.bin/agent-browser install
      # Scenarios replaying against a local app? Start it before replay:
      # - name: start the app
      #   run: (npm run dev &) && sleep 5
      - name: lint scenarios
        run: ./node_modules/.bin/agent-qa scenario check-all
      - name: replay suite
        id: gate
        working-directory: .
        env:
          AGENT_BROWSER_BIN: ${{ github.workspace }}/node_modules/.bin/agent-browser
          # Read only when [baselines] store = "turso"; harmless otherwise.
          TURSO_API_KEY: ${{ secrets.TURSO_API_KEY }}
        run: ./node_modules/.bin/agent-qa replay --all --quiet --record-video --report /tmp/qa-report.md
        # sharded: replay --all --shard ${{ matrix.shard }}/2 --quiet --record-video --report /tmp/qa-report.md
      - name: upload run evidence
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: agent-qa-runs
          path: scenarios/*/replays/
          retention-days: 14
      - name: publish shot diffs + comment on the PR
        id: publish
        if: failure() && github.event_name == 'pull_request'
        env:
          # Set a QA_COMMENT_TOKEN secret (a PAT) to get real inline video
          # players in the drift comment — user tokens may upload
          # attachments; the workflow token cannot.
          GH_TOKEN: ${{ secrets.QA_COMMENT_TOKEN || secrets.GITHUB_TOKEN }}
          PR_NUMBER: ${{ github.event.pull_request.number }}
        run: |
          set -e
          git config user.name "agent-qa-bot"
          git config user.email "actions@users.noreply.github.com"
          diffs=$(find scenarios -path '*/replays/*/shots-diff/*.diff.png' | sort || true)
          if [ -z "$diffs" ]; then
            echo "no shot diffs — replay failed on a non-visual claim"
            gh pr comment "$PR_NUMBER" --body "agent-qa replay failed on a non-visual claim — see the workflow log and the \`agent-qa-runs\` artifact (video included)."
            exit 0
          fi
          echo "drifted=1" >> "$GITHUB_OUTPUT"
          stamp="${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
          # Collect BEFORE switching branches — checkout drops the
          # committed baselines from the working tree.
          staging=$(mktemp -d)
          for f in $diffs; do
            run_dir=$(dirname "$(dirname "$f")")
            sid_dir=$(dirname "$(dirname "$run_dir")")
            sid=$(basename "$sid_dir")
            shot=$(basename "$f" .diff.png)
            mkdir -p "$staging/$sid"
            cp "$f" "$staging/$sid/$shot.diff.png"
            if [ -f "$sid_dir/baselines/$shot.png" ]; then cp "$sid_dir/baselines/$shot.png" "$staging/$sid/$shot.baseline.png"; fi
            if [ -f "$run_dir/screenshots/$shot.png" ]; then cp "$run_dir/screenshots/$shot.png" "$staging/$sid/$shot.actual.png"; fi
            for v in mp4 webm; do if [ -f "$run_dir/run.$v" ]; then cp "$run_dir/run.$v" "$staging/$sid/run.$v"; fi; done
            # GitHub renders images inline in comments but never embeds a
            # repo-linked mp4 — transcode a gif so the video plays in place.
            if [ -f "$run_dir/run.mp4" ]; then
              ffmpeg -y -v error -i "$run_dir/run.mp4" -vf "fps=8,scale=640:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer" "$staging/$sid/run.gif" || true
            fi
          done
          # npm install churns package-lock.json — restore tracked files so
          # the branch switch below isn't refused.
          git checkout -- . 2>/dev/null || true
          # APPEND to shot-diffs — a force-pushed orphan would orphan earlier
          # runs' commits and every image link in older comments would 404.
          if git fetch origin shot-diffs:shot-diffs 2>/dev/null; then
            git checkout shot-diffs -q
          else
            git checkout --orphan shot-diffs-tmp -q
            git rm -rf . >/dev/null 2>&1 || true
          fi
          mkdir -p "$stamp"
          cp -r "$staging"/* "$stamp/"
          # Bound the branch — keep the newest 25 runs only.
          ls -d */ 2>/dev/null | grep -E '^[0-9]+-[0-9]+/$' | sort -rn | tail -n +26 | xargs -r rm -rf
          git add -A
          git commit -m "shot diffs for run $stamp" -q
          git push origin HEAD:shot-diffs -q || { git pull --rebase origin shot-diffs -q && git push origin HEAD:shot-diffs -q; }
          # blob/…?raw=1 renders inline in comments for private repos too —
          # raw.githubusercontent.com 404s on private repos regardless of login.
          base="${{ github.server_url }}/${{ github.repository }}/blob/shot-diffs/$stamp"
          run_url="${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}"
          body="**agent-qa golden drift** — shot claims differed from baseline. [Open the run]($run_url) — artifact \`agent-qa-runs\` has every step screenshot."$'\n\n'
          # gh --attach uploads the mp4 as a user-attachment → GitHub
          # renders a real video player inline — but the upload endpoint
          # only accepts user tokens (PAT/OAuth), not the workflow's
          # GITHUB_TOKEN (ghs_*). Attach only for user tokens; the default
          # falls back to an inline gif + mp4 link.
          attach_args=()
          if gh pr comment --help 2>/dev/null | grep -q -- '--attach' && \
             case "$GH_TOKEN" in ghp_*|github_pat_*|gho_*|ghu_*) true;; *) false;; esac; then attach_ok=1; else attach_ok=0; fi
          for sdir in "$staging"/*/; do
            sid=$(basename "$sdir")
            body+="### $sid"$'\n'
            if [ "$attach_ok" = 1 ] && [ -f "$sdir/run.mp4" ]; then
              body+="[replay video]($sdir/run.mp4)"$'\n\n'
              attach_args+=(--attach "$sdir/run.mp4")
            else
              if [ -f "$stamp/$sid/run.gif" ]; then body+="![replay video]($base/$sid/run.gif?raw=1)"$'\n\n'; fi
              for v in mp4 webm; do if [ -f "$stamp/$sid/run.$v" ]; then body+="[full video ($v)]($base/$sid/run.$v?raw=1)"$'\n\n'; break; fi; done
            fi
            for d in "$sdir"*.diff.png; do
              shot=$(basename "$d" .diff.png)
              body+="**$shot** — baseline · actual · diff"$'\n\n'
              if [ -f "$stamp/$sid/$shot.baseline.png" ]; then body+="![baseline]($base/$sid/$shot.baseline.png?raw=1)"$'\n'; fi
              if [ -f "$stamp/$sid/$shot.actual.png" ]; then body+="![actual]($base/$sid/$shot.actual.png?raw=1)"$'\n'; fi
              body+="![diff]($base/$sid/$shot.diff.png?raw=1)"$'\n\n'
            done
          done
          body+="If this is the intended UI, comment \`goldens apply\` to re-mint the stored baselines — or approve the waiting \`goldens apply\` job below (previous versions stay in the store — \`agent-qa baselines revert <sid>\` rolls back). To fix instead, push a commit; replay reruns on the new head."
          gh pr comment "$PR_NUMBER" --body "$body" "${attach_args[@]}"
          echo "## agent-qa golden drift" >> "$GITHUB_STEP_SUMMARY"
          echo "[Diff images + video]($base/) · [run]($run_url)" >> "$GITHUB_STEP_SUMMARY"
      - name: comment verdict on the PR
        if: success() && github.event_name == 'pull_request'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          RUN_URL: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}
        run: |
          printf '\n\n[run artifacts](%s/artifacts)\n' "$RUN_URL" >> /tmp/qa-report.md
          gh pr comment ${{ github.event.pull_request.number }} --body-file /tmp/qa-report.md

  # Approve/deny gate: on a golden failure this job waits on the `goldens`
  # environment. Create it in Settings → Environments with a required
  # reviewer — without it the job runs immediately when a replay fails.
  # Only offered when shot claims actually drifted — approving a mint
  # on a non-visual failure would store baselines rendered from a
  # broken page.
  goldens_apply:
    name: goldens apply (approve to re-mint)
    needs: replay
    if: always() && needs.replay.result == 'failure' && needs.replay.outputs.drifted == '1' && github.event_name == 'pull_request'
    runs-on: ubuntu-latest
    timeout-minutes: 30
    environment: goldens
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.pull_request.head.sha }}
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - name: install agent-qa + browser
        run: |
          npm install --no-audit --no-fund @rasmusjosefsson/agent-qa
          ./node_modules/.bin/agent-browser install
      # Start your app here too if scenarios need it running.
      - name: re-mint baselines + push to the store
        env:
          AGENT_BROWSER_BIN: ${{ github.workspace }}/node_modules/.bin/agent-browser
          TURSO_API_KEY: ${{ secrets.TURSO_API_KEY }}
        run: |
          # --update-baselines mints at teardown but still exits 1 on the
          # failed claim — the mint itself is what we want.
          ./node_modules/.bin/agent-qa replay --all --quiet --update-baselines || true
          ./node_modules/.bin/agent-qa baselines push --all
      - name: commit local-store goldens back to the PR
        if: github.event.pull_request.head.repo.full_name == github.repository
        run: |
          git config user.name "agent-qa-bot"
          git config user.email "actions@users.noreply.github.com"
          git checkout -B "${{ github.head_ref }}"
          git add scenarios/*/baselines/
          if git diff --cached --quiet; then
            echo "remote store or no local baseline change — nothing to commit"
          else
            git commit -m "goldens: re-mint baselines" -q
            git push origin "HEAD:${{ github.head_ref }}"
          fi
      - name: confirm on the PR
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          PR_NUMBER: ${{ github.event.pull_request.number }}
        run: |
          gh pr comment "$PR_NUMBER" --body "Goldens re-minted from the approved run and pushed to the store. Revert with \`agent-qa baselines revert <sid>\` if that approval was a mistake."

  # Comment trigger: `goldens apply` on a PR does the same re-mint without
  # the environment gate. Gated to repo members — an outsider's comment
  # must not be able to overwrite the goldens store.
  apply:
    name: re-mint goldens (comment)
    if: >-
      github.event_name == 'issue_comment' &&
      github.event.issue.pull_request &&
      contains(github.event.comment.body, 'goldens apply') &&
      contains(fromJSON('["OWNER","MEMBER","COLLABORATOR"]'), github.event.comment.author_association)
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
        with:
          ref: refs/pull/${{ github.event.issue.number }}/head
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - name: install agent-qa + browser
        run: |
          npm install --no-audit --no-fund @rasmusjosefsson/agent-qa
          ./node_modules/.bin/agent-browser install
      # Start your app here too if scenarios need it running.
      - name: re-mint baselines + push to the store
        env:
          AGENT_BROWSER_BIN: ${{ github.workspace }}/node_modules/.bin/agent-browser
          TURSO_API_KEY: ${{ secrets.TURSO_API_KEY }}
        run: |
          ./node_modules/.bin/agent-qa replay --all --quiet --update-baselines || true
          ./node_modules/.bin/agent-qa baselines push --all
      - name: commit local-store goldens back to the PR
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          set -e
          head=$(gh pr view "${{ github.event.issue.number }}" --json headRefName,headRepository --jq 'select(.headRepository.nameWithOwner == "${{ github.repository }}") | .headRefName')
          if [ -z "$head" ]; then
            echo "forked PR — cannot push; mint locally and commit instead"
            exit 0
          fi
          git config user.name "agent-qa-bot"
          git config user.email "actions@users.noreply.github.com"
          git checkout -B "$head"
          git add scenarios/*/baselines/
          if git diff --cached --quiet; then
            echo "remote store or no local baseline change — nothing to commit"
          else
            git commit -m "goldens: re-mint baselines" -q
            git push origin "HEAD:$head"
          fi
      - name: confirm on the PR
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          PR_NUMBER: ${{ github.event.issue.number }}
        run: |
          gh pr comment "$PR_NUMBER" --body "Goldens re-minted from this head and pushed to the store — replay will pass on the next run. Revert with \`agent-qa baselines revert <sid>\` if this was a mistake."

  # Mint-on-merge: goldens track the default branch, so a drifting PR
  # that merges leaves the store stale — the push run's replay fails and
  # this job adopts the merged render. No approval needed: merged =
  # adopted. `baselines revert <sid>` rolls back a bad adopt; the store
  # keeps every version.
  mint_on_push:
    name: mint goldens on default-branch failure
    needs: replay
    if: >-
      always() &&
      needs.replay.result == 'failure' &&
      github.event_name == 'push' &&
      github.ref_name == github.event.repository.default_branch
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - name: install agent-qa + browser
        run: |
          npm install --no-audit --no-fund @rasmusjosefsson/agent-qa
          ./node_modules/.bin/agent-browser install
      # Start your app here too if scenarios need it running.
      - name: re-mint baselines + push to the store
        env:
          AGENT_BROWSER_BIN: ${{ github.workspace }}/node_modules/.bin/agent-browser
          TURSO_API_KEY: ${{ secrets.TURSO_API_KEY }}
        run: |
          ./node_modules/.bin/agent-qa replay --all --quiet --update-baselines || true
          # A store push failure leaves goldens stale — every later PR
          # falsely drifts. Retry once, then fail loudly.
          ./node_modules/.bin/agent-qa baselines push --all || {
            sleep 10
            ./node_modules/.bin/agent-qa baselines push --all
          }
      - name: commit local-store goldens back
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          git config user.name "agent-qa-bot"
          git config user.email "actions@users.noreply.github.com"
          git add scenarios/*/baselines/
          if git diff --cached --quiet; then
            echo "remote store or no local baseline change — nothing to commit"
          else
            git commit -m "goldens: adopt merged render" -q
            git push origin "HEAD:${{ github.ref_name }}"
          fi
      - name: flag stale store on failure
        if: failure()
        run: echo "::error::golden mint failed — the store is now stale; re-run this job or push baselines manually"
"####;

pub fn cli(args: &[String]) -> Result<u8> {
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        println!(
            "agent-qa init — bootstrap an agent-qa scenario directory\n\nUsage:\n  agent-qa init [dir] [--force] [--ci] [--store <local|github|turso>] [--repo <owner/name>] [--url <libsql://…>] [--token-env <NAME>]\n\n  dir         Target directory (default: cwd)\n  --force     Overwrite existing files (scenario.json, agent-qa.toml)\n  --ci        Also write .github/workflows/agent-qa.yml — the golden loop\n              (replay gate + drift images/video comment + `goldens apply`)\n  --store     Golden storage backend. `local` (default) commits baselines\n              with the repo; `github` uses a second repo (detected from\n              `git remote get-url origin` or --repo); `turso` uses a libSQL\n              db (requires --url).\n  --repo      owner/name for --store github (default: detect from origin)\n  --url       libsql://… db URL for --store turso\n  --token-env Env var the token lives in (default: GITHUB_TOKEN for\n              github, TURSO_AUTH_TOKEN for turso)"
        );
        return Ok(0);
    }
    let mut root: Option<PathBuf> = None;
    let mut force = false;
    let mut ci = false;
    let mut store = Store::Local;
    let mut repo: Option<String> = None;
    let mut url: Option<String> = None;
    let mut token_env: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let mut val = |flag: &str| -> Result<String> {
            i += 1;
            args.get(i)
                .cloned()
                .with_context(|| format!("init: {flag} needs a value"))
        };
        match a.as_str() {
            "--force" => force = true,
            "--ci" => ci = true,
            "--store" => {
                store = match val("--store")?.as_str() {
                    "local" => Store::Local,
                    "github" => Store::Github,
                    "turso" => Store::Turso,
                    other => bail!("init: --store {other:?} — expected local, github, or turso"),
                };
            }
            "--repo" => repo = Some(val("--repo")?),
            "--url" => url = Some(val("--url")?),
            "--token-env" => token_env = Some(val("--token-env")?),
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
        i += 1;
    }
    let root = match root {
        Some(r) if r.is_absolute() => r,
        Some(r) => std::env::current_dir().context("cwd")?.join(r),
        None => std::env::current_dir().context("cwd")?,
    };
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    let baselines = baselines_block(&root, store, repo, url, token_env)?;
    init_at(&root, force, ci, baselines.as_deref(), &mut |line| {
        println!("{line}")
    })?;
    Ok(0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Store {
    Local,
    Github,
    Turso,
}

/// The `[baselines]` TOML block for a remote store, or None for local.
/// `repo`/`url`/`token_env` are the raw flag values; github's repo falls
/// back to the `owner/name` parsed from `git remote get-url origin`.
fn baselines_block(
    root: &Path,
    store: Store,
    repo: Option<String>,
    url: Option<String>,
    token_env: Option<String>,
) -> Result<Option<String>> {
    match store {
        Store::Local => Ok(None),
        Store::Github => {
            let repo = match repo {
                Some(r) => r,
                None => detect_github_repo(root).context(
                    "init: --store github needs a repo — pass --repo owner/name \
                     (could not parse one from `git remote get-url origin`)",
                )?,
            };
            let token_env = token_env.unwrap_or_else(|| "GITHUB_TOKEN".into());
            Ok(Some(format!(
                "[baselines]\nstore = \"github\"\nrepo = \"{repo}\"\ntoken_env = \"{token_env}\"\n"
            )))
        }
        Store::Turso => {
            let url = url.context("init: --store turso needs --url <libsql://…>")?;
            let env = token_env.unwrap_or_else(|| "TURSO_AUTH_TOKEN".into());
            Ok(Some(format!(
                "[baselines]\nstore = \"turso\"\nurl = \"{url}\"\ntoken_env = \"{env}\"\n"
            )))
        }
    }
}

/// `owner/name` from `git remote get-url origin` — handles
/// `git@host:owner/repo(.git)`, `ssh://git@host/owner/repo(.git)` and
/// `https://host/owner/repo(.git)` shapes.
fn detect_github_repo(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout);
    github_repo_from_url(url.trim())
}

/// `owner/name` from a git remote URL — handles
/// `git@host:owner/repo(.git)`, `ssh://git@host/owner/repo(.git)` and
/// `https://host/owner/repo(.git)` shapes.
fn github_repo_from_url(url: &str) -> Option<String> {
    if url.starts_with('/') || url.starts_with('.') || url.starts_with("file:") {
        return None;
    }
    let path = url
        .rsplit_once(':')
        .map(|(_, p)| p)
        .unwrap_or(url)
        .trim_start_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.rsplitn(3, '/');
    let name = parts.next()?;
    let owner = parts.next()?;
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(format!("{owner}/{name}"))
}

fn init_at(
    root: &Path,
    force: bool,
    ci: bool,
    baselines: Option<&str>,
    note: &mut dyn FnMut(&str),
) -> Result<()> {
    // 1. agent-qa.toml — pin scenarios under ./scenarios so goldens commit.
    let toml_path = root.join("agent-qa.toml");
    let toml_exists = toml_path.exists();
    if !toml_exists {
        let mut body = TOML.to_string();
        if let Some(b) = baselines {
            body.push('\n');
            body.push_str(b);
        }
        fs::write(&toml_path, body).with_context(|| format!("write {}", toml_path.display()))?;
        note("created agent-qa.toml     (scenarios_root = ./scenarios)");
    } else {
        let body = fs::read_to_string(&toml_path)?;
        let mut out = body.clone();
        let mut changed = false;
        if force && !out.contains("[paths]") {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("\n[paths]\nscenarios_root = \"./scenarios\"\n");
            changed = true;
        }
        if let Some(b) = baselines {
            if !out.contains("[baselines]") {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
                out.push_str(b);
                changed = true;
            }
        }
        if changed {
            fs::write(&toml_path, out).with_context(|| format!("write {}", toml_path.display()))?;
            note("updated agent-qa.toml     (added missing sections)");
        } else {
            note("kept    agent-qa.toml     (already present)");
        }
    }
    if let Some(b) = baselines {
        note(&format!(
            "        golden store: {}",
            b.lines().nth(1).unwrap_or("?")
        ));
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
            note("created .github/workflows/agent-qa.yml  (golden loop: replay + drift comment + goldens apply)");
        } else {
            note("kept    .github/workflows/agent-qa.yml  (already present)");
        }
    }

    note("");
    note("next: agent-qa replay hello          # smoke-check the setup");
    note("      agent-qa start \"my flow\"      # record your first scenario");
    if ci {
        note("      # .github/workflows/agent-qa.yml runs the golden loop on every PR");
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
        init_at(tmp.path(), false, false, None, &mut |l| {
            lines.push(l.to_string())
        })
        .unwrap();
        assert!(tmp.path().join("agent-qa.toml").exists());
        assert!(tmp.path().join("scenarios/hello/scenario.json").exists());
        let gi = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(gi.contains("scenarios/*/replays/"));
        // second run keeps everything
        let mut lines2 = Vec::new();
        init_at(tmp.path(), false, false, None, &mut |l| {
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
        init_at(tmp.path(), true, false, None, &mut |l| {
            lines2.push(l.to_string())
        })
        .unwrap();
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("[paths]") && toml.contains("scenarios_root"));
    }

    #[test]
    fn init_ci_writes_pr_gate_workflow() {
        let tmp = TempDir::new().unwrap();
        let mut lines = Vec::new();
        init_at(tmp.path(), false, true, None, &mut |l| {
            lines.push(l.to_string())
        })
        .unwrap();
        let wf = tmp.path().join(".github/workflows/agent-qa.yml");
        let body = fs::read_to_string(&wf).unwrap();
        assert!(body.contains("pull_request:") && body.contains("replay --all"));
        // push is scoped to main so branch pushes don't double-run with pull_request
        assert!(body.contains("branches: [main]"));
        assert!(body.contains("goldens_apply") && body.contains("shot-diffs"));
        assert!(body.contains("@rasmusjosefsson/agent-qa"));
        // second run leaves it alone
        fs::write(&wf, "custom").unwrap();
        init_at(tmp.path(), false, true, None, &mut |_| {}).unwrap();
        assert_eq!(fs::read_to_string(&wf).unwrap(), "custom");
    }

    #[test]
    fn init_store_flags_write_baselines_table() {
        // github with an explicit repo
        let tmp = TempDir::new().unwrap();
        init_at(
            tmp.path(),
            false,
            false,
            baselines_block(tmp.path(), Store::Github, Some("o/g".into()), None, None)
                .unwrap()
                .as_deref(),
            &mut |_| {},
        )
        .unwrap();
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("[baselines]"));
        assert!(toml.contains("store = \"github\""));
        assert!(toml.contains("repo = \"o/g\""));
        assert!(toml.contains("token_env = \"GITHUB_TOKEN\""));
        // the emitted table parses as the real config
        let cfg: crate::paths::ConfigFile = toml::from_str(&toml).unwrap();
        assert_eq!(cfg.baselines.unwrap().repo.as_deref(), Some("o/g"));

        // turso requires --url
        assert!(baselines_block(tmp.path(), Store::Turso, None, None, None).is_err());
        let b = baselines_block(
            tmp.path(),
            Store::Turso,
            None,
            Some("libsql://d-o.turso.io".into()),
            Some("MY_TOKEN".into()),
        )
        .unwrap()
        .unwrap();
        assert!(b.contains("store = \"turso\"") && b.contains("MY_TOKEN"));

        // turso table appended to an existing toml that lacks [baselines]
        let tmp2 = TempDir::new().unwrap();
        fs::write(tmp2.path().join("agent-qa.toml"), "[paths]\n").unwrap();
        init_at(
            tmp2.path(),
            false,
            false,
            baselines_block(
                tmp2.path(),
                Store::Turso,
                None,
                Some("libsql://d-o.turso.io".into()),
                None,
            )
            .unwrap()
            .as_deref(),
            &mut |_| {},
        )
        .unwrap();
        let toml = fs::read_to_string(tmp2.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("store = \"turso\"") && toml.contains("TURSO_AUTH_TOKEN"));
    }

    #[test]
    fn github_repo_from_url_parses_remote_shapes() {
        for (url, want) in [
            ("git@github.com:o/r.git", Some("o/r".into())),
            ("git@github.com:o/r", Some("o/r".into())),
            ("https://github.com/o/r.git", Some("o/r".into())),
            ("ssh://git@github.com/o/r.git", Some("o/r".into())),
            ("https://ghes.acme.io/o/r", Some("o/r".into())),
            ("file:///tmp/bare.git", None),
        ] {
            assert_eq!(github_repo_from_url(url), want, "{url}");
        }
    }

    /// `init --help` prints usage (it used to fail "unknown flag").
    #[test]
    fn help_flag_prints_usage() {
        for args in [vec!["--help".to_string()], vec!["-h".to_string()]] {
            assert_eq!(cli(&args).unwrap(), 0, "args {args:?}");
        }
    }
}
