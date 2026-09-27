# Visual testing — golden screenshots

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
(red pixels over a faded baseline) and fails the step with the diff ratio. A
missing baseline fails with the `shot-accept` hint — baselines are minted, never
implicitly trusted.

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

## Authoring tips

- Pin `do/viewport` early; baselines record whatever the viewport was at mint.
- Prefer clipped shots for widgets that live among dynamic chrome (nav
  indicators, timestamps).
- `flush --auto-shots` + one replay + `shot-accept` is the three-command path
  from a recording to a golden suite.
