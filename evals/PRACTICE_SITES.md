# Practice Site Backlog

Use these sites for future agent-qa eval suites after the QA Playground corpus is in place.

## Candidate Sites

| Site | URL | Notes |
| --- | --- | --- |
| Automation Exercise | `https://automationexercise.com/test_cases` | Large catalog of end-to-end commerce flows, auth, cart, checkout, forms, and API-adjacent scenarios. Good for multi-page journey evals. |
| UI Testing Playground | `http://www.uitestingplayground.com/home` | Focused UI traps: dynamic IDs, hidden layers, AJAX waits, client-side delays, click handling, text input, scrollbars, and shadow DOM-like gotchas. Good for locator robustness evals. |
| Practice Automation | `https://practice-automation.com/` | Broad practice site with forms, tables, popups, sliders, calendars, file upload, and waits. Good for medium-complexity UI evals. |
| LetCode Slider | `https://letcode.in/slider` | Focused slider/range-control page. Good for pointer/drag/keyboard interaction coverage once agent-qa supports those gestures well. |

## Follow-Up Suites

1. `automation-exercise`
   - Start with login/register, product search, add-to-cart, checkout, contact form.
   - Expect auth/session setup needs.

2. `ui-testing-playground`
   - Start with page-smoke cases, then dynamic ID, AJAX data, hidden layers, click, text input, scrollbars.
   - Expect locator-healing and wait semantics gaps.

3. `practice-automation`
   - Start with forms, tables, popups, calendars, file upload.
   - Expect assert and upload coverage gaps.

4. `letcode`
   - Start with slider page only.
   - Expect drag/range input support gaps.

## Rule

Keep each site in its own suite in `evals/cases.ts` or split into `evals/cases/<site>.ts` when the file grows too large.
