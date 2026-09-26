# `scenario/2` reference

`schema/scenario-schema.json` defines the scenario contract. `cli/src/scenario.rs`
defines the matching Rust types. The schema validates a scenario before replay.

A scenario has an `id`, an `intent`, replay steps, optional `env` setup and cleanup,
and optional provenance.

```json
{
  "schema": "scenario/2",
  "id": "open-users",
  "intent": "open the users page",
  "env": {
    "open": [{ "kind": "nav", "url": "https://example.com/users" }]
  },
  "steps": [
    {
      "id": "s0",
      "intent": "open users",
      "kind": "do",
      "verb": "goto",
      "value": { "from": "literal", "literal": "https://example.com/users" }
    },
    {
      "id": "s1",
      "intent": "users are visible",
      "kind": "check",
      "claim": {
        "subject": { "element": { "role": "heading", "name": "Users" } },
        "predicate": "isVisible"
      }
    }
  ],
  "producedBy": {
    "producer": "agent-recorder"
  }
}
```

Use `record-step do` and `record-step check` to create steps. The recorder assigns
`id` and `kind`. Do not hand-edit them into drafts.

`env.open` and `env.close` accept the existing generic `EnvOp` kinds. They are
`fresh`, `useProfile`, `nav`, `cookie`, `localStorage`, `gql`, and `flag`.
`record-setup` records one schema-valid `env.open` value.

### Native dialogs (alert/confirm/prompt)

Resolve a pending native dialog with a `dialog` do-step:

```json
{
  "id": "s2",
  "intent": "accept the alert",
  "kind": "do",
  "verb": "dialog",
  "params": { "action": "accept" }
}
```

`params.action` is `"accept"` or `"dismiss"`; `params.text` optionally carries
prompt input. Assert on a pending dialog with the `{"dialog": true}` check
subject — `exists`/`notExists` for presence; string predicates like `contains`
match the dialog's message.

Scenarios containing dialog steps replay with `AGENT_BROWSER_NO_AUTO_DIALOG=1`
set automatically (otherwise agent-browser auto-accepts `alert` before the
`dialog` step can observe it). Record with the same env var so the dialog
stays pending between the click that opens it and the resolving step —
prefer it over `agent-browser --no-auto-dialog`, which can swallow the first
navigation when the flag triggers the browser launch. `eval`, `snapshot`, and
screenshots cannot run while a dialog is pending, so per-step sidecars are
skipped for those steps.

Use `agent-qa scenario check <scenario.json>` before replay. It validates the
schema and runs the scenario linter.
