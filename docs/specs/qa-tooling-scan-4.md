# Tooling scan 4: durability, healing & visual-diff engines (Oct 2026)

Fourth pass. Two questions this round: (a) should the pi chat integration
sit on `earendil-works/pi`'s durable-agent harness, and (b) what do the
visual-regression / agentic-replay tools do that we still lack — lastest,
assrt.ai, cairn, and the JUnit/flakiness tooling ecosystem. Products named
here are external references for design context only; nothing
vendor-specific enters core code.

## pi-durable evaluation — verdict: HOLD

`@earendil-works/pi-durable` (`earendil-works/pi`, `packages/durable`) is a
durable agent harness: `Harness.open(storage)` → `root()` conversation →
`submit()`/`wait()`, with an Entry/Document/Task model where tasks are
checkpointed state machines that commit atomically before anything is
shown. MemoryStorage ships; watching and subagents are built in.

Why hold rather than adopt:

- **The README flags the package Experimental — API changes without
  notice.** Pinning the workbench's chat persistence to a moving upstream
  is a recurring migration tax.
- **The gap it fills is already filled.** The workbench's pi backend keeps
  conversation state in `SessionManager` JSONL under
  `<recordDir>/agent-session` — it survives idle teardown and restarts at
  the granularity we actually need (conversation-level resume).
- **Recording state is independently durable.** Sessions, steps, runs and
  goldens live in the sidecar tree on disk; a mid-turn crash loses only
  the in-flight chat turn, which JSONL resume already covers.

Revisit when the API stabilizes or when a need it uniquely covers appears:
subagent task graphs with checkpointed mid-turn resume, or cross-session
durable task state.

## What exists

**lastest** (self-hosted visual regression platform) — three diff engines
behind one claim (pixelmatch, SSIM, Butteraugli), AI auto-classification
of drift ("expected UI change" vs "regression"), and Smart Run: git-diff →
run only affected tests. Smart Run is our `onlyWhen` — shipped.

**assrt.ai** — LLM test authoring with a self-heal path and
perceptual-hash diffs to ignore anti-aliasing noise, plus per-viewport
diff thresholds. The perceptual-hash idea is a cheaper sibling of SSIM —
a hash-level "same image?" gate before pixel work.

**cairn** — the closest external match to our architecture: an agent
discovers the flow ONCE, the result is frozen, replays forever with zero
model calls, and on drift a healer repairs **just the broken step** then
re-freezes. Their heal is model-driven at replay time.

**JUnit/flakiness tooling** (flakiness.io, Qualflare, every CI dashboard)
— a universal JUnit XML ingestion layer plus flake scoring across runs.
Our `--junit` output already feeds this ecosystem; audit.json carries the
per-run data a flake score would consume.

## What we shipped from this scan

**Resolve-plugin heal rung** (`auto_heal.rs`, `strategy: "plugin-resolve"`)
— the cairn insight, adapted to our determinism-first ordering. The
deterministic strategy ladder already heals name drift when exactly one
live candidate matches. Now, when the ladder exhausts, the OPTIONAL
`resolve` plugin gets one rung: it sees the live snapshot candidates of
the step's role and may pick the element the recorded name meant. The pick
heals exactly like a deterministic strategy — same retry-once semantics,
same `heal.jsonl` audit row, same `diffs/<stepId>.patch.json` for
`heal-promote` to consume, same `AGENT_QA_HEAL_STRICT` exit gate.

Guardrails that keep it inside our model:

- Plugin absent → behaviour identical to before (the rung is skipped).
- Plugin error → warn on stderr, treated as no-pick — a dead resolver can
  never wedge or fail a replay.
- The pick is still replayed as a concrete role+name — nothing model-side
  becomes load-bearing at replay.
- `AGENT_QA_NO_HEAL` disables it together with the rest of auto-heal.

This is the one place the "replay never calls plugins" invariant bends:
**by explicit opt-in**, on a path that already audits every correction,
and only after every deterministic strategy has failed. The
deterministic-first ordering is preserved — the plugin is the last rung,
never the first.

## Ranked gaps still open

1. **Perceptual diff tolerance** — a second shot-diff engine alongside
   pixel_diff (SSIM-lite or perceptual-hash gate) for layouts where
   subpixel shifts are noise but structure matters. lastest's three-engine
   model shows the shape; ours would be a `tolerance.engine` claim field.
2. **Drift-triage plugin kind** — an optional, non-gating plugin that
   summarizes *why* a shot drifted (the assrt/lastest auto-classify idea)
   before the verdict comment posts. Same architecture as `resolve`: a
   stdin/stdout binary, skipped when unconfigured. Not started — needs a
   plugin-kind contract + a place for the summary to live (comment? run
   report?).
3. **Flake score** — audit.json already records per-step outcomes and
   heals; a `--flaky` report across runs would rank scenarios by
   instability. Small, useful, unstarted.

Non-adoptions reaffirmed: model-in-replay healing as the *first* resort,
hosted dashboards, multi-browser matrices (agent-browser is
Chromium-only by design).
