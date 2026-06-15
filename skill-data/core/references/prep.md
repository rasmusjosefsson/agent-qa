## env.open / env.close — Seeded State For Replay

Scenarios carry preflight setup and teardown under a single `env` key
with two arrays: `env.open[]` runs before the first step, `env.close[]`
runs after the last step (and on abort, depending on per-op policy).

```json
{
  "schema": "scenario/2",
  "id": "accounts-prefill-5",
  "intent": "open accounts with prefilled data",
  "env": {
    "open": [
      {
        "kind": "gql",
        "intent": "seed account",
        "url": "https://api.example.com/graphql",
        "query": "mutation SeedAccount($name: String!) { seedAccount(name: $name) { id name } }",
        "variables": { "name": "qa-{{vars._unique}}" },
        "saveAs": "account"
      },
      { "kind": "flag", "name": "new-accounts-list", "enabled": true }
    ],
    "close": [
      {
        "kind": "gql",
        "intent": "cleanup account",
        "url": "https://api.example.com/graphql",
        "query": "mutation Cleanup($id: ID!) { cleanupAccount(id: $id) }",
        "variables": { "id": { "$value": { "from": "step", "stepId": "account", "path": "$.id" } } },
        "policy": { "alwaysRun": true, "onFailure": "continue" }
      }
    ]
  },
  "steps": [
    {
      "id": "fill-account-name",
      "do": { "verb": "type", "by": "label", "value": "Account name", "text": "{{vars.account.name}}" }
    }
  ]
}
```

`EnvOp.kind` is one of: `nav`, `cookie`, `localStorage`, `gql`, `flag`.
Per-op behavior is controlled by `policy` (`alwaysRun`,
`onAbort: 'run' | 'skip'`, `onFailure: 'abort' | 'continue'`).

### Wiring saved values into later operations

Two channels, both generic — no the target app vocabulary:

1. **`{{vars.<name>}}` string templates** — substitute a saved binding
   (from `env.open[].gql.saveAs` or a recording-time `fill-unique
--save-as`) into any string field. Deep-path supported via
   `{{vars.<name>.field}}`.

2. **`{ "$value": <Value> }` variable wrappers** — at a `variables` leaf
   in an `env.open[].gql` / `env.close[].gql` op, pass a single-key
   object whose key is `$value` and whose value is a `Value` channel
   (`{from:'step'|'input'|'literal'|'mint'|'loop', ...}`). Lets a
   single-shot (non-`forEach`) op reference any earlier binding without
   needing the `{{item.*}}` placeholders that are scoped to `forEach`.

### Capped-resource cleanup — "drain enough slots, don't just delete one"

Many orgs cap a resource (`5 of 5 custom objects available`). A
deterministic `env.open[]` hook MUST free enough slots before the
scenario's main steps run. The canonical pattern (cap = `C`, scenario
creates `K` per run):

```json
{
  "kind": "gql",
  "intent": "list current things (for cleanup)",
  "url": "https://<env>.example.com/graphql",
  "query": "query ListThings { things { collection { id } } }",
  "variables": {},
  "saveAs": "existingThings"
},
{
  "kind": "gql",
  "intent": "delete up to (C-K) existing things to guarantee at least K free slots",
  "url": "https://<env>.example.com/graphql",
  "query": "mutation Delete($id: ID!) { deleteThing(id: $id) }",
  "variables": { "id": "{{item}}" },
  "forEach": { "from": "step", "stepId": "existingThings", "path": "$.things.collection[0:N].id" }
}
```

**Picking `N` for the JSONPath slice** is judgement, and getting it wrong
silently leaks state across runs (the scenario passes today, fills the
org tomorrow). Rules:

| Starting state                                        | Slice that survives forever in steady-state     | Slice that ALSO survives a polluted-org state (recommended default) |
| ----------------------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------- |
| Org always clean before run                           | `[0:K]` (delete only what this run will create) | `[0:C-K]` (drain almost everything; safest)                         |
| Org may have N leaked entities from prior broken runs | `[0:K]` LEAKS — run N+L will hit cap            | `[0:C-K]` recovers in one run                                       |

For the typical `C=5, K=1` case (this is the cap-bound custom-objects
fixture under `tmp/agent-qa-scenarios/`), use `[0:4]`. Rationale: after
delete the count is ≤1; after the create-step it's ≤2; cap never
reached for any starting state 0..5.

If your slice depth is `[0:1]`, you're saying "delete exactly one." That
keeps a clean org clean forever, but a polluted org never recovers — you
delete 1 per run, leak `K-1` per run (if `K>1`), or break the moment any
prior run leaked. **Treat `[0:1]` as the unsafe choice unless you can
prove no prior run ever leaked.**

Other variants:

- `[-1:]` — delete the most-recent thing only (useful for "undo my last run").
- `$.collection[*].id` — delete EVERY existing entity. Most aggressive; use
  when the org should be empty after cleanup. Be sure no other test or
  human work in this org will be wiped.
