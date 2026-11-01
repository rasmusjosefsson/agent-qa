---
name: verify-agent-qa
description: Drive the agent-qa workbench (localhost UI) and its CLI to prove runs, recording, replay, cases, and settings on an isolated scenarios root. Use when changing the workbench, the web server, record/replay, or when a change needs a real browser or CLI proof.
---

# Verify agent-qa

The primary surface is the workbench: `agent-qa web`, a localhost UI for runs, cases, and settings. The CLI is the other user surface (record, flush, replay). Both must talk to the same disposable scenarios root. Do not drive the default workbench on port 7878, and do not write `tmp/agent-qa-scenarios` in the repo.

Scripts live next to this file, under `.agents/skills/verify-agent-qa/`. That directory is the skill. `.cursor/skills/verify-agent-qa` is a symlink to it for Cursor. Run the scripts from anywhere; they locate the repo.

```bash
.agents/skills/verify-agent-qa/scripts/launch.sh
.agents/skills/verify-agent-qa/scripts/doctor.sh
.agents/skills/verify-agent-qa/scripts/browser.sh <agent-browser args>
.agents/skills/verify-agent-qa/scripts/aq.sh <agent-qa args>
.agents/skills/verify-agent-qa/scripts/fixture.sh start|url|stop   # tmux session aqa-fixture-<run-id>
.agents/skills/verify-agent-qa/scripts/cleanup.sh
```

`launch.sh` prints the workbench origin on stdout (`http://127.0.0.1:<port>`, no trailing slash) and writes `/tmp/agent-qa-verify/current`. Append paths as `$VERIFY_URL/cases`. A trailing slash on `VERIFY_URL` makes `$VERIFY_URL/cases` request `//cases`, which the server does not route to Cases. The other scripts load the current run. Evidence goes to `/tmp/agent-qa-verify-evidence/<run-id>/` and survives cleanup.

## Launch

`launch.sh` builds what a fresh checkout does not have, then serves an empty scenarios root:

- Node.js 24 or newer. If `node` on `PATH` is older (`/exec-daemon/node` is Node 22 on the cloud image), the script uses `~/.nvm/versions/node/v24.*/bin/node`, installing it with nvm when that directory is missing.
- `cargo build --locked` of `cli/` when `cli/target/debug/agent-qa` is missing. If that Cargo is too old to read the lockfile (`edition2024`), the script installs the stable toolchain with rustup and retries once with `rustup run stable`.
- `npm install --omit=optional --no-package-lock --ignore-scripts` in `npm/agent-qa` when `agent-browser` is missing. This does not rewrite `package-lock.json`.
- `npm --prefix web ci && npm --prefix web run build` when `npm/agent-qa/lib/public/index.html` is missing.
- `agent-browser install` caches Chrome for Testing under `~/.agent-browser/browsers`. `AGENT_BROWSER_HOME` isolates this run's session sockets from other sessions; it does not move that Chrome download. If the first `browser.sh open` fails with a missing shared library, run `agent-browser install --with-deps` and launch again.

It then starts `agent-qa web --no-open` in a tmux session named `aqa-verify-<run-id>`, on the first free port from 7879 through 7900. Port 7878 is never chosen. Ready means the log contains `serving:` and `GET /` returns the built UI (the HTML references `/assets/`). A live `/api/root` with the wrong `scenariosRoot`, or a page that is only the API, is not ready.

The server command is `node npm/agent-qa/bin/agent-qa.js web --port <N> --root <state>/scenarios --no-open` with `AGENT_QA_BINARY_PATH` pointed at the debug binary. Stdout and stderr are appended to `<state>/server.log`. The ready lines look like:

```text
agent-qa web — run viewer + authoring editor + chat
  scenarios root: /tmp/agent-qa-verify/<run-id>/scenarios
  serving:        http://127.0.0.1:<port>/
  editor:         http://127.0.0.1:<port>/editor
```

`<state>/env.sh` exports `VERIFY_URL`, `VERIFY_ROOT`, `VERIFY_RECORD`, `VERIFY_EVIDENCE`, `VERIFY_BROWSER_SESSION` (`aqa-workbench-<run-id>`), `VERIFY_RECORD_SESSION` (`aqa-record-<run-id>`), `AGENT_QA_SCENARIOS_DIR`, `AGENT_QA_RECORD_DIR`, and `AGENT_BROWSER_HOME`. Source it only through the scripts.

A failed launch deletes its own state directory. It does not delete evidence directories.

## Doctor

Run `doctor.sh` before driving, and again whenever the page is blank or the CLI writes somewhere unexpected. It is read-only. It exits 0 and prints `ok <url>` only when all of these hold:

