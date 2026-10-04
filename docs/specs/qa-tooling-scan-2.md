# Tooling scan 2: AI-native, monitoring, and signal-claim QA (Oct 2026)

Second pass beyond the first landscape scan (`llm-first-qa-landscape.md`,
which covered tester.army, self-healing runtimes, LLM-first authoring,
PR-driven coverage, and the visual-regression tools). This pass surveyed the
AI-native agents and the quality-signal corners of the market. Products named
here are external references for design context only; nothing vendor-specific
enters core code.

## What exists

**AI-native agents**

- *Midscene.js* — vision-first agent SDK (web/Android/iOS/desktop). Acts and
  asserts on screenshots, no selectors; YAML test files plus atomic
  `aiTap`/`aiAssert` APIs; caches plans for replay speed; ships "skills" that
  let coding agents drive it.
- *Stagehand* (Browserbase) — agent SDK over Playwright: `act`/`extract`/
  `observe` primitives, hybrid accessibility-tree trimming (fewer tokens),
  self-healing actions, step cache for deterministic fast replay, session
  inspector for every AI decision.
- *Octomind* — hosted gen+run for Playwright suites. The interesting part is
  **source-level healing**: a heal is a pending proposal you `pull` into the
  repo — never a silent source edit. "Zero silent commits" policy.
- *Momentic* — layered failure response worth stealing wholesale:
  1. locator auto-heal (in-run, no source edit),
  2. transient recovery (auto-clear cookie banners/modals, retry the step),
  3. permanent healing (classify, propose, apply),
  4. **quarantine** (keep the flaky test running and collecting data without
     letting it block CI).
- *QA Wolf* — hosted gen/run/report; human-verified suites. Mostly parity.

**Monitoring as code**

- *Checkly* — turns existing Playwright specs into scheduled synthetic
  monitors selectable by project/tag, run from 20+ regions; monitoring defined
  as code (TS/Terraform/Pulumi); alerting + status pages + AI RCA. The
  takeaway isn't hosted monitoring — it's that the same artifact (the test)
  doubles as the production probe, and that *a run needs an alert sink*.

**Quality-signal claims**

- *axe-core / pa11y / Lighthouse CI* — a11y as a gateable claim: per-rule
  violations with severity, per-URL minimum scores via `assertMatrix`
  (route-matched thresholds). Lighthouse also does perf budgets
  (LCP/CLS/TBT/fcp) with the same per-route matrix.

**API-layer**

- *Schemathesis* — property-based API testing from OpenAPI/GraphQL: generates
  cases from schema constraints, chains operations statefully, learns IDs
  mid-run, every failure ships a curl reproducer. Browser-layer adjacent —
  our `callGqlApi` + `saveAs` covers *authored* chains; schema-driven
  generation is out of scope.

**Auth in tests**

- *Testim/mabl/Testrigor* — TOTP as a test primitive (register the seed,
  the runner emits the 6-digit code). Complements email-link flows.

## Where agent-qa already stands

| Pattern | Status |
| --- | --- |
| Source-level healing, pull-model | `heal-promote`, `heal-chronic --apply` — Octomind's pull proposal, owned |
| In-run locator heal | `auto_heal` + `heal.jsonl` |
| AI-decision audit trail | `events.jsonl`, run artifacts, report server — Stagehand's session inspector, owned |
| Deterministic replay of authored artifacts | core model — what Stagehand's step cache approximates |
| PR evidence + drift comments | golden loop + drift publish, owned |
| Email verification channel | `{mail}` verb + `agent-qa mail` — just shipped |
| Agent-discovery surface | `/llms.txt`, `.md` mirrors, `agent.json`, `agent-qa mcp` — just shipped |

## Gaps worth closing (ranked)

1. **Quarantine mode** (Momentic layer 4) — `quarantine: true` on a scenario
   or claim: the step runs, failures are recorded and reported, but the run
   verdict doesn't fail the gate. `heal-chronic` already surfaces repeat
   healers; quarantine is the containment half — run and measure without
   blocking. Small, high-value, directly serves flake tracking.

2. **Transient recovery** (Momentic layer 2) — deterministic version: a
   `presteps`/`dismiss` mechanism that tries a list of overlay-closers
   (cookie banners, modals, interstitials) before a step retries, rather
   than AI-detecting obstructions. Could be `params.dismiss: [<locator>...]`
   on a step or a scenario-level `beforeEachStep` hook.

3. **a11y claim** — `{a11y: {maxViolations: 0, severity: "serious"}}` check
   predicate: inject axe-core via `eval`, report per-rule violations as
   claim details. Fully deterministic, gateable; the per-route threshold
   matrix maps naturally onto per-scenario params.

4. **perf budget claim** — `{perf: {metric: "lcp"|"cls"|"tbt"|"fcp",
   maxMs: 2500}}` via the Performance API; Lighthouse's assertMatrix = our
   per-scenario params. Cheap, deterministic, complements shot claims.

5. **totp verb** — `{totp: {secret: "{{secrets.X}}"}}` → current 6-digit
   code bound via `saveAs`. HMAC-SHA1/30s window is ~40 lines; rounds out
   the `{mail}` channel for 2FA flows.

6. **Alert sink** (Checkly's lesson) — runs on a schedule already work
   (cron/CI); the missing piece is `agent-qa.toml [notify]` — a webhook/
   Slack sink `run-report` posts verdicts to on failure. Replay-as-monitor
   without a hosted service.

## Explicit non-adoptions

- **Vision-based assertions** (Midscene's screen-only assert) — model in the
  gate, same non-goal as AI-tuned diffing.
- **Autonomous goal-driven replay** — already documented as the open half of
  landscape item 9; discover's deterministic slice stands.
- **Cross-platform drivers** (Android/desktop/RDP) — out of scope for a
  browser-layer tool.
- **Hosted dashboards / status pages** — report server + PR comments are
  enough; a hosted SaaS is a different product.
