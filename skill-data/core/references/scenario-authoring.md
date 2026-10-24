# Scenario authoring

Record a scenario through the CLI. The recorder owns the scenario id, step ids,
and step kinds. Do not create a separate setup file.

Use `record-setup` for generic preconditions that replay must restore, beyond
the baseline `start` already wrote. It accepts one existing `EnvOp` value and
writes it to `env.open` at flush time. A default `start` seeds one
`{ "kind": "fresh" }`. `start --open <url>` also seeds a `nav` to that URL;
do not `record-setup` the same URL again. With no `--open`, record the entry
`nav` here. Also use `record-setup` for `cookie`, `localStorage`, `gql`, and
`flag`.

```bash
agent-qa record-setup '{"kind":"nav","url":"https://example.com/users"}'
agent-qa record-setup '{"kind":"flag","name":"example-flag","enabled":true}'
```

Use `record-step do` for browser actions and `record-step check` for the result.
The recorder validates drafts before it appends them. `flush` seals the
scenario, including its setup and provenance.
