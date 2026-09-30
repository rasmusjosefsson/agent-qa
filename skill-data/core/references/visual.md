# Visual checks (shot claims)

A `{"shot": "<stepId>"}` claim pixel-diffs the run's screenshot of a step
against a committed baseline PNG. This is the UI-regression loop: baseline
goldens live in git, replay compares, drift produces a red delta map.

## Claim shape

```json
{
  "id": "s3",
  "intent": "the dashboard still looks right",
  "kind": "check",
  "claim": {
    "subject": { "shot": "s2" },
    "predicate": "matches",
    "tolerance": { "pixels": 0.01 }
  }
}
```

- `subject.shot` is the id of the do-step whose screenshot to compare —
  every do-step already writes `screenshots/<id>.png` per replay.
- `tolerance.pixels` is the allowed fraction of differing pixels
  (default `0.01` = 1%). A color-block appearing in the nav still trips it.
- Optionally `clip: {"on": <locator>}` crops the diff to one element's box —
  use it to ignore noisy chrome around the widget you care about.

## Baselines

Live at `<scenario>/baselines/<stepId>.png` and are committed with the
scenario. Mint or re-mint them with:

```bash
agent-qa shot-accept <sid> --dry-run       # preview: new / identical / update <pct>
agent-qa shot-accept <sid>                 # latest run, all shot claims
agent-qa shot-accept <sid> --steps s3      # one claim only
agent-qa replay <sid> --update-baselines   # run, then accept its shots
```

`--dry-run` is the review step before applying new goldens: it reports what
each baseline would become without writing. Accept also warns when the
scenario file changed since the run you're minting from (hash mismatch —
the captures may describe an older scenario).

**Baselines are environment-bound.** Fonts and rasterization differ between
your laptop and CI runners, so mint baselines in the environment that gates
PRs (or keep `pixels` tolerant). When CI diffs a locally-minted baseline,
re-mint in CI — the ui-goldens workflow exposes an `update-goldens`
dispatch job that does this and opens a PR with the CI-rendered PNGs.

## Authoring shortcuts

- `agent-qa flush --auto-shots` appends a shot claim after every do-step —
  full-page visual coverage for free; prune the noisy ones.
- `agent-qa crawl <url>` drafts a goto+shot pair per same-origin route —
  a smoke skeleton covering every linked page.
- The workbench editor adds a visual check for a step with its camera
  button; the Runs pane renders failed diffs inline and can re-mint the
  baseline (one claim or the whole run) without the terminal.

## Masking volatile regions

`{"shot": "<stepId>", "mask": ["<css>", ...]}` hides elements (clocks, live
counters, random ids) during capture — union of every claim's mask becomes
one injected `visibility:hidden` stylesheet around each screenshot. Mark
volatile markup with `data-qa-volatile` once and mask on that attribute.

## Responsive goldens

`do/viewport` before a shot step resizes mid-scenario — one scenario can
hold a 1280x800 baseline and a 375x812 baseline for the same surface.
`evals/selftest` demonstrates the pattern.

## Stability

Shot-claim scenarios always navigate fresh (the warm-page goto skip is
bypassed so the mask applies to a live document), and replay waits for
fonts + a stable layout tick before screenshotting. Pixel diffs ignore
sub-pixel antialias jitter (channel delta ≤ 32/255) — cross-host font AA
doesn't flake goldens. If a run still flakes visually, prefer `clip` or
`mask` to narrow the compared region over raising `pixels` — a tolerance
you have to keep raising is a scenario with a real instability.

## Failure output

A miss writes `shots-diff/<stepId>.diff.png` (red overlay on the run shot)
next to the run dir and fails the step with the differing-pixel fraction.
Read the diff image first — it tells you whether drift is content (re-mint)
or a bug (fix the app).

## Structural goldens (domshot claims)

`{"domshot": "<stepId>"}` is the text counterpart of `shot`: it diffs
the step's ARIA snapshot (`snapshots/<stepId>.txt` — written per step
already) against `baselines/<stepId>.snap.txt`. Prefer it when the
regression you care about is *structural* — an element appearing,
disappearing, a role/name change — because the tree is immune to font
rasterization and AA noise, and the failure artifact is a unified text
diff (`<run>/domshots-diff/<stepId>.diff.txt`) an LLM can read directly.

```json
{ "claim": { "subject": { "domshot": "s2", "skip": ["generated \\d+:\\d+"] }, "predicate": "matches" } }
```

- `skip` lists regexes; lines matching any drop from BOTH sides before
  comparing — the text equivalent of `mask` (volatile rows, timestamps).
- `@eN` element refs are normalized to `@e` automatically (numbering
  shifts across runs — never hand-compare on refs).
- Only `matches` is supported; equality is exact after normalization.

Mint with `agent-qa domshot-accept <sid>` (same flags as `shot-accept`:
`--dry-run`, `--steps`, `--json`; `--update-baselines` on replay mints
both kinds). Minting defaults to the ids domshot claims actually
reference — check-step sidecar snapshots are not minted into baselines
(they'd be orphan goldens). Lint rules `domshot-without-baseline` and
`orphan-baseline` cover the text side the same as the pixel side.
