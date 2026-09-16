# Design review — does it match the design, not just "does it work"

Replay answers **does it work**. This reference covers the other question:
**does it look like the design we agreed on**.

Keep them apart. A failed claim is always a bug. A design mismatch is
sometimes a deliberate deviation someone already accepted. Putting both in
one pass/fail lane ends the same way every time: either design noise blocks
every PR, or design gets muted to keep CI green. Both end with design ignored.

So one replay, two lanes: behaviour (`replay` exit code) and design
(`design review` exit code). CI can require the first and surface the second.

## The rule that matters most

**Judge layout, spacing, hierarchy, component choice, and state. Never text
content.**

A Figma frame shows mock copy, placeholder names, invented prices, lorem
paragraphs. The built page shows real data from a real account. Different
words are expected and are **never** a `fail`. Flag text only when the
*structure* of the text differs: a heading that became body copy, a label
that vanished, a truncation the design didn't have, three list items where
the design commits to a fixed set of exactly two.

Same for anything else the data drives — avatar images, row counts, chart
shapes, date values, badge colours that encode status. If the difference is
"different content", it is not a finding.

## Three depths — pick per ask

| The user's ask | Depth |
| --- | --- |
| "Does this Figma PNG match `<url>`?" | One-shot, no scenario |
| "Record this flow and check it keeps matching the design" | Scenario + designs |
| "Does the flow still work?" | Plain replay, no designs involved |

### Depth 1 — one-shot, no scenario

No `scenario.json`, no `designs/` folder, no gate. Screenshot the page,
compare against the export, answer in one turn.

```bash
agent-qa start "design review checkout" --open <url> --session dr
agent-qa browser screenshot --full /tmp/page.png
```

Then read `/tmp/page.png` and the user's Figma export side by side and report
per difference. Same rules as below: structure not content, and ask instead
of guessing.

If the user then says "keep checking this" — that's the cue to move to
depth 2. The PNG they already gave you becomes `designs/<stepId>.png`.
Nothing is re-done.

### Depth 2 — scenario + designs

The reference images are a **convention, not a schema change**. Nothing in
`scenario.json` points at them.

```text
<sid>/
  scenario.json
  designs/
    open-cart.png        reference export, named after the step id
    open-cart.md         optional notes: "8px gap, primary button right"
    verdicts.json        committed decisions
  replays/<runId>/
    screenshots/open-cart.png
    design-review.md     written by `design review`
```

Getting images in there is your job, not the user's — you know the step ids
from `scenario.json`, they don't. Whether the PNG arrives as a Figma export,
a paste in chat, or a Figma MCP fetch, it ends as a file at
`designs/<stepId>.png`.

Designs are sparse on purpose. Cover the steps that have a design worth
holding to. `design review` reports coverage ("3 of 11 steps have a design")
so an uncovered step reads as uncovered rather than passing.

## The loop

```bash
agent-qa replay <sid>                    # behaviour lane, as usual
agent-qa design review <sid>             # design lane; writes design-review.md
```

`design review` pairs every `designs/<stepId>.*` with that run's
`screenshots/<stepId>.png` and prints the pairs with absolute paths. Open
each pair, judge it, then record one decision per step:

```bash
agent-qa design verdict <sid> --step <id> --ok
agent-qa design verdict <sid> --step <id> --accepted --reason '<why the deviation is fine>'
agent-qa design verdict <sid> --step <id> --fail --reason '<what drifted>'
agent-qa design verdict <sid> --step <id> --ask --reason '<what the human must decide>'
```

Verdicts are **decisions, not results**:

| Verdict | Meaning | Gate |
| --- | --- | --- |
| `ok` | Matches | passes |
| `accepted` | Deviates, and we decided that's fine | passes |
| `fail` | Real drift | blocks |
| `ask` | You are not confident | blocks |
| `needs-review` | No verdict recorded yet | blocks |
| `stale` | Design file changed since the verdict | blocks |
| `no-screenshot` | Design exists, the step produced no shot | blocks |
| `unknown-step` | Filename matches no step id (usually a typo'd export) | blocks |

`accepted` is load-bearing. Without it, a known platform constraint gets
re-flagged on every run until nobody reads the output. `--accepted` and
`--fail` both require `--reason`: a deviation nobody justified is
indistinguishable from drift nobody noticed.

`design review` exits `0` only when every design is `ok` or `accepted`, and
`2` otherwise. It reads screenshots from the replay run and never drives a
browser.

## Never auto-pass ambiguity

When you cannot tell whether a difference is a bug or a decision — record
`--ask` and **stop and ask the user**. Do not reason yourself into `ok`
because the flow worked, and do not reason yourself into `fail` because a
pixel moved.

Ask when:

- the design shows a state you can't reach in this run (empty, error, loading);
- the design is ambiguous about behaviour you can't see in a still (hover, focus, motion);
- the build looks deliberate but different — a newer design system version, a component that got replaced;
- the design can't be built as drawn on this platform and you'd be guessing at the intended fallback.

The exit code holds the line: `ask` keeps the gate red until a human turns it
into `ok`, `accepted`, or `fail`. That is the mechanism, rather than trusting
a reviewer's restraint.

## Decisions expire when the design changes

Every verdict stamps the sha256 of the design file it judged. Re-export the
frame from Figma, the bytes change, the hash moves, and every verdict for
that step flips to `stale` — back to needs-review.

That kills "we reviewed that once in March". It also means you never have to
manually invalidate anything: shipping a new design *is* the invalidation.

`verdicts.json` is committed next to the scenario, so an accepted deviation
shows up in the PR diff as a line a reviewer can argue with.

## Using this on a PR

```bash
agent-qa replay <sid> --profile <p>      # must pass — behaviour
agent-qa design review <sid>             # surfaced — design
```

Attach `replays/<runId>/design-review.md` and the screenshot/design pairs as
the evidence. A red design lane with a clear `ask` is a better PR comment
than a green one that skipped the question.
