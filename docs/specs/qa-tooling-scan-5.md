# Tooling scan 5: evidence depth, drift triage & the AI-QA shakeout (Oct 2026)

Fifth pass. Every ranked gap from scans 2–4 has now shipped (quarantine,
perf claims, totp, notify; flow DSL, includes, data rows, when; perceptual
diffs, triage plugin, flake score, resolve fan-out, resolve-heal). This
round re-surveys what the ecosystem moved on — Playwright's agent loop,
Argos's diff identity, Chrome DevTools for agents — and where the
remaining leverage is for us. Products named are external references for
design context only; nothing vendor-specific enters core.

## What moved since the last scan

**Playwright shipped its agent trio** (planner → generator → healer).
The loop is now the reference architecture for agent-authored suites —
and we've matched every stage on our own terms: planner ≈ `discover` +
`plan.md` (shipped), generator ≈ `record` + `compile`/`describe` flow DSL
(shipped), healer ≈ `auto_heal` + `heal-chronic` + the optional
resolve-plugin rung (shipped, deterministic-first). Their healer puts a
model in the loop at replay; ours keeps the run deterministic and lets
the optional resolver answer one bounded question per stuck step. Parity
on shape, different tradeoff on the loop.

**Octomind shut down** (service ended May 2026). The managed/discovery
SaaS tier is thinning — the durable demand is for owned, portable test
artifacts, which is our entire model (scenario files you keep, replay
with no service).

**Momentic / QA Wolf converge on the same admission**: coding agents
write the tests, maintenance is the cost center. Momentic puts the AI in
the heal path; QA Wolf puts humans there. Both still ship
selector-based tests that break on UI change — our `auto_heal` ladder +
audit trail targets the same cost without a model in the gate.

**Stagehand v3 caches `act()` results** (local cacheDir + server-side
cache) so identical instructions skip the LLM. That's the agentic
world's rediscovery of the frozen-scenario idea — our scenario IS the
cache, written once and replayed forever with zero model calls. Their
hit-threshold quirks (cache returns MISS below ~100 hits) are the
failure mode we avoid by freezing rather than caching.

## What exists that's still ahead of us

**Argos** — deterministic pixel diffing (odiff: multi-pass thresholds +
pixel clustering to separate noise from change — same territory as our
perceptual engine + diff regions). The interesting pieces we lack:

- **Change fingerprint**: a stable signature computed from the *shape*
  of a diff, so a recurring flaky drift is recognized as "the same
  change as before" and can be ignored per (test, fingerprint) pair —
  without hiding a new regression elsewhere in the same screenshot.
  Our diff regions already produce the geometry this would hash.
- **Ignored-change ledger**: ignored fingerprints stay listed with
  fire counts, so an ignore that outlives its flake is visible rather
  than becoming a blind spot.

**Chrome DevTools for agents** (chrome-devtools-mcp) — three things on
top of raw CDP we don't capture today:

- **Perf traces**: `performance_start_trace`/`stop` records a real
  Chrome trace (trace.json.gz) with insight analysis — the artifact a
  perf-claim miss needs for RCA. Our perf claims report numbers, not
  the trace behind them.
- **CrUX field data**: traces can pull real-user p75 percentiles from
  the Chrome UX Report API — the calibration layer that turns a lab
  budget into a defensible one.
- **Source-mapped console errors** — stack frames resolved back through
  source maps instead of minified line:col.

## Ranked gaps

1. **Drift fingerprint + known-drift ledger** — hash each diffed shot's
   region signature (we already emit regions); a repeated identical
   fingerprint is "the same drift as run N" in the report, and an opt-in
   `knownDrift` ledger can suppress a previously triaged fingerprint
   while still failing on new ones. Directly on top of #535's region
   output; the cheapest item on this list and the one that most reduces
   re-triage toil.
2. **Perf trace artifact** — CDP `Tracing` capture around perf-claim
   steps (`perf/<step>.trace.json.gz`), linked from the run report, so a
   budget miss carries the evidence needed to diagnose it.
3. **CrUX field context** — optional key'd lookup annotating perf claim
   rows with field p75 for the tested URL/origin; annotation, not a gate
   (field data only exists for public origins anyway).
4. **Source-mapped console stacks** — resolve minified frames via the
   page's source maps in the run's console artifact; pure DX.

## Non-adoptions (reaffirmed)

- **Model-in-replay healing** (Playwright healer, Momentic) — resolved
  last time and again: the optional resolve rung is the furthest the
  model gets into a replay, and only after the deterministic ladder
  fails.
- **Action-result caching instead of frozen scenarios** (Stagehand) —
  a cache mutates under you; a scenario is the artifact. Their model
  validates ours.
- **Managed test-writing services** (QA Wolf) — different product, not
  a gap.
- **Hosted review dashboards** (Argos, lost-pixel) — the workbench is
  the local answer; the diff fingerprint idea is what we take, not the
  SaaS.
