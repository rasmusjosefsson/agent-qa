# Landscape scan: LLM-first QA systems

Scan of comparable tools (Sept 2026) — what they do, what agent-qa already
covers, and which patterns are worth stealing. Products named here are external
references for design context only; nothing vendor-specific enters core code.

## What exists

**Self-healing runtimes**

- *robotframework-selfhealing-agents* — LLM repairs broken locators at runtime;
  emits healed files + diff reports for human review.
- *ai-driven-e2e* — LangGraph agent over Playwright; verifies a healed selector
  against the live DOM before re-running the step.
- *@ia-qa/self-healing* — closest in spirit: a local MCP server maps pages to a
  role/name/selector contract, diffs drift deterministically, and returns a
  PASS/FIX/BLOCK verdict a CI job can gate on. AI suggestions run only on what
  the deterministic diff gave up on, and always after human review.

**LLM-first authoring**

- *Spectr* — plain-English stories → Playwright/pytest; model-agnostic; failure
  clustering across runs.
- *Assrt* — crawls the app via the accessibility tree, writes *real Playwright
  TypeScript* you own and can eject; re-locates elements by intent, not CSS.
- *Rynora* — no stored scripts at all: intent is re-executed autonomously every
  run (trades determinism for zero maintenance).

**PR-driven coverage**

- *Spur* (GitHub App) — reads a PR's diff, generates change-aware browser tests,
  triggers related saved tests against the preview URL, and comments the result
  set on the PR. This is the exact loop we want.
- *self-testing-github-action* — same idea as a workflow: analyze PR → generate
  tests with an LLM → run → comment.

**Visual regression**

- *Applitools Eyes* — AI-tuned diffing (ignores anti-aliasing noise), match
  levels, and one-click baseline promotion across viewports.
- *Argos* — deliberately deterministic pixel diffs; solves flake at *capture
  time* (wait for fonts, images, network idle; hide carets) plus per-screenshot
  thresholds. The reproducibility argument holds: a black-box AI pass is hard
  to debug in a gate.

## Where agent-qa stands

| Pattern | Status |
| --- | --- |
| Deterministic replay of recorded flows | `replay` + scenario.json — owned artifacts, not generated scripts |
| Runtime self-heal | `auto_heal` + `heal.jsonl` + `heal-promote` |
| Heal debt visibility | `heal-chronic`, `heal-list`, Runs-pane trail |
| Intent-stable locators | role+name ladder, `scope`, `i18nKey` |
| Visual goldens | `{"shot"}` claims + baselines + diff maps + `shot-accept`/`--update-baselines` |
| PR evidence gate | `visual-evidence.yml` requires a media URL on UI diffs |

## Gaps worth closing

1. **PR-triggered scenario growth** (Spur's loop) — a GitHub Action / automation
   that reads the PR diff, records scenarios for the changed routes against the
   preview deployment, replays the touched suite, and comments results +
   baseline diffs on the PR. `ui-goldens.yml` (#147) is the regression half;
   the *growth* half (record on the PR's app) is the missing piece.
2. **Capture-time stabilization before shots** — Argos's lesson: kill noise
   before diffing. A shot claim could wait for `document.fonts.ready` +
   network-idle + image decode before comparing. Cheap, deterministic, and
   directly reduces tolerance hacks.
3. **PASS/FIX/BLOCK-style verdict** — `heal-chronic` flags churn; a single
   machine-readable run verdict (all-clean / auto-fixed / needs-review) would
   let CI distinguish "green because stable" from "green because self-healed
   again" without parsing jsonl.
4. **Failure clustering** — Spectr groups failures by root cause; our compare
   view already pairs runs, but grouping step failures by shared signature
   (same heal target, same HTTP 5xx) would shrink triage on big suites.
5. **Coverage-by-crawl** — Assrt's a11y crawl as a *recording bootstrap*: walk
   the app, emit candidate scenarios for the flows it finds, let the human
   prune. Turns "write scenarios" into "approve scenarios".

Non-goals learned from the field: don't adopt AI-tuned diffing (opaque in a
gate), don't abandon stored scenarios for pure intent-execution (unreviewable),
don't make heal suggestions silent writes (audit trail is the product).
