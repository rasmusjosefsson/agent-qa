---
name: core
description: Record and replay generic browser scenarios through CDP.
allowed-tools: Bash(agent-qa:*), Bash(agent-browser:*)
---

# agent-qa core

Use agent-qa when a browser journey must be recorded and replayed. The core is
generic. A downstream skill or plugin supplies product routes, profiles,
fixtures, feature flags, and cleanup policy.

## Before recording

Use one browser session for the full journey. If you attach agent-browser to an
external CDP browser, set the connection before every direct browser command and
every agent-qa command.

```bash
export AGENT_BROWSER_CDP=9223
export AGENT_BROWSER_PIN_TAB=1
```

You can put the same local values in `agent-qa.toml`.

```toml
[browser]
cdp = "9223"
pin_tab = true
```

`start` resolves the connection once and saves it in local recorder state. All
browser children during the recording inherit that same connection. The endpoint
and pin policy never enter `scenario.json`.

`--byo`, `--launch`, `--port`, `--tab`, and `--clone-profile` are not supported.
`agent-qa byo-doctor` only reports local browser availability.

## Record a replayable scenario

Start the recording. Pass `--source-ref` only when an opaque upstream reference
is useful to a future reader.

```bash
agent-qa start "verify the users page" --session qa-run --source-ref "change:123"
```

Record setup before actions. `record-setup` accepts existing generic `EnvOp`
shapes. Use it for repeatable `fresh`, `useProfile`, `nav`, `cookie`,
`localStorage`, `gql`, and `flag` operations.

```bash
agent-qa record-setup '{"kind":"fresh"}'
agent-qa record-setup '{"kind":"nav","url":"https://example.com/users"}'
agent-qa record-setup '{"kind":"flag","name":"example-flag","enabled":true}'
```

Drive one action. Then append one direct scenario draft. The recorder assigns
sequential ids and injects `kind`. Do not provide either field.

```bash
agent-browser --session qa-run open https://example.com/users
agent-qa record-step do '{
  "intent": "open users",
  "verb": "goto",
  "value": {"from":"literal","literal":"https://example.com/users"}
}'

agent-qa smart-click "Edit user"
agent-qa record-step check '{
  "intent": "the editor is visible",
  "claim": {
    "subject": {"element": {"role":"dialog","name":"Edit user"}},
    "predicate": "isVisible"
  }
}'
```

`smart-click` and `fill-unique` record direct `do` drafts. For a fixed manual
fill, drive the field and record a direct `type` draft. `record-step` accepts
only `do` and `check` drafts.

Flush and verify the sealed contract.

```bash
agent-qa flush
agent-qa scenario check <scenario.json>
agent-qa replay <sid> --session qa-run
```

Use explicit cleanup operations in `env.close` when a recording changes state.

## Healing

Replay never mutates the sealed scenario. Use the audited correction flow when a
recorded value changes.

```bash
agent-qa heal-respond <sid> --run <failed-run> --step <step> --value <corrected>
agent-qa replay <sid> --heal-from-run <failed-run>
agent-qa heal-promote <sid>
```

## References

- `references/verbs.md` lists the recording and replay commands.
- `references/schema.md` describes `scenario/2`.
- `references/scenario-authoring.md` describes recorded setup.
- `references/replay.md` describes deterministic replay and manual healing.
- `references/unique-tokens.md` describes unique replay values.
