# Per-locator heal opt-out (`heal: { mode: "off" }`)

> **TL;DR.** Set `heal: { "mode": "off" }` on a `Locator` to declare
> the recorded name load-bearing. The replay resolver then requires a
> literal match (modulo trim) and refuses to fall through into the
> digit-tolerant / generated-suffix tolerant tiers. Omitting `heal` (or
> writing `heal: { "mode": "auto" }`) keeps today's behavior — they
> are byte-identical. `AGENT_QA_NO_HEAL=1` is unchanged and still wins
> over any per-locator setting.

## When to reach for this

By default the resolver heals through accessible-name drift caused by
counters (`"Submit 19987 items pending"`) and `generated id`-style
suffixes (`"Parent qrstuvwx"`) — see `gotchas.md` § "Numeric-suffix
drift in accessible names is handled by replay". That default is
right for almost every recording.

You only need `heal: { "mode": "off" }` when the recorded name **is
the test** — clicking a different row / option is a false positive,
not a healable variant. Examples:

- Picking `"Order #19856"` specifically. A drift to `"Order #19857"`
  must fail the step instead of silently clicking the wrong order.
- Choosing the row for a fixed seed entity whose suffix the test
  itself baked in. If the suffix drifts, that's a data-setup bug,
  not something the resolver should paper over.

If the recorded name is incidental (just whatever the recorder
captured), leave `heal` absent. The default tolerant tiers will keep
the scenario replayable when the live name drifts at replay time.

## Shape

```jsonc
{
  "stepId": "click-target-order-row",
  "type": "click",
  "target": {
    "by": "role",
    "value": "row",
    "name": "Order #19856",
    "nameMatch": "exact",
    "heal": { "mode": "off" },
  },
}
```

Two notes:

1. `heal` is independent of `nameMatch`. `nameMatch` is the
   syntactic dial (`"exact" | "contains" | "regex"`); `heal` is the
   tolerance dial. `"contains"` and `"regex"` ignore the tolerant
   tiers already, so `heal` is only meaningful on `"exact"` (the
   default).
2. Schema: closed enum `mode: "auto" | "off"`. `"auto"` is the
   default; omitting the field is identical to writing
   `"heal": { "mode": "auto" }`. Future graded heal levels (e.g.
   `"whitespace-only"`) will be added by growing the enum vocabulary,
   not by reshaping the field.

## Resolver precedence (locked behavior)

| `AGENT_QA_NO_HEAL` | `locator.heal.mode` | Effective behavior                                      |
| ------------------ | ------------------- | ------------------------------------------------------- |
| unset              | (omitted) / `auto`  | literal trim → digit-tolerant → generated-suffix (default) |
| unset              | `off`               | literal trim only — fail if drift would have rescued    |
| `=1`               | (any)               | literal trim only — env wins, process-wide              |

The env-var precedence is intentional: env is the operational kill
switch for deterministic-dispatch sweeps and a careless author must
not be able to paper over a CI-wide strictness sweep by writing
`"mode": "auto"` on a locator.

## If you came from Playwright

Playwright's `getByRole({ name, exact })` has TWO knobs:

- `name` — the pattern.
- `exact` — case-sensitive whole-string match.

agent-qa keeps the same split, but the second axis is different.
`nameMatch: "exact"` is the **syntactic** dial (literal vs regex vs
substring) — analogous to Playwright's `exact`. `heal: { mode: "off" }`
is the **tolerance** dial: it disables the resolver's drift-tolerant
fallback tiers, which Playwright doesn't ship at all. The two dials
are orthogonal; set both if you want literal + no-heal.

## What happens on a miss

When `heal.mode: "off"` is set (or `AGENT_QA_NO_HEAL=1` is in the
env) and the literal trim-compare fails, the step fails with the
standard `locatorMissMessage` path. The resolver runs a
single second-pass probe to ask "what tier WOULD have rescued this
if heal were on?" and surfaces the answer two ways:

1. **The thrown miss message gains a suffix** naming the tier:

   ```
   locator failed for click drifted-button: no node matched by=role value='button' (name '91 days') (would have matched via digit-tolerant)
   ```

   When the probe finds nothing in the search subtree that would
   have matched even with tolerance, the suffix is omitted — the
   drift is real, not a one-character hiccup, and the operator
   should NOT chase a phantom heal fix.

2. **A new `[replay]` audit line** prints to stderr, prefixed
   `would-have:` so it's unambiguously distinct from the existing
   `[replay]   <tier> match: ...` lines that fire when a tier
   actually heals:

   ```
   [replay]   would-have: digit-tolerant rescued: "91 days" → "90 days"
   ```

   The role-fallback layer emits the sibling line when the in-page
   matcher detects the would-have rescue independently:

   ```
   [replay]   would-have: digit-tolerant rescued (role-fallback): "91 days" → "90 days"
   ```

   Suppress both with `AGENT_QA_QUIET_HEAL=1` (same flag that
   suppresses the existing `<tier> match:` audit lines — coupled by
   default).

The two surfaces are independent: the thrown error always carries
the diagnostic suffix for CI reports; the audit line gives operators
a `rg`-able trail across a full replay.

A bare `[replay]   <tier> match:` line (NOT prefixed `would-have:`)
on stderr alongside a strict-mode failure remains a regression
signal — the in-page matcher is firing the heal tier despite
`mode: "off"`. The diagnostic supplements, but does not replace,
the strictness contract.

## Implementation seams (for the next agent)

The two layers that compute the opt-out share a helper to prevent
drift:

- `cli/src/scenario.rs:isNoHeal(loc)` — `env || loc.heal?.mode === 'off'`.
- `cli/src/scenario.rs` (Node-side snapshot resolver `nameMatches`) —
  calls `isNoHeal(loc)` before tier 1 / tier 2.
- `cli/src/runner.rs:roleFallbackRequest` (in-page
  fallback) — calls `isNoHeal(loc)` to set `request.noHeal`; the
  in-page predicate honors the bool unchanged.

Resolver paths that route through these helpers and therefore honor
the opt-out: `target`, every nested `within[]`, `has`, every entry of
`candidates[]`, and any assertion lookup that resolves via the same
`Locator` shape (`assertVisible`, `assertExists`, `assertText`,
`readText`, …). Strategies handled by the DOM-probe fast path
(`by: "css" | "xpath"`) never enter the tolerant tiers in the first
place, so `heal` has no effect on them.

Hand-validation playbook (run before merging any change to the
helper, the resolver gate, or the in-page matcher):
`manual-tests/heal-policy/README.md`. Ships three fixture
scenarios that isolate the `heal` axis on a live staging button.
