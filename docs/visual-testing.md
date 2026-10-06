# Visual testing

agent-qa ships a pixel-diff golden framework: every do-step already captures a
screenshot during replay, so a `{"shot"}` claim compares that capture against a
checked-in baseline — no separate rendering pipeline.

## The claim

```json
{
  "subject": { "shot": "s3" },
  "predicate": "matches",
  "tolerance": { "pixels": 0.05 }
}
```

`<stepId>` names the step whose `<run>/screenshots/<stepId>.png` is compared
against `<scenario>/baselines/<stepId>.png`. `matches` is the only predicate;
`tolerance.pixels` is the allowed fraction of differing pixels (default `0.01`).

On a miss the runner writes a delta map to `<run>/shots-diff/<stepId>.diff.png`
(red pixels over a faded baseline), an RCA-lite report to
`<run>/rca/<stepId>.rca.json` (the elements intersecting the diff region — the
claim error names the top suspects), and fails the step with the diff ratio. A
missing baseline fails with the `shot-accept` hint — baselines are minted, never
implicitly trusted.

## Sensitivity

`tolerance.preset` picks a named bundle instead of numbers: `strict`
(pixels `0`, aa `16`), `balanced` (default: pixels `0.01`, aa `32`), `relaxed`
(pixels `0.05`, aa `64`). `tolerance.pixels`/`tolerance.aa` override the preset
individually. `aa` is the per-channel delta below which a pixel doesn't count —
raise it for cross-platform text AA, lower it for surgical diffs.

## Element-clipped shots

A full-page diff flags noise anywhere on screen. `clip` narrows the check to one
element's box:

```json
{
  "subject": {
    "shot": "s3",
    "clip": { "raw": { "kind": "css", "value": "#result-card" }, "reason": "card" }
  },
  "predicate": "matches"
}
```

The locator resolves live at claim time (`getBoundingClientRect`, scaled by the
device-pixel factor) and both images are cropped to that box. Pair clipped shots
with a pinned `do/viewport` so record and replay rects agree. Baselines stay
full-page; the crop applies at compare time.

## The loop

```bash
# Author: record, then inject one shot claim per do-step
agent-qa record …
agent-qa flush --auto-shots

# First run captures screenshots
agent-qa replay my-flow

# Mint baselines from that run (equivalently: replay --update-baselines)
agent-qa shot-accept my-flow
# per-step mints: agent-qa shot-accept my-flow --steps s1,s3

# Steady state — the claim diffs every replay
agent-qa replay my-flow          # fails + writes diff maps on visual drift

# Intentional change shipped → re-mint in one pass
agent-qa replay my-flow --update-baselines
```

`--update-baselines` mints even when shot claims failed — a failing diff is
exactly the re-mint case, so the flag runs before the summary exits.

## Masking volatile UI

`{"shot": "<stepId>", "mask": ["<css>", ...]}` hides volatile regions —
clocks, live badges, randomized ids — during screenshot capture. All masks in
the scenario union into one injected stylesheet (`visibility: hidden`) applied
around every step screenshot and removed after, so it survives DOM remounts.
A scenario with shot claims always performs a real `goto` — the warm-page
reuse would leave a stale document where masked elements don't exist.

```json
{ "claim": { "subject": { "shot": "s3", "mask": ["[data-qa-volatile]"] },
             "predicate": "matches" } }
```

Mark stable-but-variable markup with `data-qa-volatile` once and every golden
ignores it.

## Responsive goldens

A `do/viewport` step resizes mid-scenario, so one scenario holds desktop and
mobile goldens: `viewport 1280x800 → shot s3`, then `viewport 375x812 → shot
s5` mints two baselines in one run (`evals/selftest/scenarios/
selftest-responsive` demonstrates it).

## Before you mint

