# Scenario authoring

Record a scenario through the CLI. The recorder owns the scenario id, step ids,
and step kinds. Do not create a separate setup file.

Use `record-setup` for generic preconditions that replay must restore. It accepts
one existing `EnvOp` value and writes it to `env.open` at flush time.

```bash
agent-qa record-setup '{"kind":"fresh"}'
agent-qa record-setup '{"kind":"flag","name":"example-flag","enabled":true}'
agent-qa record-setup '{"kind":"nav","url":"https://example.com/users"}'
```

Use `record-step do` for browser actions and `record-step check` for the result.
The recorder validates drafts before it appends them. `flush` seals the
scenario, including its setup and provenance.
