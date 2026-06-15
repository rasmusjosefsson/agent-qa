## Assert steps (record-time-validated contract checks)

> **The user's stated intent IS the contract.** If the user said "make
> sure the `<entity>` is created" / "assert the row was deleted" /
> "verify the email updated", the assertion's `intent` field MUST be
> that verbatim claim. Not "submit visible". Not "Overview tab visible".
> Not "modal closed". Failure messages quote `intent` — make them
> readable as the contract being broken.
>
> **Don't invent assertions.** If the user didn't ask for one, don't
> record one. Asserts are a contract the user signed; the agent doesn't
> get to add unsolicited contracts.
>
> **Don't downgrade silently.** If you can't prove the user's claim from
> the page, the recorder will refuse the step (exit 4). Surface the
> available signals to the user — don't quietly substitute a weaker
> assertion.

`assert` is a fourth step kind alongside `navigation` / `action` /
`wait`. Use it after an action when the user explicitly asks for
verification — "the `<entity>` is created", "the deleted `<entity>` no
longer appears". Replay halts on a violated assert; the recorder
_also_ refuses to persist an assert that's already false at the moment
you try to record it (exit 3) AND refuses asserts whose claim isn't
backed by an identity-grade page signal (exit 4).

### The determinism-ordered ladder

When a user asks to assert "X was created / changed / deleted", walk
this ladder top-down and pick the FIRST signal the live page exposes:

| Rank | Signal kind                                                                   | Drift surface                                        | Use when                                                                                                         |
| ---- | ----------------------------------------------------------------------------- | ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| 1    | **URL**                                                                       | Almost none — `location.pathname` is a stable string | Create-flows that route to `/<entity>/<id>` (or any nested noun path ending in `/<id>`). **Most deterministic.** |
| 2    | **DOM identity** (`present` / `text` on a heading containing the typed value) | i18n / copy / DOM refactors / role drift             | The typed name is visible in a heading or labelled element. **Proves _which_ entity.**                           |
| 3    | **Toast / alert** (`present` on `[role=alert]`)                               | Auto-dismiss timing, environment differences         | A confirmation banner names the action. Last-resort confirmation.                                                |
| 4    | **List row** (`text row <typedName>`)                                         | Pagination / sort / virtualisation                   | A new row in a list. Brittle — row may be hidden even when entity exists.                                        |

For "create" flows where both URL and DOM identity are available, record
**two asserts** — URL first, DOM identity second. Replay halts on whichever
fails first; you get richer triage signal.

### Snapshot-first assert discovery (MANDATORY before picking a rung)

