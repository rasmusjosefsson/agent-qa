---
name: qaplayground-eval-loop
description: Coordinates agent-qa eval hardening to reach 100% pass rate across QA Playground cases. Use when working on evals under evals/, QA Playground practice pages, running cheap-model agent evals, babysitting subagents, or iterating on framework/doc gaps until replay passes.
---

# QA Playground Eval Loop

Run a supervised loop: dispatch a subagent to run one eval target, stop on first blocker, fix the smallest framework/docs/eval issue, then rerun until the target is 100% passing.

## Target Shape

Use one target at a time:

```bash
cd evals
bun run run.ts --suite qaplayground --page forms --list
bun run run.ts --suite qaplayground --page forms --json --timeout-ms 600000
```

The harness injects explicit command paths into the model prompt: one `agent-qa` binary, one `agent-browser` binary, one `AGENT_QA_EVAL_SESSION`, plus isolated scenario/record roots. If the model guesses `agent-qa --session ... record-step` or searches for `node_modules/.bin/agent-browser`, treat that as an eval prompt/harness failure.

Prefer page-level batches first (`--page forms`), then individual failing cases (`--case <id>`).

## Coordinator Loop

1. Pick the smallest target that matters: one case if debugging, one page if validating progress.
2. Start a subagent with exact target command and instruction to stop at first framework/doc/eval issue.
3. Require the subagent to report:
   - command run
   - case id
   - scenario path if any
   - replay path if any
   - exact failure output
   - likely framework/doc/eval gap
4. Inspect artifacts yourself only after the subagent reports a blocker.
5. Apply the smallest fix.
6. Run local deterministic checks.
7. Relaunch the same target.
8. Repeat until target reports 100% pass.

## Subagent Prompt Template

```text
You are working in /Users/rasmusjosefsson/Developer/agent-qa.

Goal: run this agent-qa eval target and stop at the first blocker:
<COMMAND>

Rules:
- Use the eval harness only; do not hand-run Playwright/Puppeteer/Selenium.
- If an eval case fails, stop and report. Do not fix code.
- If agent-qa records a scenario but replay fails, report scenario path, replay path, exact replay failure, and case id.
- Do not edit generated artifacts by hand: no `scenario.json`, `scenario.steps.jsonl`, replay artifact, or eval harness edits.
- If replay fails, stop and report. Do not patch generated artifacts to make the eval pass.
- If the model fails to follow instructions, report stdout/stderr and prompt path.
- If all cases pass, report the pass count and artifact root.

Return only: outcome, failing case id, command, artifacts, exact failure, and suspected gap.
```

## Fix Policy

Smallest fix wins:

- Eval prompt issue: edit `evals/cases.ts` only.
- Harness issue: edit `evals/lib/harness.ts` or `evals/run.ts` only.
- Missing agent instruction: edit `skill-data/core/SKILL.md` or a focused reference.
- Runtime replay/recording bug: edit the narrow `cli/src/**` module and add focused tests.

Do not hand-edit generated `scenario.json` or `scenario.steps.jsonl` as the solution. The eval must pass through recording and replay. If a subagent manually edits artifacts, treat that eval as failed even if replay later passes.

## Required Checks

After changes, run the relevant subset:

```bash
cd evals && bun run run.ts --suite qaplayground --page forms --list
cd cli && cargo test --locked
cd cli && cargo fmt --check
git diff --check
```

For TypeScript-only eval changes, at least run:

```bash
cd evals && bun run run.ts --help
cd evals && bun run run.ts --suite qaplayground --page forms --list
```

## Useful Artifact Summary

```bash
bun .agents/skills/qaplayground-eval-loop/scripts/summarize-reports.ts evals/results
```

## Done Criteria

- Target suite reports 100% pass.
- `evals/QA_PLAYGROUND_CHECKLIST.md` updated with pass/block status.
- Every framework gap has either a fix or a follow-up ticket/checklist item.
- No untracked non-ignored eval artifacts are committed.
