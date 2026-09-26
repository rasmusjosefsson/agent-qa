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
| Forms | `/practice/forms` | page-load + TC01-TC15 | complete | Golden TC01-TC15 pass. The page hosts 5 forms (F01 login, F02 personal, F03 address, F04 interests, F05 account setup); TC07's terms error has no testid so it's scoped via `#registrationForm` text. |
| Dropdowns | `/practice/dropdowns` | page-load + TC01-TC10 | complete | Golden TC01-TC10 pass; keep them green. |
| Alerts & Dialogs | `/practice/alerts-dialogs` | page-load + TC01-TC10 | complete | Golden TC01-TC10 pass. TC01-TC06 drive native alert/confirm/prompt via the bundled `evals/fixtures/dialogs.html` (the live page has none); TC07-TC09 cover DOM dialogs. |
| File Upload | `/practice/file-upload` | page-load + Upload TC01-TC02 + Upload TC06-TC15 + Download TC01-TC14 | deep | Golden Upload TC01-TC02, TC06-TC08, TC10, TC11, TC15 pass; downloads covered by `download:tc01` on the bundled fixture (the live page ships no download widget). Gaps: TC09 (no cancel control), TC12 (viewport), TC13 (native file dialog), TC14 (inputs lack accessible names). |
| Input Fields | `/practice/input-fields` | page-load + TC01-TC12 | complete | Golden TC01-TC12 pass. The `result-s02` echo only updates on real keystrokes, so fill-replayed values must be asserted on the `value` property, not the echo. |
| Buttons | `/practice/buttons` | page-load + TC01-TC15 | complete | Golden TC01-TC04, TC06-TC07, TC09, TC12-TC13, TC15 pass. `dblclick` replays TC04's double-click; TC09 uses `focus` + `press Enter`; TC12 reloads and re-reads the echo. Gaps: TC05 (right-click — no agent-browser verb), TC08 (viewport), TC10 (screen-reader), TC11 (hover visual), TC14 (design spec). |
| Data Table | `/practice/data-table` | page-load + TC01-TC06 | complete | Golden TC01-TC06 pass. `row-count` text + `book-row` testids make row assertions deterministic; search filters live via `#table-search-input`. |
| Radio & Checkbox | `/practice/radio-checkbox` | page-load + TC01-TC15 | complete | Golden TC01-TC12 + TC15 pass. Gaps: TC13 (screen-reader semantics — nothing to assert beyond `checked`), TC14 (visual state). Keyboard nav is covered via `focus` + `press` (ArrowDown moves radio selection; Space toggles checkboxes). |
| Date Picker | `/practice/date-picker` | page-load + TC01-TC05 | complete | Golden TC01-TC05 pass. `dp-constrained-input` enforces min/max and surfaces violations in `result-s05`. |
| Links | `/practice/links` | page-load + TC01-TC12 | complete | Golden TC01-TC07, TC11-TC12 pass. `tab` switches to the `_blank` tab for TC03; `focused` + `press Enter` covers TC06; broken-link TC05 asserts the `href` attribute rather than loading the external 500 page. Gaps: TC08 (accessible label), TC09 (hover visual), TC10 (right-click context menu). |
| Tabs & Windows | `/practice/tabs-windows` | page-load + TC01-TC05 | complete | Golden TC01, TC03-TC04 pass via the `tab` verb (`t2`, `close t2`, `t1` switching; claims evaluate against the active tab). Gaps: TC02 (window titles — `tab list` output is not claimable), TC05 (Ctrl+click — no modifier-click verb). |
| Multi Select | `/practice/multi-select` | page-load + TC01-TC05 | complete | Golden TC01-TC05 pass. Comma-separated `select` values cover the native multi-select (TC01-TC02); custom panels click `[role=option]:nth-child(n)` (TC03-TC04); tag chips remove via the nested `×` button (TC05, remove-only — the page has no add-tag input). |
| mDocks.dev | external | none | not started | Decide whether this belongs here or in a separate external-site suite. |

