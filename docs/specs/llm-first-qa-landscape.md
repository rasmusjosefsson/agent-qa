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
6. **Migration playbooks** — TesterArmy ships per-framework `/migrate` pages
   whose core is a copy-paste prompt a coding agent executes (discover specs →
   map → verify → report). Mechanical mapping lands in
   `docs/migrating.md`; the equivalent `agent-qa migrate <dir>` importer is
   the follow-up.
7. **MCP surface** — TesterArmy ships `e2e mcp`, a stdio server exposing its
   CLI to agent clients (record/replay/lint as MCP tools). agent-qa's skills
   already put the verbs in context; a thin `agent-qa mcp` wrapper would put
   them in scope for MCP-native agents that never read a skill file.
8. **Verification channels** — TesterArmy provisions temp mail inboxes so the
   agent can read OTP codes and verification links during a run, plus HTTP
   basic-auth and stored credentials. Our profile plugins cover login state,
   but nothing reads an inbox: a `mail` helper (spin up or point at a
   mailpit/temp-mail endpoint, poll for message, extract link/code) would
   unblock signup/verify flows end-to-end.
9. **Discovery runs** — TesterArmy's "give the agent a goal, no saved steps,
   report every bug" and its PR exploration agent (read the diff → write a
   plan → execute in a real browser). Gap #1 covers the PR-triggered half;
   goal-driven exploration is the bigger half — `crawl` emits a coverage
   scenario today, but nothing hunts for defects autonomously.
10. **Agentic steps that lower to deterministic replay** — e2e's twist: an
    `agent.act("upgrade the workspace")` step records the actions the agent
    took, and later runs replay them with zero model calls until the app
    changes. That's our record/replay loop read backwards — interesting as a
    model-assisted *heal* fallback (a step whose locator can't resolve asks a
    model once, records the new action, replays deterministically after),
    not as an authoring surface.
11. **Agent-discovery surface on the site** — llms.txt, `.md` mirrors of every
    docs page, and a `.well-known/agent.json` capability manifest. Cheap; our
    site already renders docs but nothing advertises them to agents.

### E2E visual-testing tools (Percy, Applitools, Argos, Chromatic)

Same pass over the dedicated visual tools — what each does well and whether a
deterministic version fits here:

12. **Capture stabilization pack (Argos)** — the strongest borrowable: before a
    screenshot, freeze animated GIFs at frame 0, pause CSS animations, wait for
    `[aria-busy]` to clear, wait for CSS background images, and disable font
    hinting/subpixel rendering for cross-OS consistency. One JS injection at
    capture time kills the dominant flake class (animations mid-frame, lazy
    images, loading spinners). Cheap and orthogonal to everything else.
13. **Ignore regions / `data-visual-test` masking (Argos, Percy, Applitools)** —
    paint over dynamic elements (dates, ads, carousels) before capture on both
    sides so they can never diff. Ours would be a `mask: [locators]` option on
    shot claims — deterministic, no AI ignore needed.
14. **Diff RCA-lite (Applitools)** — SHIPPED: on a shot miss the runner maps
    the diff map's red-pixel bounds back to CSS space and lists the elements
    intersecting it — `<run>/rca/<stepId>.rca.json` plus the top suspects in
    the claim error ("suspects: body>div#card>button"). A miss now says what
    moved, not just that red pixels exist.
15. **Floating regions + layout-level matching (Applitools)** — SHIPPED as
    `{"layout": "<stepId>"}` claims: a `layouts/` sidecar captures every
    visible element's bounding box keyed by a stable DOM path; the claim diffs
    it against `baselines/<stepId>.layout.json` with `tolerance.px` (default 4)
    plus `moved`/`added`/`removed` churn budgets — the floating-region budget
    as counts. Mint with `layout-accept`.
16. **Flake score per scenario (Argos)** — flaky badges, stability %, history
    per test from replay history. Our `audit` already keeps runs; a stability
    column in `audit stats` / run-report is a small rollup over data we own.
17. **Change-scoped re-snapshotting (Chromatic TurboSnap)** — SHIPPED as
    `onlyWhen` + `replay --all --changed <file>` / `--changed-git <ref>`:
    a scenario whose globs match no changed path skips (SKIP row in
    `--report`, not a failure); scenarios without `onlyWhen` always run.
    Config-driven, no dep graph.
18. **Per-snapshot diff sensitivity presets (Percy)** — SHIPPED as
    `tolerance.preset`: `strict` (pixels 0 / aa 16), `balanced` (0.01 / 32 —
    the default), `relaxed` (0.05 / 64). `tolerance.pixels`/`tolerance.aa`
    override the preset individually.

Non-goals learned from the field: don't adopt AI-tuned diffing (opaque in a
gate), don't abandon stored scenarios for pure intent-execution (unreviewable),
don't make heal suggestions silent writes (audit trail is the product), don't
buy a hosted-dashboard dependency for diff review (the PR comment IS our
dashboard — keep it that way).
