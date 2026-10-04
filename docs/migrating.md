# Migrating an existing suite to agent-qa

This doc is for teams moving a Playwright or Cypress suite onto agent-qa scenarios. Two paths: `agent-qa migrate <dir>` for a mechanical conversion, and re-recording (preferred when the app is reachable).

## What converts well

- Flow-level specs that drive a real page: navigate, click, fill, assert.
- Suites whose value is "does the flow still work," not "does the component render."

Skip: API-only tests, component tests, `page.route()`/intercept mocking, tests that assert implementation details (class names, internal state). agent-qa scenarios assert what a user perceives; implementation-detail assertions do not survive the move and usually should not.

## The mechanical path: `agent-qa migrate`

```bash
agent-qa migrate tests/ --out scenarios --dry-run   # preview, no writes
agent-qa migrate tests/ --out scenarios             # writes scenarios/<spec>/scenario.json
agent-qa scenario check-all                         # validate + lint the result
```

Each spec file becomes one scenario; unmapped lines (mocks, intercepts, driver internals) are printed per file — never silently dropped. Then replay each scenario once against the running app and mint goldens with `shot-accept` where the spec asserted visuals.

## The mechanical mapping

| Playwright / Cypress | agent-qa scenario.json |
| --- | --- |
| `page.goto(url)` / `cy.visit` | `{"kind":"do","verb":"goto","value":"<url>"}` |
| `click` | `{"kind":"do","verb":"click","on":<locator>}` |
| `fill` / `type` | `{"kind":"do","verb":"type","on":<locator>,"value":"<text>"}` |
| `selectOption` / `check` / `uncheck` | `{"kind":"do","verb":"select"}` (or `check` / `uncheck`) with `on` set to the control |
| `expect(...).toBeVisible()` | `{"kind":"check","claims":[{"subject":{"element":<locator>},"predicate":{"isVisible":true}}]}` |
| `expect(...).toHaveText` / `toContainText` | same, with `equals`/`contains` on the element's text |
| explicit waits (`waitForSelector`, `cy.wait`) | drop them — replay waits on its own |
| storageState / login fixture | a `do` login block, or a profile — see `docs/configuration.md` |

A `<locator>` is `{"role":"button","name":{"exact":"Checkout"}}` — preferred — or a raw escape hatch `{"raw":{"kind":"css"|"xpath"|"testId"|"text","value":"…"},"reason":"<why>"}`. Raw CSS carries over mechanically; `role`+`name` heals better when the UI drifts.

## The conversion prompt

`agent-qa migrate` covers the common spec shapes. For what it reports as unmapped — page objects, dynamic locators, fixtures with real setup — this prompt handles the rest. Paste into a coding agent running in the repo with the old suite. It needs `agent-qa` installed (`npm i @rasmusjosefsson/agent-qa`) and your app reachable (or a replay base URL).

```text
Convert this repo's <Playwright|Cypress> suite to agent-qa scenarios.

1. Discover: find the config (playwright.config.*, cypress.config.*) and spec
   files. Note the baseURL — scenario `goto` steps need full or relative paths.
2. For each spec, write scenarios/<name>/scenario.json following
   schema/scenario-schema.json (schema "scenario/2"): each step gets id s<N>,
   intent (one human sentence — it shows up in the run video rail), and kind.
   - page.goto / cy.visit            → do/goto with the URL in value
   - click/fill/select/check actions → do/click or do/type; keep locators,
     prefer role=+name over CSS where both exist
   - expect(...)/assertions          → check steps with claims
   - waits, retries, waitsFor*       → drop; replay waits itself
   - storageState/login fixtures     → a do/goto+type login block, or note it
3. Run `agent-qa scenario check-all` — fix everything it reports.
4. Replay each scenario once against the running app:
   `agent-qa replay <sid>` — fix or drop steps that fail; a converted
   scenario that can't replay once is not migrated.
5. Mint goldens where the spec asserted visuals:
   `agent-qa shot-accept <sid>` after a clean replay.
6. Report: converted / skipped / failed per spec file.
```

## The re-record path

When the app runs locally, re-recording a flow is often faster than converting it: `agent-qa start`, drive the flow once, `agent-qa flush`. You get richer steps (probes, network, per-step evidence) than any spec conversion produces — and the scenario is verified by construction. Convert the specs that are stable; re-record the ones that were always flaky.

## CI parity

`agent-qa init --ci` emits the replay-on-PR workflow plus the golden loop (drift comment with diffs + video, mint-on-merge). If the old suite's value was "catch regressions before merge," the emitted workflow is the whole replacement — see `docs/github-action.md`.
