# QA Playground Coverage Plan

Goal: cover every QA Playground practice area with agent-qa evals, then promote each area from cataloged prompts to replay-proven scenarios.

## Coverage Status

| Area | Page | What It Covers | Catalog Status | Replay Status | Next Work |
| --- | --- | --- | --- | --- | --- |
| Input Fields | `/practice/input-fields` | Different input field types and states | page-load + TC01-TC12 | cataloged | Add exact prompts and prove type, append, tab, clear, disabled, readonly replay paths. |
| Buttons | `/practice/buttons` | Click, double-click, right-click, disabled buttons | page-load + TC01-TC15 | cataloged | Add exact prompts; identify double-click/right-click replay support gaps. |
| Forms | `/practice/forms` | Form fill, submit, validation scenarios | page-load + TC01-TC15 | deep | TC01-TC05 golden pass; add exact prompts and replay validation for TC06-TC15. |
| Dropdowns | `/practice/dropdowns` | Single and multi-option dropdown selections | page-load + TC01-TC10 | deep | Golden dropdown runners exist; run/verify all and update status. |
| Data Table | `/practice/data-table` | Reading, sorting, filtering table data | page-load + TC01-TC06 | cataloged | Add deterministic assertions despite dynamic table data. |
| Alerts & Dialogs | `/practice/alerts-dialogs` | Browser alerts, confirms, prompts, DOM dialogs | page-load + TC01-TC10 | deep/blocked | DOM toast/modal/dialog paths pass; native alerts/prompts need framework support. |
| Radio & Checkbox | `/practice/radio-checkbox` | Radio buttons and checkboxes in different states | page-load + TC01-TC15 | cataloged | Add exact prompts and checked/disabled assertions. |
| Date Picker | `/practice/date-picker` | Date pickers and time/date inputs | page-load + TC01-TC05 | cataloged | Add exact dates and value assertions. |
| Links | `/practice/links` | Link navigation and link metadata | page-load + TC01-TC12 | cataloged | Add exact prompts; decide new-tab and broken-link support. |
| Tabs & Windows | `/practice/tabs-windows` | Browser tabs and pop-up windows | page-load + TC01-TC05 | cataloged | Identify multi-tab/window replay support gaps. |
| Dynamic Waits | `/practice/dynamic-waits` | Explicit/implicit waits for dynamic content | page-load + TC01-TC05 | complete | Keep golden TC01-TC05 passing. |
| Multi Select | `/practice/multi-select` | Multiple item selection from lists/dropdowns | page-load + TC01-TC05 | cataloged | Add exact prompts and replay-proof select/deselect flows. |
| File Upload | `/practice/file-upload` | File upload and download scenarios | page-load + Upload TC01-TC02 + Upload TC06-TC15 + Download TC01-TC14 | deep | Upload TC01-TC02 replay pass after canonical path fix. Upload TC03-TC05 intentionally excluded because no live upload button exists. Validate remaining upload/download cases. |
| Bank App | `/bank` | End-to-end POM-style bank demo app | page-load + TC-LOGIN-01-TC-LOGIN-05 | complete | Keep golden login cases passing. |
| mDocks.dev | external | Markdown reader experience | not cataloged | not started | Decide whether this belongs in QA Playground suite or a separate external-site suite. |

## Current Counts

- QA Playground catalog: `163` cases.
- File Upload catalog: `27` cases (`1` page-load + `12` upload + `14` download). Upload TC03-TC05 are excluded because the live page has no upload submit button.
- Complete pages: Bank App, Dynamic Waits.
- Deep/partial pages: Forms, Dropdowns, Alerts & Dialogs, File Upload.
- Catalog-only pages: Input Fields, Buttons, Data Table, Radio & Checkbox, Date Picker, Links, Tabs & Windows, Multi Select.

## Near-Term Order

1. Continue File Upload validation/edge cases TC06-TC15.
2. Decide and implement download capture strategy for Download TC01-TC14.
3. Promote Dropdowns to complete by running existing golden TC01-TC10.
4. Promote Forms TC06-TC15.
5. Add exact prompts and golden proofs for Input Fields and Buttons.

## Notes

- Do not use Selenium or Playwright in eval implementations. If source TCs mention `sendKeys()` or `setInputFiles()`, treat those as intent only.
- File upload replay must use `uploadBySelector`, which flushes to `do/upload`; do not fake upload with `fillBySelector`.
- Relative upload fixture paths are canonicalized at replay using `AGENT_QA_REPO_ROOT`.
- Upload TC03-TC05 are intentionally not cataloged. Those cases require an upload submit button/progress/success flow, but the live widget exposes only `#file-upload`; the visible `Download Image/PDF/Excel/Word` buttons belong to Download test cases.
