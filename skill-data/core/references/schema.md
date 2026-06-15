# `scenario/v1` schema + control flow — what an agent needs to know

You normally don't author `scenario.json` by hand — the recorder does. This reference exists for the rare cases you need to:

- Read a scenario to debug a failing replay.
- Hand-edit a step (e.g. fix a stale binding).
- Author a template under `templates/` for `runTemplate`.
- Reason about why the schema is shaped a particular way.

## Canonical sources

| What                                                        | Where                                                         |
| ----------------------------------------------------------- | ------------------------------------------------------------- |
| Live JSON schema (validated at flush)                       | `schema/scenario-schema.json`                                  |
| Rust types (Step union, Locator, Condition, etc.)           | `cli/src/scenario.rs`                                          |

When the live code disagrees with the proposal, **the live code wins.** Update the proposal then.

## What `scenario.json` looks like in one breath

```
{
  "scenarioSchema": "scenario/v1",
  "id": "...", "intent": "...", "tags": [...],
  "parameters": { ... }, "profiles": { ... }, "defaultProfile": "...",
  "defaults":  { timeoutMs, onFailure, gesture, runAs },
  "setup":     { nav, cookies, localStorage, featureFlags, gql, dialogPolicy },
  "steps":     [ /* the action timeline */ ],
  "teardown":  { gql, policy, onFailure },
  "meta":      { producer, producedAt, recordedAgainstOrg, recordedByProfile, healHistory[] }
}
```

Every step carries a **`stepId`** (heal markers + profile gates reference it, not the array index); a `type` discriminator; named fields per type; optional `runFor` / `skipFor` / `expectFailFor` for per-profile gating; optional `evidence` channel map.

## Control-flow step types — implemented but author-facing

All five exist in the code (types, schema, validator, preflight, replay handlers in `cli/src/runner.rs`, dispatch wiring, unit tests):

| Step          | What it does                                                                         | Producer that emits it                                            |
| ------------- | ------------------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| `if`          | Branch on a `Condition`; runs `then[]` or optional `else[]`                          | None — author-only                                                |
| `while`       | Loop `do[]` while `Condition` holds; bounded by `maxIterations` + `timeoutMs`        | None — author-only                                                |
| `forEach`     | Fan `do[]` over JSONPath into prior data; loop var via `as`                          | None — author-only                                                |
| `group`       | Visual nesting / labelled batch; passthrough semantics                               | None — author-only                                                |
| `runTemplate` | Load `templates/<name>.json`, substitute `params`, execute; optional `saveResultsAs` | None — author-only                                                |

So **`group` is the only control-flow step you'll see in recorded or imported scenarios today.** The other four work end-to-end but no pipeline emits them; you only encounter them if a human authored the scenario or built a template.

## The `Condition` union (used by `wait`, `if`, `while`)

```
{ kind: 'elementVisible',  target: Locator, timeoutMs? }
{ kind: 'elementHidden',   target: Locator, timeoutMs? }
{ kind: 'elementExists',   target: Locator }
{ kind: 'varDefined',      name: string }
{ kind: 'varEquals',       name: string, value: unknown }
{ kind: 'urlMatches',      comparator: Comparator, value: string }
{ kind: 'flagEnabled',     name: string }
```

**Raw JS conditions are deliberately rejected.** The runner doesn't ship a JS sandbox. If a use case needs more, add a new `Condition.kind` — don't introduce a string-eval escape hatch.

## When to reach for control flow

You almost never should from a recording context. The recorded `steps[]` is strictly linear. If you find yourself needing branching:

1. **Most common alternative is per-step gating.** `runFor: ["admin-user"]` / `skipFor: ["viewer"]` / `expectFailFor: [...]` covers "this gesture is profile-X-only" and "this assert is expected to fail for profile Y" without adding control flow.
2. **`wait` with a `condition` ≠ branching.** It pauses until the condition holds; it doesn't choose a different path. Use it when the page sometimes shows a transient loader.
3. **`runTemplate` IS the right move for repeated sub-flows** (login, seeding, common cleanup) once a sub-flow is used in ≥3 scenarios. Templates live under `templates/` and are scenario-shaped with `params[]` at root.
4. **`forEach` is real value for bulk teardown.** Walk a GQL response (`each.source` + `each.path` as JSONPath into prior data) and run `do[]` per element. Note: distinct from `setup.gql[].forEach`, which fans one GQL op over data — that's a setup-only feature, not the step type.
5. **`while` is rare.** Use it for "click 'Load more' until it disappears" patterns. Always bound with both `maxIterations` AND `timeoutMs`.

## Why the schema is shaped this way (one-line per decision)

- **Single-level `type` discriminator** (not `type: action` + `method: click`) — readable from the first key, validator narrows with one branch.
- **Named fields, not positional `args[]`** — readable without producer code, not locked to a function signature.
- **Named bindings (`saveAs` + `{{vars.<name>}}`)** — insertions don't silently rewire references like positional `{{steps[3].args[1]}}` would.
- **Typed `within[]` of locator objects** — machine-authorable, resolves through Playwright's `.filter({ has })`.
- **Setup / teardown as top-level fields, not step types** — keeps the action timeline auditable; teardown gets a natural finally block.
- **Closed locator-strategy enum** (`role/label/text/testid/placeholder/title/altText/css/xpath`) — keeps captures portable; `css`/`xpath` are visible escape hatches, not defaults.
- **Intent required on asserts** — runner refuses to persist asserts not pinned to an identity-grade signal; vacuous green prevented at record time.

Whole models explicitly rejected: another-tool chained subjects (`.find().within().should()`), QA Wolf code-as-test, Shiplight DRAFT mode.

## Don't trust this file over the code

If you read something here that contradicts `schema/scenario-schema.json` or `cli/src/scenario.rs`, the code is right and this file is stale. Fix this file.
