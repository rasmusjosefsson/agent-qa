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

## Gaps worth closing (ranked) — status

1. **Quarantine mode** (Momentic layer 4) — **SHIPPED**: opt-in
   `quarantine` on the scenario (`true` or `{reason, until}`). The run
   executes and records failures honestly (`SUMMARY: FAIL — quarantined`,
   `QUAR-FAIL` report rows, `audit.quarantined`) but exits 0. Deliberately
   **not** auto-assigned: `reason` is linted when absent
   (`quarantine-without-reason` warning) and `until` expires the
   containment (`quarantine-expired` error + the flag stops silencing) so
   it can't become a dumping ground.

2. **Transient recovery** (Momentic layer 2) — **SHIPPED** (already): the
   `dismiss` verb + per-run dismissal list — registered closers are
   re-applied deterministically before every interactive step, which is
   the `beforeEachStep` hook this item described.

3. **a11y claim** — **SHIPPED** (already): `{a11y: {impact, rule, within,
   incomplete}}` runs axe-core via `agent-browser a11y` and counts
   matching violations; numeric predicates gate the count.

4. **perf budget claim** — **SHIPPED**: `{perf: "lcp"|"cls"|"tbt"|"fcp"|
   "ttfb"|"load"}` reads the metric via Performance API + buffered
   observers (`{metric, maxDwellMs}` for the dwell). Numeric predicates
   compare the value (ms; cls is the score); `exists`/`notExists` test
   presence (lcp before paint is absent; cls/tbt report 0).

5. **totp verb** — **SHIPPED**: `{totp, params:{secret, digits, period,
   algorithm, offset}}` computes the current code locally (HMAC-SHA1/
   SHA256, RFC 4226) and binds `{code, period, remaining}` via `saveAs`;
   `agent-qa totp <seed>` probes it from the CLI.

6. **Alert sink** (Checkly's lesson) — **SHIPPED**: `[notify] url + on` in
   `agent-qa.toml`; the runner POSTs the verdict payload after every run
   (Slack-compatible `text` + `sid`/`verdict`/`summary`/`runDir` fields).
   Best-effort — a failed POST warns, never changes the exit code.
   `agent-qa notify test` probes the wiring.

## Explicit non-adoptions

- **Vision-based assertions** (Midscene's screen-only assert) — model in the
  gate, same non-goal as AI-tuned diffing.
- **Autonomous goal-driven replay** — already documented as the open half of
  landscape item 9; discover's deterministic slice stands.
- **Cross-platform drivers** (Android/desktop/RDP) — out of scope for a
  browser-layer tool.
- **Hosted dashboards / status pages** — report server + PR comments are
  enough; a hosted SaaS is a different product.