`shot-accept --dry-run` reports per candidate step `new` / `identical` /
`update <diffPct>` — the review step before re-minting. The diff itself
ignores sub-pixel antialias jitter (a pixel counts only when a channel moves
more than 32/255), the same threshold pixelmatch uses; fonts render a few
levels differently across hosts and Chromium builds and shouldn't flake
goldens.

## In the workbench

- The Runs pane renders the red diff map inline on a failed shot claim and
  offers **Re-mint baseline** (one step) or **Accept shots** (every capture in
  the run) — both hit `shot-accept` under the hood.
- The editor can append a shot claim after any do-step (camera action), and the
  Runs pane can insert checks into a saved scenario mid-review.

## In CI

`.github/workflows/ui-goldens.yml` replays the checked-in golden suite
(`evals/selftest`) on PRs that touch the UI, uploads screenshots + diff maps as
artifacts, and comments the result on the PR. Pair it with the visual-evidence
gate so UI PRs must carry before/after screenshots.

Baselines are environment-bound — a laptop's font stack differs from CI's.
Mint them on the same image that gates: `Actions → ui-goldens → Run workflow`
re-mints off main and opens a PR; on a feature PR a maintainer comments
`/qa accept` and the `qa-accept` workflow re-mints *on the PR's own head*,
pushing the new goldens to the branch so the gate goes green.

## Authoring tips

- Pin `do/viewport` early; baselines record whatever the viewport was at mint.
- Prefer clipped shots for widgets that live among dynamic chrome (nav
  indicators, timestamps).
- `flush --auto-shots` + one replay + `shot-accept` is the three-command path
  from a recording to a golden suite.

## Structural goldens (domshot)

`{"domshot": "<stepId>"}` diffs the step's ARIA snapshot (the per-step
`snapshots/` sidecar) against `baselines/<stepId>.snap.txt` instead of
pixels. It's the LLM-first variant: no font/AA flake risk, and the failure
artifact is a unified text diff under `domshots-diff/` that reads like a
code review hunk. Use `skip: ["<regex>", ...]` to drop volatile lines.
Mint with `domshot-accept <sid>` (defaults to the steps domshot claims
reference; `--dry-run`/`--steps`/`--json` mirror shot-accept).

## Geometry goldens (layout)

`{"layout": "<stepId>"}` diffs the step's element-geometry capture (the
`layouts/` sidecar — every visible element's bounding box keyed by a stable
DOM path) against `baselines/<stepId>.layout.json`. Structure-only: it flags
"moved / added / removed" elements (`tolerance.px`, default `4`; tolerable
churn via `tolerance.moved`/`added`/`removed`), not color or text drift —
that stays `shot`'s job. Mint with `layout-accept <sid>`.

## Selective replay (onlyWhen)

`"onlyWhen": ["src/checkout/**"]` on a scenario means: under
`replay --all --changed <file>` / `--changed-git <ref>`, run only when a
changed path matches a glob (`*`/`**`/`?`). Scenarios without `onlyWhen`
always run; skipped ones report as SKIP rows, not failures — the cheap way
to keep the full golden suite green-checked on every PR while only
replaying what the diff touched.

## Inspecting a diff (diff viewer)

When a golden PR comment or a workbench run shows a shot miss, open the
interactive diff viewer: a wipe slider you drag between baseline and
current, a **blink** mode that alternates the two so the changed region
flickers in place, side-by-side, the red diff map, a blend overlay, and
cursor-anchored zoom (wheel, `+`/`−`/`0`/`1`; `[`/`]` cycle modes).

- **In the PR comment**: each failed shot's details block links
  *open diff viewer* — it serves `evals/diff-viewer/viewer.html` off the
  scratch branch where the diff images are published.
- **In the workbench**: the shot-diff card toggles
  `before/after | blink | diff map` with a 50–250% zoom slider.
- **Standalone**: `viewer.html?b=<baseline.png>&c=<current.png>&d=<diff.png>`
  — no build, works from any static host or `file://`.
