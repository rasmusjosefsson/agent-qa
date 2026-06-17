# agent-qa Evals

Agent-run evals for recording and replaying scenarios with cheap models.

## Usage

```bash
cd evals

# Run all eval cases
bun run eval

# Run just Saucedemo
bun run eval:saucedemo

# Run QA Playground cases
bun run eval:qaplayground

# Run one QA Playground page
bun run run.ts --suite qaplayground --page forms

# Run deterministic framework proof for Forms TC01
bun run golden:forms:tc01

# Run deterministic framework proof for Forms TC02
bun run golden:forms:tc02

# Run one case
bun run run.ts --case qaplayground-alerts-dialogs-tc09-advanced-dialog-close

# List cases without running models
bun run run.ts --suite qaplayground --list

# JSON output
bun run run.ts --suite qaplayground --json
```

## Saucedemo E2E

Runs the same prompt used during development against a model, then inspects the isolated eval artifact directory for a flushed scenario with a passing replay.

```bash
cd evals
bun run eval:saucedemo
```

## QA Playground

Starts with `https://qaplayground.com/practice/alerts-dialogs` cases that avoid native browser alerts:

- `qaplayground-alerts-dialogs-tc09-advanced-dialog-close`
- `qaplayground-alerts-dialogs-tc07-toast-visible`

Also includes page-load smoke evals for every internal QA Playground practice page from `https://qaplayground.com/practice`:

- `qaplayground-input-fields-page-load`
- `qaplayground-buttons-page-load`
- `qaplayground-forms-page-load`
- `qaplayground-dropdowns-page-load`
- `qaplayground-data-table-page-load`
- `qaplayground-alerts-dialogs-page-load`
- `qaplayground-radio-checkbox-page-load`
- `qaplayground-date-picker-page-load`
- `qaplayground-links-page-load`
- `qaplayground-tabs-windows-page-load`
- `qaplayground-dynamic-waits-page-load`
- `qaplayground-multi-select-page-load`
- `qaplayground-file-upload-page-load`

Native alert/confirm/prompt cases should be added after agent-qa has explicit dialog recording/replay support.

Use `QA_PLAYGROUND_RUNBOOK.md` when advancing the catalog one `TCxx` at a time and updating `HANDOFF.md` for the next agent. Use `QA_PLAYGROUND_LOG.md` as the running history of fixes, discoveries, artifacts, and follow-ups.

See `PRACTICE_SITES.md` for external practice sites queued for later suites.

Defaults:

```text
provider: opencode
model: github-copilot/gpt-5-mini
```

Override them:

```bash
bun run saucedemo.ts --model github-copilot/gpt-5-mini
bun run run.ts --provider opencode --model github-copilot/gpt-5-mini --json
```

The runner sets isolated paths for each run:

```text
AGENT_QA_SCENARIOS_DIR=evals/results/<runId>/scenarios
AGENT_QA_RECORD_DIR=evals/results/<runId>/record
AGENT_QA_EVAL_SESSION=eval-<caseId>
```

It passes when it finds a `scenario.json` containing the Saucedemo checkout flow and at least one replay audit with `exitCode: 0` or a passing summary.

The prompt gives the agent explicit binaries:

```text
agent-qa: ./cli/target/debug/agent-qa when built, otherwise agent-qa
agent-browser: agent-browser, or AGENT_QA_EVAL_AGENT_BROWSER_BIN when set
```