**Before recording any assert, read the snapshot of the prior step and
grep for the typed value(s).** Every `record-step` writes
`tmp/agent-qa-scenarios/<SID>/snapshots/NNN.txt` post-settle — the same
output `agent-browser snapshot -i` produces. The snapshot of the
post-action page (the gesture you're about to assert on) is the one
indexed at the LAST recorded step before your assert. **Use the
on-disk file — do NOT call `agent-browser snapshot` again.** Re-snapshotting
is slower, racy (page may have changed), and wastes the snapshot the
recorder already captured for exactly this purpose.

> **Aside: assert steps ALSO get a snapshot** (`snapshots/<assertIdx>.txt`),
> but it's written AFTER the assert commits — too late to influence
> which rung to record. The assert-step snapshot exists for two
> downstream uses: (a) `compare` pairs the recording's vs replay's
> assert snapshot to surface UI drift when the predicate boolean is
> identical; (b) post-hoc triage when the user disputes a passing
> assert ("you asserted URL but the row didn't appear"). For assert
> SELECTION you always want `snapshots/<lastNonAssertStep>.txt`.

```bash
# 1. Find the snapshot for the last recorded step (the one whose result
#    your assert will check). Step indices are 0-padded to 3 digits.
SID=$(cat tmp/scenario.env | grep ^SID= | cut -d= -f2)
LAST=$(printf '%03d' $(($(wc -l < tmp/scenario.steps.jsonl) - 1)))
SNAP="tmp/agent-qa-scenarios/$SID/snapshots/$LAST.txt"

# 2. Grep for every value the user might want asserted on. For a created
#    entity, that's typically: the name parts (firstName, lastName,
#    company, etc.), the email, the slug, the id (if visible). Use the
#    literal record-time values (from `fill-unique`'s `typing:` log line)
#    — bindings have not yet been substituted in the snapshot.
grep -iE "<firstNameValue>|<lastNameValue>|<emailValue>" "$SNAP"
```

Read the grep output and map each match back to a rung:

| Snapshot line shape                        | Rung                          | Example                                                 |
| ------------------------------------------ | ----------------------------- | ------------------------------------------------------- |
| `- heading "<value>" [level=N, ref=...]`   | DOM identity (`present`)      | `heading "Mock Foo Bar Baz" [level=6]`                  |
| `- link "<value>" [ref=...]`               | DOM identity (`present` link) | `link "qa-foo@example.com"`                             |
| `- text "<value>"` inside a `row`          | List row (rung 4)             | inside `- row "..."` ancestor                           |
| `- generic [role=alert] ... "Created <X>"` | Toast (rung 3)                | top-of-tree banner                                      |
| URL bar shows `/<entity>/<numericId>`      | URL (rung 1)                  | inspect the same step's `probes/NNN.json` → `url` field |

**Rule:** record the highest-rung assert backed by the snapshot, AND a
URL assert if a `/<entity>/<id>` route is also live. If the DOM-identity
rung shows a heading containing the typed value, that's strictly
stronger evidence than URL alone — URL only proves "some entity exists
at this id"; the heading proves "the entity you just typed is the one
on screen."

**When the binding token machinery fails** (see gotchas.md — `fill-unique
--save-as` bindings can be invisible to `record-step assert`'s validator),
do NOT silently fall back to URL-only. Instead:

1. Read `snapshots/<LAST>.txt` and confirm the typed value IS visible somewhere.
2. Report to the user: "the bound value is on screen in `<rung>`, but
   the token validator refuses `{{vars.<name>}}` — I can record URL-only
   (weaker, doesn't prove _which_ entity) OR hard-code the recorded
   literal `<value>` (replay-incompatible). Which do you want?"
3. Never invent a third option.

### Create-flow URL lag (any SPA — read before recording asserts)

Post-submit SPA route flips commonly lag the modal-close signal by
200-1200ms (server roundtrip → list refresh → side-panel mount → route
push to `/<noun>/<id>`). The `intentSatisfied` predicate for `submit`
intent returns true on the FIRST observed change — typically the
dialog drop — so without protection the next step records mid-flight
state and the assert quality checker sees no `/<noun>/<id>` URL.

`smart-click` mitigates this automatically: after a `submit`-intent
click satisfies the first-change check, it polls `location.pathname`
for stability (default 800ms idle, 3000ms cap) and re-probes the full
state once settled. So in the common path you do nothing.

If you bypass `smart-click` (e.g. plain `agent-browser click @<ref>`
on a Save / Add / Create / Submit / Update / Delete / Confirm button),
**insert `agent-browser wait 1500` yourself before the next
`record-step`**. Otherwise the keyframe captures pre-navigation state
and the URL-rung assert refuses with empty `availableSignals[]`.

Tune via env: `URL_SETTLE_IDLE_MS=1500` for slow staging,
`URL_SETTLE_IDLE_MS=0` to disable for tests / non-routing submits.

### Save every fill value the post-create UI might surface

The DOM-identity and URL rungs are useless without saved values on the
right values. **For every `fill-unique` whose value the post-create UI
might display in a heading, breadcrumb, URL slug, or list-row link,
pass `--save-as <name>`.** Cheap insurance — `--save-as` costs nothing if
unused.

A common failure mode: bind only the entity's "name" field, then
discover at assert time that the post-create page shows
`<firstName> <lastName>` or `<prefix>-<id>` and you can't construct
the link / heading text deterministically. Result: truncate + re-narrate
5-10 steps to retroactively add bindings (per [`recovery.md`](recovery.md)).
Avoidable by binding everything upfront.

### `--save-as` is required for DOM-identity asserts on per-replay values

The DOM-identity rung asserts a value visible on the page. If that value
came from a `fill-unique` step (e.g. an `<entity>` name typed at create
time), it's different every replay (`Mock <Entity> {{vars._unique}}`
generates a fresh string per replay). The assert can only match the
typed value if the scenario **saved** that value at fill time and the
assert references the saved var:

```bash
agent-qa fill-unique \
  "<Entity> name" --template "Mock <Entity> {{vars._unique}}" --save-as <entityName>

agent-qa record-step assert \
  '{"kind":"present","args":["heading","{{vars.<entityName>}}"],"intent":"the <entity> is created"}'
```

**Concrete (fictional) example with the placeholders filled in for
copy-paste readability — substitute your actual entity:**

```bash
agent-qa fill-unique \
  "Widget name" --template "Mock Widget {{vars._unique}}" --save-as widgetName

agent-qa record-step assert \
  '{"kind":"present","args":["heading","{{vars.widgetName}}"],"intent":"the widget is created"}'
```

`--save-as <name>` writes a minted scenario/v1 binding onto the action
step. At replay time, the engine generates the unique value once,
types it, AND stores it under `<name>` so downstream `{{vars.<name>}}`
references resolve to the same value within one replay.

Without `--save-as`, the typed value is one-shot — regenerated per replay
but unreferenceable. Use `--save-as` whenever a downstream assert (or
another fill / URL pattern) needs the same value.

### Token grammar (read this once, then never again)

One grammar end-to-end: `{{vars.<name>}}`. The recorder rejects
anything else at record time with a copy-pasteable hint, and `verify`
rejects anything else post-flush. The bare `{{name}}` form (and
legacy `{{unique:*}}` / `{{uuid}}` / `{{timestamp}}`) is dead.

| Where                                                     | Token shape                                                    | Resolves to                 |
| --------------------------------------------------------- | -------------------------------------------------------------- | --------------------------- |
| `fill-unique --template "..."`                            | `{{vars._unique}}`                                             | per-step / per-replay nonce |
| `--save-as <name>` declares a binding                     | `<name>`                                                       | the typed value             |
| any later step's `args` (assert, fill, URL pattern, etc.) | `{{vars.<name>}}`                                              | the saved binding value     |
| also valid in args                                        | `{{vars._now}}`, `{{vars._runId}}`, `{{vars._currentProfile}}` | builtins                    |

**Common mistake the recorder now catches:** writing `{{accountName}}`
(no `vars.` prefix) or `{{vars.accountName_unique}}` (made-up suffix)
in assert args. Both refuse with `bare token '...' is not valid
scenario/v1 grammar. Use '{{vars.accountName}}' instead.` at record
time — no orphan keyframe, no silent literal substitution.

### `present heading "{{vars.boundValue}}"` vs `text heading <name> <expected>`

When the bound value IS the visible accessible name (the heading's
`textContent` equals the typed value), use **`present`** with two args:

```bash
'{"kind":"present","args":["heading","{{vars.widgetName}}"],"intent":"the widget is created"}'
```

The `name` arg doubles as the identifier AND the value being asserted —
no separate `expected` is needed. Predicate matches because the heading's
text contains the bound value.

When the heading text is something distinct from a value you typed
(e.g. a static title displayed alongside a separate dynamic field), use
**`text`** with three args — `[role, name, expected]` where `name`
identifies _which_ heading and `expected` is the text to compare:

```bash
'{"kind":"text","args":["heading","Welcome banner","Welcome {{vars.firstName}}"],"intent":"banner shows username"}'
```

Rule of thumb: if the bound value IS the heading text, use `present`.
If you're checking a non-bound expected string against a heading
identified by a fixed name, use `text`.

### Assertion kinds

> **Role availability gotcha — read this before picking a role.** The assert predicate matches **explicit `[role="..."]` attributes** in the DOM, plus a small special-case set: `button` (matches `<button>`), `link` (matches `<a[href]>`), `textbox` (matches `<input>`/`<textarea>`), `checkbox`, `combobox` (matches `<select>` and `[role="combobox"]`), and `heading` (matches `h1..h6`). **`cell`, `row`, `rowgroup`, `cell`, `gridcell`, `listitem`, etc. are NOT matched** even when `agent-browser snapshot -i` shows them in its accessibility tree — the snapshot tool computes virtual roles from semantic HTML (e.g. a `<div>`-based table renders as `row`/`cell` in the tree but has no `role=` attribute). The predicate's selector is `document.querySelectorAll('[role="X"]')` plus the special cases above; nothing else.
>
> **Practical implication:** for SPA list pages where the user-facing identifier is a row, walk the snapshot for the FIRST role in this priority order that contains the value you want to assert: `link` (most common — name links to detail pages), `heading`, `button`, then `textbox` for value asserts. If none of those carry the value, you need a different gesture (navigate to the detail page where the value lands in a `heading`) — not a different role.
>
> **Verify the predicate before recording:** `cli inspect --json` includes the URL + dialog state. To verify what `present role:X "Y"` would match, run the equivalent eval directly:
>
> ```bash
> agent-browser --session <s> eval '(()=>document.querySelectorAll(\'[role="X"]\').length)()'
> ```
>
> If the count is zero, the assert WILL fail at record time with exit 3 — pick a different role.

```bash
# URL identity — most deterministic. args[0] is a regex source string
# matched against location.pathname + location.search.
# Pattern shape: '^/<entity>/<id-pattern>' — substitute the route the
# entity actually lives at. The probe suggests this for you on exit 4.
agent-qa record-step assert \
  '{"kind":"url","args":["^/widgets/\\d+"],"intent":"the widget is created"}'

# DOM identity — assert the bound value is the heading text.
agent-qa record-step assert \
  '{"kind":"present","args":["heading","{{vars.widgetName}}"],"intent":"the widget is created"}'

# Presence / absence — element visible or gone after the action.
agent-qa record-step assert \
  '{"kind":"absent","args":["row","Old Widget"],"intent":"the widget was deleted"}'

# Count — N matching elements.
agent-qa record-step assert \
  '{"kind":"count","args":["row","Widget",3],"intent":"three widgets remain after delete"}'

# Value — input/textbox value matches expected.
agent-qa record-step assert \
  '{"kind":"value","args":["textbox","Email","admin@example.com"],"intent":"email field prefilled"}'
```

**Required fields:**

- `kind` — `present | absent | text | count | value | url`
- `args` —
  - `url`: `[pattern]` (regex source string, no flags)
  - `present` / `absent`: `[role, name]`
  - `text` / `value`: `[role, name, expected]` (string)
  - `count`: `[role, name, expected]` (number or numeric string)
- `intent` — **required**, non-empty. The user's verbatim claim. An
  assertion without an intent is a check, not a contract.

### Exit codes

| Exit | Meaning                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0    | OK — keyframe landed, predicate true, signal-backed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 2    | Heal-mode failure detector tripped (alert / lingering modal)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 3    | Predicate evaluated FALSE at record time (contract already broken). Stderr carries `detail` from the predicate; structured payload at `<sid>/failed/assert-refused-NNN-<isoTs>/payload.json` (`refusalReason: predicate-false-at-record-time`). Retry at same step index auto-stores the orphan keyframe.                                                                                                                                                                                                                                                                                                                                        |
| 4    | Claim looks like creation/mutation (`created` / `deleted` / `updated` / etc.) but the recorded `(kind, args)` doesn't match any identity-grade signal the page exposes. **Stdout carries structured JSON: `{unprovable, stepIndex, claim, recordedAssert, availableSignals[], hint}`. Same JSON also persisted to a sidecar file at `<sid>/failed/assert-refused-NNN-<isoTs>/payload.json`** (path printed on stderr) — read from there if your shell/runner swallowed the stdout. Surface the available signals to the user; don't downgrade silently. Bypass for one step with `SKIP_ASSERT_QUALITY_CHECK=1` (use sparingly — defeats the rule). |

**On exit 4: the agent's job is to surface the available signals to the
user verbatim and ask which to use** — not to silently downgrade to a
weaker shell-presence assert. Example interaction:

> User: "assert the `<entity>` is created"
> Agent: (records) → exit 4
> Agent to user: _"Can't prove 'the `<entity>` is created' from this
> page. Available signals (most-deterministic first): URL
> `/<entity>/12345` (suggest pattern `^/<entity>/\d+`), heading
> containing `Mock <Entity> abc`. Pick one or weaken the claim."_

**Retrying after a refused assert (exit 3 OR exit 4).** When you
re-issue `record-step assert` at the same step index after a refusal,
the orphan keyframe the prior refusal left in `probes/NNN.json`
is auto-stored into the sibling `failed/assert-refused-NNN-<isoTs>/`
dir (alongside the payload from that prior refusal). No manual `rm`
needed; the evidence is preserved in the store. Both exit codes
write a `payload.json` so the retry trigger fires uniformly: exit 3
records `refusalReason: "predicate-false-at-record-time"` with the
predicate `detail`; exit 4 records `unprovable: true` with the
`availableSignals[]`. Other "keyframe exists" causes (parallel writer,
half-finished heal, manual mucking) still abort loudly — only
refusals trigger auto-store.

**Replay-time failure.** `replay` halts on `ok=false` and writes a
failure-time PNG at `<replay-dir>/<NNN>.assert-fail.png` so triage doesn't
need a re-run.

**The same JS runs at both sites** (record-time settle gate + replay-time
`performAssert`). Single source of truth: `cli/src/claims.rs`.