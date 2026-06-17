# QA Playground Evals

Canonical tracker for QA Playground coverage. Keep this file as the single source of truth for status, process, and notable findings.

## Goal

Cover every QA Playground practice area with `agent-qa` evals, then promote each area from cataloged prompts to replay-proven golden scenarios.

## How Coverage Works

- Catalog cases live in `evals/cases.ts` and drive model evals.
- Golden runners live in `evals/golden/` and prove the binary/framework path without an LLM.
- Result artifacts live in `evals/results/` and are ignored.
- Use golden runners first. Model evals come after the deterministic path is stable.

Golden runners prove:

- `agent-browser` can perform the browser action.
- `agent-qa record-step` can represent the action/check.
- `flush` produces a valid `scenario.json`.
- `verify` accepts the recording.
- `replay` executes it again and passes.

## Coverage Status

| Area | Page | Coverage | Status | Next Work |
| --- | --- | --- | --- | --- |
| Bank App | `/bank` | page-load + TC-LOGIN-01-TC-LOGIN-05 | complete | Keep golden login cases passing. |
| Dynamic Waits | `/practice/dynamic-waits` | page-load + TC01-TC05 | complete | Keep golden TC01-TC05 passing. |
| Forms | `/practice/forms` | page-load + TC01-TC15 | deep | TC01-TC05 golden pass; continue TC06-TC15. |
| Dropdowns | `/practice/dropdowns` | page-load + TC01-TC10 | deep | Golden runners exist; run/verify all and mark complete if stable. |
| Alerts & Dialogs | `/practice/alerts-dialogs` | page-load + TC01-TC10 | deep/blocked | DOM toast/modal/dialog paths pass; native alert/confirm/prompt need framework support. |
| File Upload | `/practice/file-upload` | page-load + Upload TC01-TC02 + Upload TC06-TC15 + Download TC01-TC14 | deep | Upload TC01-TC02 replay pass; continue upload validation cases and decide download strategy. |
| Input Fields | `/practice/input-fields` | page-load + TC01-TC12 | cataloged | Add exact prompts and golden proofs for type, append, tab, clear, disabled, readonly. |
| Buttons | `/practice/buttons` | page-load + TC01-TC15 | cataloged | Add exact prompts; identify double-click/right-click support gaps. |
| Data Table | `/practice/data-table` | page-load + TC01-TC06 | cataloged | Add deterministic assertions despite dynamic table data. |
| Radio & Checkbox | `/practice/radio-checkbox` | page-load + TC01-TC15 | cataloged | Add exact prompts and checked/disabled assertions. |
| Date Picker | `/practice/date-picker` | page-load + TC01-TC05 | cataloged | Add exact dates and value assertions. |
| Links | `/practice/links` | page-load + TC01-TC12 | cataloged | Add exact prompts; decide new-tab and broken-link support. |
| Tabs & Windows | `/practice/tabs-windows` | page-load + TC01-TC05 | cataloged | Identify multi-tab/window replay support gaps. |
| Multi Select | `/practice/multi-select` | page-load + TC01-TC05 | cataloged | Add exact prompts and replay-proof select/deselect flows. |
| mDocks.dev | external | none | not started | Decide whether this belongs here or in a separate external-site suite. |

## Current Counts

- QA Playground catalog: `163` cases.
- File Upload catalog: `27` cases: `1` page-load + `12` upload + `14` download.
- Complete pages: Bank App, Dynamic Waits.
- Deep/partial pages: Forms, Dropdowns, Alerts & Dialogs, File Upload.
- Catalog-only pages: Input Fields, Buttons, Data Table, Radio & Checkbox, Date Picker, Links, Tabs & Windows, Multi Select.

## File Upload Notes

- Use `uploadBySelector`; it flushes to `do/upload`.
- Do not fake upload with `fillBySelector`.
- Relative upload fixture paths are canonicalized at replay using `AGENT_QA_REPO_ROOT`.
- Fixtures live in `evals/fixtures/`.
- Upload TC03-TC05 are intentionally not cataloged. They require an upload submit button/progress/success flow, but the live widget exposes only `#file-upload`; the visible `Download Image/PDF/Excel/Word` buttons belong to Download test cases.

Passing proof:

- Upload TC01: `do/upload` + filename display replay passes.
- Upload TC02: `bun run golden:file-upload:upload:tc02` passes.

## Runbook

Use this when advancing one documented `TCxx` at a time.

1. List page cases:

```bash
cd evals
bun run run.ts --suite qaplayground --page <page> --list
```

2. Inspect live page behavior before trusting documented selectors:

```bash
agent-browser --session inspect-<page>-<tc> open https://qaplayground.com/practice/<page>
agent-browser --session inspect-<page>-<tc> eval '(() => [...document.querySelectorAll("[data-testid], [id]")].map((el) => ({ tag: el.tagName, id: el.id, testid: el.getAttribute("data-testid"), text: (el.innerText || el.textContent || "").trim() })).slice(0, 120))()'
```

3. Add or update a deterministic golden runner in `evals/golden/`.

4. Runner flow:

```text
start -> open page -> record navigation -> drive action -> record action/check -> flush -> verify -> replay
```

5. Prefer stable selectors: `data-testid`, then `id`, then visible text.

6. Prefer `record-step wait` for replayable state checks:

- `selector` for stable DOM node presence.
- `selectorText` for scoped text checks.
- `selectorAbsent` for absence checks.
- `text` only when page documentation cannot create false positives.

7. Do not hand-edit generated `scenario.json` or `scenario.steps.jsonl`.

8. After a pass, update this doc and add a package script if useful.

## Pass Criteria

A case is golden-passing only when all are true:

- Golden runner exits `0`.
- Replay prints `SUMMARY: N/N (PASS)`.
- `verify <sid>` exits `0`.
- No generated artifacts were hand-edited.
- Any framework fix has focused unit coverage.

Recommended checks:

```bash
cd evals
bun run golden:<page>:<tc>

cd ../cli
cargo test --locked
```

## Recent Findings

- Bank TC-LOGIN-01 through TC-LOGIN-05 pass with golden runners.
- Dynamic Waits TC01-TC05 pass with golden runners.
- Forms TC01-TC05 pass with golden runners.
- Alerts & Dialogs TC07-TC09 cover DOM toast/modal/dialog paths; TC01-TC06 remain native dialog framework gaps.
- File upload initially crashed Chrome with `RESULT_CODE_KILLED_BAD_MESSAGE` when replay passed ambiguous relative file paths. Canonicalizing upload file paths fixed this.
- `selectorText` is safer than broad text on docs-heavy QA Playground pages because tutorial/test-case text can create false positives.

## Near-Term Order

1. Continue File Upload upload validation cases TC06-TC15.
2. Decide and implement download capture strategy for Download TC01-TC14.
3. Promote Dropdowns to complete by running existing golden TC01-TC10.
4. Continue Forms TC06-TC15.
5. Add exact prompts and golden proofs for Input Fields and Buttons.

## Commands

```bash
cd evals

# All QA Playground catalog entries
bun run run.ts --suite qaplayground --list

# One page
bun run run.ts --suite qaplayground --page file-upload --list

# File Upload TC02 golden proof
bun run golden:file-upload:upload:tc02
```

## Retired Docs

The old `evals/QA_PLAYGROUND_CHECKLIST.md`, `evals/QA_PLAYGROUND_COVERAGE_PLAN.md`, `evals/QA_PLAYGROUND_RUNBOOK.md`, and `evals/QA_PLAYGROUND_LOG.md` are retained only as pointers to this file.
