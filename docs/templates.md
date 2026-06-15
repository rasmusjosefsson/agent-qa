# Templates

Reusable sub-scenarios you can call from a scenario with `runTemplate`.

## What a template is

A template is a small JSON file containing an ordered list of steps, validated against the same scenario schema.
Templates live next to the scenario that uses them:

```
<scenarioDir>/templates/<name>.json
```

A step of kind `runTemplate` loads one by name and executes its steps inline at replay time, as if they had been
written into the parent scenario.

## When to author one

Reach for a template when the same gesture sequence appears in more than one scenario, or more than once in the same
scenario. Common cases:

- A multi-step "create entity X" flow that several scenarios need as a setup precondition.
- A "navigate to settings, open the X panel" preamble.
- A cleanup sequence that several scenarios all want to run before asserting.

If a sequence only appears once and is unlikely to repeat, keep it inline. Templates are not a code-golf win — they
add a layer of indirection, and the cost is real when you read a scenario and have to chase what the template does.

## Resolution rules

Template names are deliberately constrained. The resolver in `cli/src/runner.rs` enforces:

- The name must match `^[A-Za-z0-9._-]+(?:\.json)?$` — letters, digits, dot, dash, underscore. No slashes.
- The name must not be an absolute path.
- After resolution, the file must live inside `<scenarioDir>/templates/`. Any path that escapes the directory
  (via `..`, symlinks, or otherwise) is rejected.
- `.json` is appended automatically if omitted.

These rules exist so a malicious or careless scenario can't reach files outside its own folder.

## Validation

Templates are validated by `cli/src/schema.rs` at load time. The error message
identifies which template failed and which fields are wrong. A template that fails validation never runs.

## Scope and bindings

A template runs inside the same binding scope as its caller. That means:

- Values saved by `fill-unique --save-as foo` or `setup.gql[].saveAs` in the parent are visible inside the template
  as `{{vars.foo}}`.
- Values saved inside the template are visible after the `runTemplate` step returns.

There is no isolation. Templates are inlined, not sandboxed.

## Authoring loop

1. Record the gesture sequence once as part of a scenario.
2. Extract the relevant step rows into a new file under `<scenarioDir>/templates/<name>.json`.
3. Replace the inline steps in the scenario with a single `runTemplate` step that names the template.
4. Replay and confirm the scenario still passes.

The on-disk layout means a template travels with the scenario that introduced it. Templates are not (yet) a
package-level library shared across scenarios.

## See also

- `agent-qa skills get core` — operational syntax for `runTemplate` and the recording verbs that produce it.
- [`architecture.md`](architecture.md) — where `templates/` sits in the artifact tree.
- `cli/src/runner.rs` — the resolver and loader.
- `cli/src/schema.rs` — the validator.
