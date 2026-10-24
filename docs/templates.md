# Templates

Reusable step lists declared on a scenario and inlined by `useTemplate`.

## What a template is

A template is an object on the scenario, under the top-level `templates`
map. Each entry needs a `steps` array (the same step shape as the
scenario). `intent`, `inputs`, and nested `templates` are optional.

```json
{
  "templates": {
    "openSettings": {
      "intent": "open the settings panel",
      "steps": []
    }
  }
}
```

A `do` step with `"verb": "useTemplate"` and
`params.template` set to that name is expanded, before dispatch, into
the template's steps. The runner walks nested templates up to 64 levels
and errors if a cycle goes deeper. There is no separate
`<scenarioDir>/templates/` directory and no `runTemplate` verb.

## When to author one

Reach for a template when the same gesture sequence appears more than
once in a scenario. Common cases:

- A multi-step "create entity X" flow used as a setup precondition.
- A "navigate to settings, open the X panel" preamble.
- A cleanup sequence several branches of the scenario all want.

If a sequence only appears once, keep it inline.

## Scope

Expanded steps join the parent step list, so they share the run's value
scope. A value saved before the `useTemplate` step is visible inside the
template, and a value saved inside the template is visible after it.
Templates are inlined, not sandboxed.

## See also

- `cli/src/runner.rs` — `flatten_steps`, which expands `useTemplate`.
- `schema/scenario-schema.json` — the `Template` definition.
- [`architecture.md`](architecture.md) — where the scenario contract sits.
