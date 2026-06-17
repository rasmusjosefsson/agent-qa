# QA Playground 100% Checklist

Goal: every QA Playground practice page has agent-qa eval coverage that records, verifies, replays, and produces stable artifacts. The catalog now has one eval entry for every documented `TCxx` plus a page-load smoke case per page; the next work is deepening generic test-case prompts into exact step-by-step workflows and making every replay pass.

## Status Legend

- `not-started`: no eval exists yet.
- `smoke`: page-load eval exists.
- `blocked`: eval found a framework gap.
- `deep`: one or more widget/test-case evals exist.
- `passing`: eval records, verifies, and replays successfully.
- `complete`: all documented test cases for that page pass.

## Current Coverage

| Area | Page | Current eval coverage | Status | Notes |
| --- | --- | --- | --- | --- |
| Bank Login | `/bank` | page-load + TC-LOGIN-01-TC-LOGIN-05 | complete | Golden TC-LOGIN-01 through TC-LOGIN-05 record, verify, and replay pass. |
| Input Fields | `/practice/input-fields` | page-load + TC01-TC12 | cataloged | Need exact step prompts and replay validation. |
| Buttons | `/practice/buttons` | page-load + TC01-TC15 | cataloged | Need exact step prompts and gesture support checks for double/right click. |
| Forms | `/practice/forms` | page-load + TC01-TC15 | deep | Golden TC01-TC05 record, verify, and replay pass. Need exact steps for TC06-TC15. |
| Dropdowns | `/practice/dropdowns` | page-load + TC01-TC10 | cataloged | Need exact step prompts and select/multi-select replay validation. |
| Data Table | `/practice/data-table` | page-load + TC01-TC06 | cataloged | Need deterministic table assertions despite dynamic faker data. |
| Alerts & Dialogs | `/practice/alerts-dialogs` | page-load + TC01-TC10 | deep/blocked | Exact TC01-TC10 prompts added. TC01-TC06 are native-dialog framework gaps until dialog actions/policy exist; golden TC07-TC09 cover DOM toast/modal/dialog paths. |
| Radio & Checkbox | `/practice/radio-checkbox` | page-load + TC01-TC15 | cataloged | Need exact step prompts and checked/disabled assertions. |
| Date Picker | `/practice/date-picker` | page-load + TC01-TC05 | cataloged | Need exact date values and value assertions. |
| Links | `/practice/links` | page-load + TC01-TC12 | cataloged | Need exact step prompts and new-tab/broken-link support decisions. |
| Tabs & Windows | `/practice/tabs-windows` | page-load + TC01-TC05 | cataloged | Need multi-tab/window support coverage. |
| Dynamic Waits | `/practice/dynamic-waits` | page-load + TC01-TC05 | complete | Golden TC01-TC05 record, verify, and replay pass with agent-qa flows. |
| Multi Select | `/practice/multi-select` | page-load + TC01-TC05 | cataloged | Need exact selecting/deselecting prompts. |
| File Upload | `/practice/file-upload` | page-load + Upload TC01-TC02 + Upload TC06-TC15 + Download TC01-TC14 | deep | Exact prompts and small upload fixtures added. Upload TC01-TC02 replay pass via uploadBySelector with canonicalized fixture paths. Upload TC03-TC05 intentionally excluded because the live widget has no upload button after file selection. Need broader upload/download replay validation. |

Current catalog size:

```bash
bun run run.ts --suite qaplayground --list | wc -l
# 163
```

## Tickets

### Ticket 1: Stabilize Eval Harness

- Add `--list` support for eval cases.
- Keep artifacts isolated under `evals/results/<runId>/`.
- Ensure every report includes case id, suite, scenario path, replay path, stdout, stderr, and error.
- Add docs for running one case, a suite, and all cases.

Acceptance:
- `bun run run.ts --suite qaplayground --list` prints every QA Playground case.
- A timed-out run writes `prompt.txt`, `stdout.txt`, `stderr.txt`, and `report.json`.

### Ticket 2: QA Playground Page-Smoke Coverage

- Add one page-load eval for each internal practice page.
- Assert the URL and at least one page-specific content marker.
- Use this as the baseline suite before deeper interaction cases.

Acceptance:
- `bun run run.ts --suite qaplayground --list` includes every internal page from `https://qaplayground.com/practice`.
- Every page has a matching `qaplayground-<slug>-page-load` case.

### Ticket 2.5: QA Playground TC Catalog Coverage

