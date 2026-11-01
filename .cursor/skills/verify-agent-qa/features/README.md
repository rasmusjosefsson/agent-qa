# agent-qa verification map

This directory is the maintained source for verifying the user-facing behavior of agent-qa. Read the index before driving the app, then use the matching feature file as the recipe.

## Baseline preconditions

- Launch an isolated workbench with `.cursor/skills/verify-agent-qa/scripts/launch.sh`. It prints `VERIFY_URL` (port 7879 or the next free port up to 7900, never 7878).
- The scenarios root is `$VERIFY_ROOT` under `/tmp/agent-qa-verify/<run-id>/scenarios`, started empty. The record dir is `$VERIFY_RECORD`.
- Run `.cursor/skills/verify-agent-qa/scripts/doctor.sh` and require `ok <url>` plus the scenarios root from this run.
- Drive the workbench only through `scripts/browser.sh` (session `aqa-workbench-<run-id>`). Drive the CLI only through `scripts/aq.sh`.
- Never drive port 7878, the repo `tmp/agent-qa-scenarios` directory, or a workbench this run did not start.

## Driving conventions

- Start every recipe from the baseline state unless its preconditions say otherwise.
- Prefer ARIA roles and accessible names over CSS selectors or DOM position. The cases dialog waits on `#case-title` because that input's accessible name exists only after it opens.
- Treat every command as literal. Keep quoted names and flags unchanged.
- Run workbench actions through `scripts/browser.sh`.
- Run record, flush, replay, and other CLI actions through `scripts/aq.sh`.
- Put proof under `$VERIFY_EVIDENCE/<feature>/`. Cleanup deletes the scenarios root and must not be the only copy of a written file.

## Proof and skip reporting

- Capture the user action and the resulting state, not only the final screen.
- UI proof includes an accessibility snapshot (`browser.sh snapshot`, not `-i` when the proof is non-interactive text) and a screenshot with the `agent-qa` sidebar name visible.
- CLI proof includes the command, stdout, stderr, and exit code.
- Mutation proof includes the file under `$VERIFY_ROOT` and the same fact read back from the workbench after a fresh `open` of its URL.
- Record the feature ID and entry point used with every artifact (`note.txt`).
- Report an unreachable path with the attempted command and the unmet precondition.
- Do not report a skipped entry point as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing the user-visible behavior. It then uses exactly four H2 sections in this order.

1. `Sub-features` lists short IDs with one line for each behavior.
2. `How to get to it (user POV)` lists every user entry point.
3. `Driving it with verify-agent-qa` starts with `Preconditions:` and uses labeled bullets that pair each user action with an exact command and observable result.
4. `Gotchas` lists traps that can waste or invalidate a verification run.

## Features

- [Runs](./runs.md) covers the empty workbench home and opening a scenario that record already flushed.
- [Record a scenario](./record-scenario.md) covers recording the local browser-contract fixture from the CLI and seeing it in Runs.
- [Replay](./replay.md) covers replaying that scenario from the Runs pane.
- [Cases](./cases.md) covers creating a test case from the sidebar and from `/cases`, and reading it back from disk.
- [Settings](./settings.md) covers the scenarios-root display and the headed-browser default.

Chat, Editor, Sets, Plans, Goldens, Personas, Environments, Knowledge, and Extensions are sidebar destinations and are not mapped yet.
