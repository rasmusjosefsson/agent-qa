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

## New-site sweeps (edge-pages-lib pattern)

For coverage against a site that isn't the QA Playground (the-internet,
saucedemo, a customer app), don't hand-write scenario JSON — use the
record-then-replay helper in `evals/golden/edge-pages-lib.ts`:

```ts
import { runEdgeGolden, edgeUrl } from "./edge-pages-lib";

await runEdgeGolden(
  "tc99", "intent text", "https://example.com/path", "#ready",
  async (p) => {
    await p.openPage();
    await p.clickSelector("#x", "click the thing");
    await p.assertElementText("#out", "done", "result shows");
  },
  // opts: { keepDialogs: true, label: "mysite" }
);
```

The runner opens a real session, executes each helper as a live browser
action *and* records it as a step (`run` + `record`), then runs
`verify` → `flush` → `replay` in a second session and writes
`golden-report.json` under `evals/results/<runId>/`. Exit code 0 = pass.

The helpers map onto the record-translate vocab — extend
`evals/golden/record-translate.ts` + add a probe to
`record-translate.test.ts` before adding a new helper, or the parity gate
fails. Helpers that drive JS (`eval`) record the nearest user action
(`clickBySelector`, `fillBySelector`, `dragBySelector`, …), never the eval.

Site quirks live in the case file, not the lib: credentialed URLs via
`edgeUrl("https://user:pass@host/…")`, dialog auto-accept via
`keepDialogs`, download asserts via `assertFile*`. New script entries go
into `evals/package.json` as `"golden:<site>:<tc>"`.

### Framework quirks worth reusing

- **AngularJS (globalsqa banking demo)**: the `select` binds a digest late —
  wait on the select itself before `selectOption`; the submit stays
  `ng-hide` until the model sets (`waitSelector('button[type=submit]:not(.ng-hide)')`
  + a ~400ms settle `waitMs`); use `clickSelectorForce` for form submits —
  a synthetic click can land before `ng-submit` attaches.
- **SPAs with in-memory state** (coffee-cart): never `open` the SPA again
  mid-scenario — navigate via in-app link clicks or the state resets.
  Watch for hidden duplicate controls (e.g. a `ul.cart-preview` shadowing
  the real row buttons) — scope selectors to the visible container.
- **`{{vars._unique}}`**: `fillSelector("...{{vars._unique}}")` records the
  template; replay mints fresh uniqueness per run (registrations, emails).
- **Below-fold clicks**: `clickSelector`/`checkSelector` auto-scroll +
  record a scrollTo step via `ensureHittable` (#321) — a recorded scenario
  never silently misses an offscreen target.
- **agent-browser trusted-click misfire at scroll depth** (CURA/
  katalon-demo-cura): `agent-browser click <sel>` reports “✓ Done” but
  delivers nothing once the page is scrolled — focus never moves, radios
  stay unchecked, submits never fire. DOM `el.click()` via `eval` works.
  Record-side workaround: `clickSelectorForce` for anything the scroll
  reached. Replay is unaffected — its css clicks dispatch the DOM
  pointer/mouse/click chain (verbs.rs `try_selector_native_click`), which
  also opens mousedown-bound widgets (bootstrap-datepicker) and commits
  their values where typing into the input leaves the widget empty —
  drive datepickers through the calendar grid, not `fillSelector`.
- **Union-merge damage**: after merge waves, diff-identical regions get
  spliced into duplicated struct fields/case arms/object methods that
  compile-fail or silently win first — sweep with `sort | uniq -d` on
  method/case names before trusting main.

### Sweep skip list (probed, not goldenable)

Don't re-probe these — verified unreachable, bot-gated, or broken:

- `nopcommerce` demos, `demo.opencart.com`, `webdriveruniversity.com` —
  Cloudflare/robot challenge.
- `uitestingplayground.com` — `ERR_CERT_COMMON_NAME_INVALID`.
- `computer-database.gatling.io`, `computer-database.herokuapp.com`,
  `olympus.realpython.org` — DNS/TLS dead.
- `seleniumbase.io/demo_page`, `magento.softwaretestingboard.com` — cert
  / origin errors.
- `buggy.justtestit.org` — reachable, but its register POST to the AWS
  API Gateway backend hangs indefinitely (XHR `loadend` never fires); all
  dynamic content (register/login/overall) is empty. The register submit
  sits below the fold — the probe was a real-site repro of the
  hit-test gap fixed by `ensureHittable` in the golden lib.
- `demo.realworld.io`, `automationintesting.com`, `openlibrary.org`,
  `demoblaze /signup`, formy `/autocomplete` (Google Places needs a key),
  formy `/switch` (404), testpages `attributes-test`, `refresh-page-test`,
  `key-click-events` (404).

## Done Criteria

- Target suite reports 100% pass.
- `evals/QA_PLAYGROUND_CHECKLIST.md` updated with pass/block status.
- Every framework gap has either a fix or a follow-up ticket/checklist item.
- No untracked non-ignored eval artifacts are committed.
- PRs that add a replay-visible capability include a recorded demo:
  `evals/capture-demo.sh --scenario <golden-run>/scenarios/<sid> --out /tmp/demo-<tc>
  --line1 "<feature>" --line2 "<what to watch for>"` produces `demo.mp4`
  (plus `demo.webp` with `--webp`)
  (two-line caption bar baked in) for embedding in the PR body or a comment.
