# agent-qa Evals

Agent-run evals for recording and replaying scenarios with cheap models.

## Saucedemo E2E

Runs the same prompt used during development against a model, then inspects the isolated eval artifact directory for a flushed scenario with a passing replay.

```bash
cd evals
bun run eval:saucedemo
```

Defaults:

```text
provider: opencode
model: github-copilot/gpt-5.1-mini
```

Override them:

```bash
bun run saucedemo.ts --model github-copilot/gpt-5.1-mini
bun run saucedemo.ts --provider opencode --model github-copilot/gpt-5.1-mini --json
```

The runner sets isolated paths for each run:

```text
AGENT_QA_SCENARIOS_DIR=evals/results/<runId>/scenarios
AGENT_QA_RECORD_DIR=evals/results/<runId>/record
```

It passes when it finds a `scenario.json` containing the Saucedemo checkout flow and at least one replay audit with `exitCode: 0` or a passing summary.