## Current Counts

- QA Playground catalog: `163` cases.
- File Upload catalog: `27` cases: `1` page-load + `12` upload + `14` download.
- Complete pages: Bank App, Dynamic Waits, Dropdowns, Alerts & Dialogs, Forms, Input Fields, Buttons, Data Table, Radio & Checkbox, Date Picker, Links, Tabs & Windows, Multi Select.
- Deep/partial pages: File Upload.
- Catalog-only pages: none.

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
- `golden:locators:tc01` proves scoped + i18n locator replay end-to-end on Multi Select.
- Golden drivers must poll for elements after a triggering click — single-shot selector lookups race post-click navigation and client-side mounts.
- File upload initially crashed Chrome with `RESULT_CODE_KILLED_BAD_MESSAGE` when replay passed ambiguous relative file paths. Canonicalizing upload file paths fixed this.
- `selectorText` is safer than broad text on docs-heavy QA Playground pages because tutorial/test-case text can create false positives.
- File Upload Upload TC01-TC02, TC06-TC08, TC10, TC11, TC15 pass with golden runners. Downloads use `do/download` + `{"file": ...}` claims on the bundled fixture — the live page exposes no download widget.
- Forms TC01-TC15 pass with golden runners. Element `attribute` claims read live IDL properties for `value`/`checked`/`disabled`/`selected`/`readOnly`/`required`, which is what reset, radio, and retain-state assertions need — `getAttribute` would only see the stale default.
- Input Fields TC01-TC12, Radio & Checkbox TC01-TC12 + TC15, Data Table TC01-TC06, Date Picker TC01-TC05 pass with golden runners via the shared `practice-lib.ts` harness.
- Two replay improvements landed from the sweep: `{"element": ..., "attribute": "focused"}` reads `document.activeElement === el` (focus assertions), and the fill self-repair now emits `input`/`change` even when the value already stuck — React-style `onInput` listeners (like the date-picker's range validation echo) depend on it.
- `pressSelector` records a real `press` step (`on` + key), replacing the old click-equivalent mapping; `focusBySelector`/`hoverBySelector`/`blurBySelector`/`clearBySelector` also translate to their verbs.
- Radio keyboard nav (ArrowDown inside a group) and Space-toggle on a focused checkbox replay natively via `focus` + `press` — no special-casing needed.
- Buttons TC01-TC04, TC06-TC07, TC09, TC12-TC13, TC15; Links TC01-TC07, TC11-TC12; Tabs & Windows TC01, TC03-TC04; Multi Select TC01-TC05 pass with golden runners — every cataloged page is now covered.
- Three replay capabilities landed from sweep II: `dblclick` (real agent-browser dblclick), `tab` (switch/close/list browser tabs; claims evaluate against the active tab), and centred `scrollTo` (scrollIntoView `block: 'center'` — default top-alignment left elements under the sticky nav and clicks were reported as covered).
- Native multi-selects take comma-separated `select` values; custom listbox panels are driven by `[role=option]:nth-child(n)` positions; page-side `selectAll`/`pre-select` buttons replay as plain clicks.
- DOM-activated clicks that open `target=_blank` tabs do NOT switch agent-browser focus — scenarios need an explicit `tab t2` step before asserting on the child tab (verified: `click` → `tab t2` → url/absence claims read the child).
- Locator `scope` chains and `name.i18nKey` resolve at replay: `scope` narrows role+name matching strictly inside each container level (no document fallback, scope-miss errors name the level), `i18nKey` reads a flat `i18n.json` beside `scenario.json`. Golden proof: `locators-tc01` clicks Select All via `{role: button, name: {i18nKey}, scope: [card testId]}`.

## Near-Term Order

1. Heal-loop v2: value-rejection classification beyond DOM probes, suggested promotions after a healed run.

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