- The pid in `<state>/server.pid` is alive.
- `GET /api/root` returns `scenariosRoot` equal to `VERIFY_ROOT` and `editor: true`.
- `GET /api/version` returns a non-empty `version`.
- `GET /` serves the built bundle (`/assets/` in the HTML).
- When `lsof` exists, port `VERIFY_PORT` is owned by that pid.
- `agent-qa doctor --json` reports the `agent-browser` probe ok. This runs `--version` only. Do not pass `--deep` here; that launches a browser.

Anything else is not worth driving. Start a new run with `launch.sh`. Do not attach to port 7878.

## Drive

Load the feature map in `features/README.md` and follow one feature file. Browser commands go through `browser.sh`, which pins `--session "$VERIFY_BROWSER_SESSION"` and the run's `AGENT_BROWSER_HOME`. CLI commands go through `aq.sh`, which pins the same scenarios root, record dir, and binary the workbench is using. A bare `agent-qa` writes the repo's `tmp/agent-qa-scenarios` and is the wrong instance.

`browser.sh` is `agent-qa browser`. Stable handles in this UI:

- Sidebar links: `link` named `Runs`, `Chat`, `Editor`, `Cases`, `Sets`, `Plans`, `Goldens`, `Personas`, `Environments`, `Knowledge`, `Extensions`, `Settings`.
- Page headings are `heading` named `Test Cases`, `Settings`, or `Goldens`. Runs uses the text `Scenarios` in the sidebar and either `Record your first scenario` or `No run selected` in the center.
- Buttons used by the map: `New case`, `Create`, `Cancel`, `Save`, `Replay`, `Refresh`, `Crawl`.
- After a case exists, the title field is a `textbox` named `Case title`. The new-case dialog's title field is labeled `Title` (`#case-title`).

First browser command for a UI proof, after `open`:

```bash
.agents/skills/verify-agent-qa/scripts/browser.sh set viewport 1280 800
```

Snapshot and screenshot:

```bash
.agents/skills/verify-agent-qa/scripts/browser.sh snapshot
.agents/skills/verify-agent-qa/scripts/browser.sh screenshot /tmp/agent-qa-verify-evidence/<run-id>/<feature>/page.png
```

`snapshot` prints the accessibility tree on stdout. `snapshot -i` drops non-interactive text such as `No test cases yet`; use a full snapshot when the proof is that text. `screenshot` takes a `.png` path as its only argument. Wait for a CSS or ARIA selector with `browser.sh wait "<selector>"` rather than sleeping.

Recording gestures against the fixture use `aq.sh browser --session "$VERIFY_RECORD_SESSION"`, not `browser.sh`. Mixing those sessions records the workbench instead of the fixture.

## Evidence

Proof is the user path, the resulting UI, and the file that path wrote.

- Put artifacts in `$VERIFY_EVIDENCE/<feature>/`. `launch.sh` creates the directory. `cleanup.sh` does not remove it.
- UI proof is an accessibility snapshot and a screenshot that show the workbench (the `agent-qa` name in the sidebar) and the result of the action. Capture the filled form or the in-progress control as well as the result.
- CLI proof is the command, stdout, stderr, and exit code, saved next to the UI artifacts.
- Mutation proof includes a second view: reload the workbench URL, and read the file the UI wrote (`<VERIFY_ROOT>/_cases/<id>/case.json`, `<VERIFY_ROOT>/_config/settings.json`, or `<VERIFY_ROOT>/<sid>/scenario.json`). Copy that file into the evidence directory before cleanup.
- The local fixture in `test-fixtures/ci-browser` is the app under test for record and replay. Do not mock it. Do not point a proof at a product site.
- `aq.sh` and `browser.sh` are the real CLI and a real Chromium. They are not dry-runs.

Record the feature id in a one-line `note.txt` beside the artifacts. An entry point you could not reach is a failure: write the command and the unmet precondition. Do not mark it verified through a different path.

## Cleanup

```bash
.agents/skills/verify-agent-qa/scripts/cleanup.sh
.agents/skills/verify-agent-qa/scripts/cleanup.sh <run-id>
.agents/skills/verify-agent-qa/scripts/cleanup.sh --all
```

With no arguments it stops the run id in `/tmp/agent-qa-verify/current`. It closes only `VERIFY_BROWSER_SESSION` and `VERIFY_RECORD_SESSION`, kills the fixture pid in `<state>/fixture.pid` if `fixture.sh start` was used, kills the server pid from `<state>/server.pid`, kills that run's tmux session, and deletes `<state>`. It does not kill by process name and it does not delete `/tmp/agent-qa-verify-evidence`.

Run cleanup after a failed drive too, so the next launch is not sharing a port or a browser session.

## Feature map

Read `features/README.md` before choosing a feature. Mapped now: runs, record a scenario, replay, cases, settings. Chat, Editor, Sets, Plans, Goldens, Personas, Environments, Knowledge, and Extensions are in the sidebar and are not mapped yet.
