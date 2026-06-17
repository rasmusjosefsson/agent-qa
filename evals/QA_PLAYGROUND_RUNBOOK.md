# QA Playground Eval Runbook

Use this when advancing QA Playground coverage one documented `TCxx` at a time.

Goal: each case has a deterministic golden runner that records, flushes, verifies, and replays without hand-editing generated artifacts. Model evals come after the golden path is stable.

## Inputs

- Current handoff: `evals/HANDOFF.md`
- Running log: `evals/QA_PLAYGROUND_LOG.md`
- Coverage tracker: `evals/QA_PLAYGROUND_CHECKLIST.md`
- Catalog: `evals/cases.ts`
- Golden runners: `evals/golden/`
- Result artifacts: `evals/results/` (ignored; do not commit)

## Start A Case

1. Read `evals/HANDOFF.md` and identify `Next Target`.
2. List the page cases:

```bash
cd evals
bun run run.ts --suite qaplayground --page forms --list
```

3. Run baseline checks:

```bash
cd ../cli
cargo test --locked
```

4. Copy the closest passing golden runner from the repo root:

```bash
cp evals/golden/forms-tc02.ts evals/golden/forms-tc03.ts
```

5. Add a package script if useful:

```json
"golden:forms:tc03": "bun run golden/forms-tc03.ts"
```

## Build The Golden Runner

Keep the runner deterministic and boring.

- Use short sessions: `golden-<page>-<tc>-<6 hex>`.
- Use isolated `AGENT_QA_SCENARIOS_DIR` and `AGENT_QA_RECORD_DIR` under `evals/results/<runId>/`.
- Drive the page with `agent-browser`.
- Immediately record each user-visible action with `agent-qa record-step`.
- Prefer stable selectors: `data-testid`, then `id`, then visible text.
- Prefer `record-step wait` for state assertions:
- `wait selector` for stable DOM nodes.
- `wait text` for visible validation copy.
- Use `record-step assert` only for role/name or URL assertions.
- Do not hand-edit `scenario.json` or `scenario.steps.jsonl`.

Standard runner flow:

```text
start -> open page -> record navigation -> actions + recorded steps -> checks -> flush -> verify -> replay
```

## Debug Failures

First classify the failure.

- Runner bug: direct browser action did not do what the TC needs.
- Site mismatch: documented selector/text does not exist on the live page.
- Framework gap: generated scenario is valid, but replay cannot express or execute it.
- Model gap: golden passes, but cheap-model eval cannot discover or execute it.

Inspect live page state with a disposable session:

```bash
agent-browser --session inspect-<page>-<tc> open https://qaplayground.com/practice/forms
agent-browser --session inspect-<page>-<tc> eval '(() => document.body.innerText)()'
```

Useful DOM probes:

```bash
agent-browser --session inspect-forms-tc03 eval '(() => [...document.querySelectorAll("[data-testid], [id]")].map((el) => ({ tag: el.tagName, id: el.id, testid: el.getAttribute("data-testid"), text: (el.innerText || el.textContent || "").trim() })).slice(0, 120))()'
```

```bash
agent-browser --session inspect-forms-tc03 eval '(() => document.querySelector("#userRegistrationForm")?.innerText)()'
```

If replay fails:

- Read the golden report error.
- Inspect `evals/results/<runId>/scenarios/<sid>/scenario.json`.
- Inspect replay audit under `replays/<runId>/audit.json`.
- Fix framework or runner, then re-record by rerunning the golden runner.
- Never patch generated scenario artifacts as the success path.

## Pass Criteria

A TC is golden-passing only when all are true:

- Golden runner exits `0`.
- Replay prints `SUMMARY: N/N (PASS)`.
- `verify <sid>` exits `0`.
- No generated artifacts were hand-edited.
- Any framework fix has focused unit coverage.
- Relevant broader checks pass.

Recommended verification:

```bash
cd evals
bun run golden:forms:tc03

cd ../cli
cargo test --locked
```

## After Golden Passes

Update `evals/QA_PLAYGROUND_CHECKLIST.md`:

- Mark the page `deep` if at least one TC passes.
- Mark it `passing` if the current target page's important workflows pass.
- Mark it `complete` only when every documented TC for that page passes or has an explicit framework-gap note.
- Include the latest passed TC range in notes.

Optionally run the cheap-model eval:

```bash
cd evals
bun run run.ts --case <case-id> --model github-copilot/gpt-5-mini --json --timeout-ms 600000
```

Treat model failure as a separate issue if the golden runner proves the framework path.

Append to `evals/QA_PLAYGROUND_LOG.md`:

- Result: pass, blocked, framework fixed, or model-only failure.
- Commands run.
- Artifact root and SID.
- Source changes made.
- Site or framework behavior learned.
- Follow-ups for later TCs.

## Write The Next Handoff

Update `evals/HANDOFF.md` at the end of each case.

Also end the agent's final response with a fenced copy/paste prompt for the next agent. The prompt should point at `evals/HANDOFF.md`, `evals/QA_PLAYGROUND_RUNBOOK.md`, and `evals/QA_PLAYGROUND_LOG.md`, name the next TC, and repeat the no-hand-editing rule. Keep it short enough to paste directly into a new session.

Include:

- Branch.
- Current goal.
- Links to `QA_PLAYGROUND_RUNBOOK.md` and `QA_PLAYGROUND_LOG.md`.
- Golden runners that exist.
- Catalog/list commands.
- Proven passing cases with command, `SUMMARY`, artifact root, and SID.
- Framework fixes or site mismatches discovered.
- Next target case id.
- Suggested runner path.
- Expected flow.
- Known selectors/text.
- Exact commands to run next.
- Rules: no artifact edits; fix framework/docs/eval prompt/golden runner and re-record.

Minimal next-target block:

````markdown
## Next Target

Continue with **Forms TC03**.

Catalog case id:

```text
qaplayground-forms-tc03-verify-invalid-email-format-shows-validation-error
```

Final-response copy/paste template:

```text
Continue from evals/HANDOFF.md. Use evals/QA_PLAYGROUND_RUNBOOK.md for the process and append discoveries/fixes to evals/QA_PLAYGROUND_LOG.md. Goal: get Forms TC04 to 100% golden record/verify/replay pass without hand-editing generated artifacts. Start by inspecting live TC04 behavior, then add evals/golden/forms-tc04.ts and update checklist/log/handoff when done.
```

Suggested golden runner:

```text
evals/golden/forms-tc03.ts
```

Expected flow:

1. Start agent-qa recording with short session.
2. Open the page.
3. Record navigation.
4. Drive the TC actions.
5. Record actions/checks.
6. Flush.
7. Verify.
8. Replay.
````

## Page Order

Work one page at a time. Current order:

1. Forms TC01-TC15.
2. Input Fields TC01-TC12.
3. Buttons TC01-TC15.
4. Dropdowns TC01-TC10.
5. Radio & Checkbox TC01-TC15.
6. Date Picker TC01-TC05.
7. Dynamic Waits TC01-TC05.
8. Multi Select TC01-TC05.
9. Data Table TC01-TC06.
10. Links TC01-TC12.
11. Tabs & Windows TC01-TC05.
12. Alerts & Dialogs TC01-TC10 after dialog support decisions.
13. File Upload upload/download cases after fixture and portability decisions.

Prefer moving linearly within a page. Skip only when the case exposes a real framework gap that needs a larger design decision.
