---
name: compare
description: The unified diff verb. 1:1 (recording vs replay) and N-way (cross-profile / cross-replay) per-pair diff with two subsystems (ARIA snapshot + screenshot pixels).
---

# `agent-qa compare` (and the `diff` alias)

ONE verb, ONE engine, multiple shapes. `agent-qa diff` is a permanent
alias — same flags, same output. Use whichever name reads better at
the call site.

The verb runs two diff subsystems on every step in every
(baseline, candidate) pair:

1. **Snapshot diff** — unified text diff of per-step ARIA
   accessibility-tree snapshots. Catches role / accessible-name
   deltas. Filter `[ref=eN]`-only changes when reading the diff —
   they're noise from re-numbering when one element is added or
   removed.
2. **Screenshot diff** — pixelmatch overlay between per-step
   full-page screenshots. Catches anything visual the snapshot
   layer misses (icon-only buttons, layout shifts, empty states).

The two together catch the failure mode that motivated the
unification: a replay missing a permission-gated `Add user` button.
The snapshot diff surfaces the dropped role + accessible-name; the
screenshot diff confirms the visual change. One command runs both
layers automatically.

## Shapes

```bash
# 1) 1:1 — latest replay vs the recording. SID from the active recorder state if omitted.
agent-qa compare
agent-qa compare <sid>
agent-qa compare <sid> <replayId>

# 2) Cross-profile (N>=2) — bootstrap, replay (or reuse), star-diff.
agent-qa compare <sid> --profiles default-user,admin-user
agent-qa compare <sid> --profiles default-user,admin-user,viewer
agent-qa compare <sid> --profiles all

# 3) Cross-replay (N>=2) — diff existing replay folders, no bootstrap.
agent-qa compare <sid> --replays <id1>,<id2>[,<id3>,...]

# 4) List-replays helper — print every replay for a SID, copy-paste CSV.
agent-qa compare <sid> --list-replays
```

Star pattern for N-way: `profiles[0]` (after alphabetical sort, OR
`--baseline <p>` override) becomes the canonical baseline; every
other profile is a candidate diffed against it. C(N-1) pairs total.
Reading the report top-down, you see "what does each candidate
have/lack relative to the baseline."

## Flags

| Flag                                 | What                                        | Default                          |
| ------------------------------------ | ------------------------------------------- | -------------------------------- |
| `--include snapshot,screenshot`      | Subsystems to run (CSV)                     | both                             |
| `--pixel-threshold <0..1>`           | Pixel-fraction cutoff for screenshot diff   | `0` (any pixel difference fails) |
| `--baseline <profile-or-replay-id>`  | Pin the star-pattern baseline (N-way only)  | first after alphabetical sort    |
| `--force`                            | Ignore replay-reuse cache (--profiles only) | false                            |
| `--serial`                           | Explicit serial mode (--profiles only)      | serial                           |

The pixel-threshold default of 0 is intentional. Weaker defaults
silently mask permission-gated UI changes. Raise it per-invocation
if legitimate font-hinting jitter dominates a particular scenario.

## Output layout

Always under `<sid>/compare/<TS>__<labels>/`. Identical structure
for 1:1 and N-way; the only thing that changes is how many pair
subdirs land under `pairs/`.

```
<sid>/compare/<TS>__<base>-vs-<cand1>[-vs-<cand2>...]/
├── compare.md                  ← aggregated human report — open this first
├── compare.json                ← meta + per-step verdicts (machine source-of-truth)
└── pairs/
    ├── <baselineLabel>-vs-<candLabel1>/
    │   ├── summary.md          ← per-step snapshot / screenshot table
    │   ├── 000.snapshot.diff   ← unified ARIA diff (only when changed)
    │   ├── 000.screenshot.diff.png  ← pixelmatch overlay (only when changed)
    │   ├── 000.baseline.png    ← copy of baseline shot
    │   ├── 000.candidate.png   ← copy of candidate shot
    │   ├── 000.compare.png     ← Percy-style triptych baseline|candidate|overlay
    │   └── ...
    └── <baselineLabel>-vs-<candLabel2>/   (only when N>=3)
        └── ...
```

`compare.md` is the single artefact a human opens to triage. The
per-pair `summary.md` files give the per-step verdict table for one
pair.

## Verdict semantics (per-pair `summary.md` cells)

- `match` — included subsystem ran, no delta detected.
- `changed` — included subsystem ran, delta detected. Counts toward exit non-zero.
- `missing` — sidecar absent on one or both sides. Does NOT count as a failure
  (older recordings without per-step screenshots are still valid; absence ≠ regression).
- `error` — diff threw (e.g. screenshot dimension mismatch). Counts toward exit non-zero.
- `skipped` — subsystem excluded via `--include`. Renders as `—`.

## Exit codes

- `0` — every included subsystem on every step in every pair is `match` /
  `missing` / `skipped`.
- `1` — at least one cell is `changed` / `error`, OR a non-recoverable
  upstream condition (missing scenario, missing replay folder, replay
  halted before completion).
- `2` — bad CLI args (unknown flag, out-of-range `--pixel-threshold`,
  fewer than 2 ids in N-way mode, `--baseline` referencing an unknown side).

## Reading the diff — gotchas

A few rakes that bit us before:

1. **ARIA snapshot diffs have ref-renumber noise.** When one element
   is removed, every following `ref=eN` shifts and produces fake +/-
   pairs. Filter ref-only changes; the real signal is added/removed
   role + accessible-name pairs.
2. **An empty snapshot diff does not always mean "no UI difference".**
   A visual-only change (icon swap, colour, layout) shows up in the
   screenshot diff but not the snapshot. Always read both subsystem
   cells in `summary.md` before declaring a step identical.
3. **The baseline you pick changes the report shape.** With star-pattern,
   `presentIn` / `absentIn` are read relative to the baseline. Prefer
   `--baseline <primary-or-similar>` so deltas read as "what default-user
   is missing" instead of the alphabetically-first profile being
   the implicit reference.

## Replay-reuse policy (--profiles mode)

Per profile, skip a fresh replay iff:

- the existing `replay.json.meta.scenarioContentHash` equals
  `sha256(scenario.steps[])` of the current `scenario.json` (the spec
  hasn't changed), AND
- replay `status` is `'ok'` (gated/error replays are not valid
  executions of the spec — re-replay anyway), AND
- `--force` was NOT passed.

Otherwise: re-replay. The hash-based check means cosmetic edits to
`scenario.json` (changing `meta.instruction`, comments, anything
outside `steps[]`) do NOT invalidate existing replays — only changes
that affect what gets replayed do.

## When this verb does NOT apply

- **Single-step inspection** of one keyframe. That's `agent-qa list`
  + `agent-browser snapshot`, not this verb.
- **Ad-hoc diff of two arbitrary snapshot/screenshot sidecars** outside
  the scenario lifecycle. Use a manual `diff` against the sidecar paths,
  or invoke the per-pair engine directly in `cli/src/compare/mod.rs`.