- `[0:1]` paired with a known-clean baseline — only safe when external
  invariants guarantee no leaks (e.g. a freshly-provisioned ephemeral org).

Do not couple cleanup intent to server-side `first: N` paging — the
scenario should declare its slice explicitly, in the JSONPath. (`first` on
the list query is for your own pagination cap, see below.)

### REWRITE the query's variables for cleanup intent (do NOT copy them verbatim from a captured query)

When you paste a list query out of a captured GraphQL POST into your cleanup
`env.open[].gql`, the variables block carries the values the **UI**
sent — those values are correct for rendering a list panel, almost
always WRONG for "find every entity the cleanup may need to delete."
The single biggest source of "the scenario starts failing after a few
runs because the org filled up" is the author copying the variables
unchanged. Rewrite them before pasting.

Concrete patterns to neutralise (substitute `null` or remove the variable
entirely; rely on the JSONPath slice to bound the iteration count):

- **Surface-scoping flags** — anything that names a UI region or
  visibility filter. Common keys: `inNavigation`, `inSidebar`, `visible`,
  `published`, `active`, `stored`, `isDraft`, `state: "ACTIVE"`,
  `status: "PUBLISHED"`. These exclude entities the cleanup needs to
  delete (newly-created entities default to "not in navigation" / "not
  published yet" → invisible to the cleanup list → leaked across runs).
  **Set every such flag to `null`** (or drop the variable; the server
  treats absent as "no filter").
- **Pagination caps** — `first: 1`, `first: 25`, `take: 10` etc. that
  match the UI page size. The cap should cover the worst-case org
  state, not the UI's default page. Use a number at least 2-3× the
  resource cap (so a delete-loop empties the org even if the cap was
  bumped server-side), OR use cursor pagination + a follow-up
  `env.open` op that walks `pageInfo.hasNextPage`. For simple cases
  `first: 100` is the right default.
- **Sort / ordering** — usually safe to leave as-is, but verify the
  recorded sort doesn't interact with a `first: N` cap to silently
  drop entities the cleanup needs (e.g. `sort: "RECENTLY_VIEWED"` +
  `first: 10` misses entities the user never viewed).
- **User / owner scoping** — `createdByMe: true`, `ownedBy: "<id>"`,
  `mine: true`. The cleanup probably wants to delete entities the
  recording session created, regardless of who owns them. Set these
  to `null` unless your cleanup is intentionally scoped (rare; usually
  the recording profile IS the owner so it doesn't matter, but be
  explicit).

The lesson from the canonical failure: a custom-objects scenario shipped
with `{ first: 1, inNavigation: true }` because the UI's navigation
panel had asked for "the first object that should appear in the side
nav." After each replay the new object was created with
`inNavigation: false` by default, so the cleanup list query returned
`[]` and the delete forEach iterated 0 times. The org accumulated one
object per run until cap, and run N+5 hit `2/6 FAIL` on the textbox
that doesn't exist when the create-object dialog lands on the
"at quota" screen. The right shape was `{ first: 100, inNavigation: null }`
plus the existing JSONPath slice. Verify your cleanup query returns a
non-empty list against a polluted-org state BEFORE shipping the scenario
— a `0/0` cleanup IS a passing replay until the org fills up.

### Sanity-check via the runtime warnings before shipping

The replay runner now emits structured `[v2-replay] env.<phase>[N]`
lines per op (kind, intent, saveAs, forEach iteration count, duration)
and a `WARNING:` line whenever a saveAs returns 0 rows or a forEach
iterates 0 times. Run your new scenario once against a polluted org
state and once against a clean state; in BOTH runs neither WARNING
line should fire on the cleanup ops (clean state legitimately fires
the saveAs=0 warning — that's fine; the polluted state must NOT). If
the polluted-state run shows `forEach=0` on the cleanup, the variable
rewrite above is the fix.

## Own-org safety (org pin)

Every `env.open[].gql` / `env.close[].gql` fire verifies the live
agent-browser session's org id matches the profile's bootstrap-time pin
BEFORE firing any GraphQL. Mismatch = throw with re-bootstrap hint, no
GQL traffic.

The pin is stamped automatically by `profile-bootstrap`:

```bash
agent-qa profile-bootstrap default
# [bootstrap:default] org pin stamped: org=12345
```

Flush stamps the active profile's org id onto the scenario at
`meta.recordedAgainstOrg`. Replay refuses if the replay profile's pin
doesn't match the scenario's recorded org — catches "wrong profile for
this scenario" before any seeding happens.

## Staging-only transport guard

Every URL passed to an `env.*.gql.url` is validated against an explicit
staging allow-list (`*.example.com`) before any fetch fires.
Production hosts are intentionally absent. New cells require a code
change + PR review.
