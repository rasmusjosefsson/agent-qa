# Tooling scan 6: agent-agnostic authoring, hermetic replay & the loop glue (Oct 2026)

Sixth pass. Scan-5's shipped items are live (drift fingerprint +
known-drift ledger, perf trace artifact); its remaining two (CrUX field
context, source-mapped console stacks) were explicitly declined. This
round re-surveys the ecosystem's newest moves and audits our own
surface for what's still missing now that the obvious gaps are closed.
Products named are external references for design context only; nothing
vendor-specific enters core.

## What moved since the last scan

**Playwright agent definitions are now per-loop** — `init-agents
--loop= vscode | claude | codex | opencode` generates agent-tailored
definitions, and VS Code 1.105+ is required for the agentic experience.
The planner/generator/healer trio is deliberately host-agnostic: same
artifact files, any agent harness. That is exactly our model — SKILL.md
+ `[skills] extra-dirs` is the equivalent surface, and our recorded
artifact (scenario.json) is portable across every harness with zero
regeneration. Parity confirmed, not a gap.

**Stagehand v4 went CDP-native** — the SDK dropped Playwright as its
runtime and now drives a CDP browser directly, keeping `act`/`extract`/
`observe` as the only model-touching calls. The biggest AI-automation
framework independently arrived at our architecture: deterministic CDP
core, model only where the deterministic path can't answer. Their
`act()` result cache is still the frozen-scenario idea with a hit-rate
threshold; our scenario file remains the stronger primitive (write once,
replay forever, zero model calls).

**Browser Use stays fully agentic** (0.13.x) — goal-in, loop-until-done.
ego/lite and similar "give the agent a real browser" tools are the other
pole. Both validate the same split we sit in: agents author, artifacts
replay. Neither threatens the deterministic-replay niche — they produce
sessions, not regression suites.

**Managed QA keeps thinning** — Octomind's shutdown (from scan-5)
stands; Momentic/QA Wolf still sell maintenance-as-service on
selector-based suites. Nothing new to adopt there.

## What exists that's still ahead of us

Audited against our current surface — a11y (axe) claims, geolocation /
locale / focus emulation, `--freeze` clock+RNG, `mock`/`unmock` +
`--offline` strict mode, `netlog`, junit, `--jobs N` parallel replay,
perceptual diffs + regions + fingerprints, resolve/triage plugin kinds.

The remaining leverage is not new primitives — it's closing three
half-open loops:

## Ranked gaps

1. **Self-authoring: plan → scenario without a coding agent** —
   `discover` already emits `plan.md`; today turning it into scenarios
   requires pi/Claude/opencode driving the skill. An `author` path
   (e.g. `agent-qa author --plan plan.md`) that drives each planned
   flow via the smart-* verbs with the resolve plugin answering
   element ambiguities would complete the "generator" stage
   deterministically — the only model in the loop is the bounded
   decision call, no coding-agent dependency. Biggest item on the
   list; also the one that makes the tool self-contained for teams
   without an agent harness.

2. **Hermetic replay (network VCR)** — `mock`/`unmock` are
   hand-authored steps; there's no record→serve path. A
   `mock --record` mode that captures request signatures + response
   bodies into the sidecar at mint time, and serves them on later
   replays (`--offline` already provides the strict shell), would
   eliminate the entire third-party/live-site flake class — the same
   class as the csi.gstatic beacon and live-site tint drift we've hit
   in evals. Determinism taken to its logical end; large but very
   on-brand.

3. **Flake-score → auto-quarantine loop** — quarantine is opt-in and
   flake score is computed but inert. `tolerance.quarantineAt: <0-1>`
   auto-marks a scenario quarantined (warn + keep running, report
   flags it) once its rolling flake score crosses the threshold —
   maintenance signal becomes self-acting. Small glue.

4. **Domshot diff in the interactive viewer** — diff-viewer.html
   handles PNG triples only; domshot drift still compares raw text.
   A `?dom=` mode rendering recorded-vs-current domshot as a
   side-by-side line-highlighted diff reuses the whole viewer/link
   pipeline for the second artifact type. Small UX item.

5. **Check-run annotations over comments** — emitted CI posts drift as
   PR comments; a `qa gate --checks` mode writing a GitHub Check Run
   with file/line annotations on the failing scenario.json steps puts
   failures in the diff view where reviewers look. Medium.

6. **Coverage ledger** — `crawl`/`discover` already map the route
   graph; a `coverage` report (discovered routes × scenario-touched
   routes, emit `coverage.md`/json + an optional claim "no uncovered
   nav paths") turns discovery into a gate. Medium.

## Non-adoptions (reaffirmed)

- **Model-in-replay healing** (Playwright healer, Momentic) — resolved
  plugin rung covers it deterministically.
- **CrUX field context, source-mapped console stacks** — scan-5 items,
  declined.
- **Full-agentic loop as a runtime mode** (Browser Use) — that's a
  different product; our agents author, never drive replay.
- **Managed services/hosted dashboards** — unchanged.
