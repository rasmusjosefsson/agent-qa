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
| Dropdowns | `/practice/dropdowns` | page-load + TC01-TC10 | complete | Golden TC01-TC10 pass; keep them green. |
| Alerts & Dialogs | `/practice/alerts-dialogs` | page-load + TC01-TC10 | complete | Golden TC01-TC10 pass. TC01-TC06 drive native alert/confirm/prompt via the bundled `evals/fixtures/dialogs.html` (the live page has none); TC07-TC09 cover DOM dialogs. |
| File Upload | `/practice/file-upload` | page-load + Upload TC01-TC02 + Upload TC06-TC15 + Download TC01-TC14 | deep | Golden Upload TC01-TC02, TC06-TC08, TC10, TC11, TC15 pass; downloads covered by `download:tc01` on the bundled fixture (the live page ships no download widget). Gaps: TC09 (no cancel control), TC12 (viewport), TC13 (native file dialog), TC14 (inputs lack accessible names). |
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
- Complete pages: Bank App, Dynamic Waits, Dropdowns, Alerts & Dialogs.
- Deep/partial pages: Forms, File Upload.
- Catalog-only pages: Input Fields, Buttons, Data Table, Radio & Checkbox, Date Picker, Links, Tabs & Windows, Multi Select.

## File Upload Notes

- Use `uploadBySelector`; it flushes to `do/upload`.
- Do not fake upload with `fillBySelector`.
- Relative upload fixture paths are canonicalized at replay using `AGENT_QA_REPO_ROOT`.
- Fixtures live in `evals/fixtures/`.
- Upload TC03-TC05 are intentionally not cataloged. They require an upload submit button/progress/success flow, but the live widget exposes only `#file-upload`; the visible `Download Image/PDF/Excel/Word` buttons belong to Download test cases.
- The live page actually exposes 8 upload widgets (`fu-single-input`, `fu-multi-input`, `fu-filename-input`/`fu-filename-display`, `fu-drop-zone`, `fu-type-input`, `fu-size-input`, `fu-hidden-zone`, `fu-progress-file`+`fu-upload-btn`), each writing to a `result-sNN` holder.
- Upload TC09 is a page gap: `#fu-upload-btn` disables after select and no cancel control exists. TC12 needs a viewport verb (framework boundary). TC13 hits the native OS file dialog (framework boundary). TC14 is a page gap: no upload input has an accessible name.
- Download TC01-TC14 have no live widget to drive — download semantics are proven on `evals/fixtures/downloads.html` via `do/download` (click a download trigger, save the file into the scenario dir) and `{"file": "..."}` claims (`exists`, name predicates, `gt` size in bytes).

Passing proof:

- Upload TC01: `do/upload` + filename display replay passes.
- Upload TC02, TC06-TC08, TC10, TC11, TC15: `bun run golden:file-upload:upload:tcNN` pass.
- Download TC01-TC03 equivalents: `bun run golden:file-upload:download:tc01` passes (fixture-based).

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
start -> record setup -> open page -> record do -> drive action -> record do/check -> flush -> scenario check -> replay
```

5. Prefer stable selectors: `data-testid`, then `id`, then visible text.

6. Prefer `record-step check` for replayable state checks. Use an element claim
with a role and accessible name when possible. Use a raw locator only when the
page has no stable semantic target.

7. Do not hand-edit generated `scenario.json or recorder-state.json`.

8. After a pass, update this doc and add a package script if useful.

9. For PRs that add or change a replay-visible capability, attach a recorded
   demo so reviewers can see the behavior. `evals/capture-demo.sh` replays a
   produced scenario headed under screen capture and writes `demo.mp4` (with a
   two-line caption bar baked in) plus the raw `demo.webm`:

```bash
evals/capture-demo.sh \
  --scenario evals/results/<golden-run>/scenarios/<sid> \
  --out /tmp/demo-<tc> --session demo-<tc> \
  --line1 "<TC> — <feature> demo" \
  --line2 "<what to watch for>"
```

Embed the mp4 in the PR body or a comment (`gh pr comment/edit --attach` or the upload step in the PR tooling) — it gets a real play button. `--webp` also writes a looped `demo.webp` for places that prefer images.

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
- Dropdowns TC01-TC10 pass with golden runners.
- Alerts & Dialogs TC01-TC10 pass with golden runners. TC01-TC06 use the `dialog` verb (accept/dismiss + prompt text) and `{"dialog": true}` claim subject against the bundled native-dialog fixture; the live page ships no `window.alert`/`confirm`/`prompt`. A click that opens a native dialog cannot return from a blocking `eval`, so click dispatch treats "error + pending dialog" as the click having fired, and step sidecars are skipped while a dialog is pending.
- The automation-exercise suite passes 26/26 golden cases. Its live DOM taught two replay lessons now covered by the framework: synthetic clicks must focus the element like a real click, and string claims need Unicode-whitespace normalization.
- Golden drivers must poll for elements after a triggering click — single-shot selector lookups race post-click navigation and client-side mounts.
- File upload initially crashed Chrome with `RESULT_CODE_KILLED_BAD_MESSAGE` when replay passed ambiguous relative file paths. Canonicalizing upload file paths fixed this.
- `selectorText` is safer than broad text on docs-heavy QA Playground pages because tutorial/test-case text can create false positives.
- File Upload Upload TC01-TC02, TC06-TC08, TC10, TC11, TC15 pass with golden runners. Downloads use `do/download` + `{"file": ...}` claims on the bundled fixture — the live page exposes no download widget.

## Near-Term Order

1. Continue Forms TC06-TC15.
2. Add exact prompts and golden proofs for Input Fields and Buttons.
3. Sweep the remaining catalog-only pages (Data Table, Radio & Checkbox, Date Picker, Links, Tabs & Windows, Multi Select).

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