- Add one eval entry for every documented `TCxx` on every QA Playground practice page.
- Include upload and download prefixes where pages have separate test-case groups.
- Keep IDs stable and unique.

Acceptance:
- `bun run run.ts --suite qaplayground --list | wc -l` returns `163`.
- Duplicate ID check returns no rows:
  `bun run run.ts --suite qaplayground --list | cut -f1 | sort | uniq -d`.
- Forms TC01 exists with exact step-by-step instructions matching the documented data-testid flow.

### Ticket 3: Alerts & Dialogs In-Page Cases

- Keep native browser alert/confirm/prompt cases explicit as framework-gap evals until dialog support exists.
- Maintain exact prompts for TC01-TC10.
- Keep golden runners for TC07 toast, TC08 sweet alert modal, and TC09 advanced share dialog.

Acceptance:
- TC09 records, verifies, and replays pass.
- TC07 records, verifies, and replays pass.
- TC08 records, verifies, and replays pass.
- Failure artifacts identify missing framework capabilities when they do not pass.

### Ticket 4: Assertion Capability Upgrade

- Add robust assertions for present/absent by role, text, and selector.
- Add value assertions for inputs where needed.
- Document the correct recording syntax in `skill-data/core`.

Acceptance:
- QA Playground input, form, dropdown, and dialog cases can assert DOM state without hand-editing scenarios.

### Ticket 5: Input Fields Deep Coverage

- Add evals for type, append, tab, clear, disabled, readonly.
- Identify whether append needs `pressKey`/typing support improvements.

Acceptance:
- All Input Fields documented test cases pass or have framework-gap tickets.

### Ticket 6: Buttons Deep Coverage

- Add evals for click, double-click, right-click, disabled, keyboard activation.
- Identify missing gesture verbs.

Acceptance:
- Single-click passes.
- Double-click/right-click either pass or produce explicit framework-gap issues.

### Ticket 7: Forms Deep Coverage

- Add valid submit case.
- Add empty submit validation case.
- Add reset case.
- Add radio/dropdown/checkbox subcases.

Acceptance:
- Golden TC01 form success, TC02 empty-submit validation, TC03 invalid-email validity, TC04 invalid-phone validation, and TC05 password minimum-length validation replay reliably.

### Ticket 7.5: Bank Login Coverage

- Add Bank Login page catalog entries for TC-LOGIN-01 through TC-LOGIN-05.
- Add golden runners for admin login, invalid login alert, password visibility toggle, Enter submit, and viewer restricted access.
- Use selector absence for viewer-only restricted controls instead of broad role/name absence when page documentation contains the same button text.

Acceptance:
- Golden TC-LOGIN-01 through TC-LOGIN-05 record, verify, and replay reliably.
- `bun run run.ts --suite qaplayground --page bank --list` shows page-load plus all five login test cases.

### Ticket 8: Dropdowns And Multi-Select Coverage

- Add select by visible text/value/index.
- Add multi-select select/deselect cases.

Acceptance:
- Native select and multi-select cases replay reliably.

### Ticket 9: Data Table Coverage

- Add table loaded, headers present, row count, cell read, row lookup.
- Decide whether read/extract scenarios should be replay checks or inspection-mode evals.

Acceptance:
- Table scenarios prove deterministic page state without relying on unstable faker data.

### Ticket 10: Tabs, Windows, Links, Dynamic Waits

- Add link navigation and tab/window cases.
- Add dynamic wait delayed-content cases.
- Identify replay/session model gaps for multiple tabs/windows.

Acceptance:
- Same-tab navigation passes.
- New-tab/window cases have explicit support or clear framework-gap tickets.

### Ticket 11: File Upload Coverage

- Added small fixture files under `evals/fixtures/`.
- Added exact upload eval prompts for Upload TC01-TC02 and Upload TC06-TC15.
- Added exact download eval prompts for Download TC01-TC14, with explicit support-gap reporting for download capture, cross-browser, viewport, keyboard, accessibility, and network-header checks.
- Added recorder support for `uploadBySelector`; Upload TC01-TC02 replay pass after canonicalizing fixture paths.
- Upload TC03-TC05 are intentionally excluded because `/practice/file-upload` exposes only `#file-upload` plus download buttons; no upload submit button/progress/success flow exists before or after file selection.

Acceptance:
- Upload scenario records and replays without absolute machine-specific paths leaking into portable scenarios, or documents the portability gap.

## External Practice Sites Backlog

See `PRACTICE_SITES.md`.
