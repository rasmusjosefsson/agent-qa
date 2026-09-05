# Verbs

## Recording

| Command | Purpose |
| --- | --- |
| `start "<intent>" [--session <name>] [--profile <name> | --keep-session] [--source-ref <opaque-reference>]` | Starts a recording. Writes typed local recorder state. |
| `record-setup '<env-op-json>'` | Appends one schema-valid generic `env.open` operation. |
| `record-step do '<draft-json>'` | Appends a `scenario/2` do draft without `id` or `kind`. |
| `record-step check '<draft-json>'` | Appends a `scenario/2` check draft without `id` or `kind`. |
| `smart-click "<accessible-name>"` | Clicks a target and appends a direct do draft. |
| `fill-unique <label> --template <template>` | Fills a unique value and appends a direct type draft. |
| `flush` | Validates and writes `<sid>/scenario.json`. |
| `verify` | Checks the active recording buffer. |

Only `do` and `check` drafts are accepted.

## Replay

`replay <sid-or-path> [--session <name>] [--profile <name>]` replays a sealed
`scenario/2` document. It writes its audit and sidecars below `replays/`.

## Connection settings

Use the same resolved external CDP connection for direct browser commands and
agent-qa commands.

```bash
export AGENT_BROWSER_CDP=9223
export AGENT_BROWSER_PIN_TAB=1
```

You can also set `cdp` and `pin_tab` in `[browser]` in `agent-qa.toml`.
`start` freezes the resolved values in local recorder state. They do not enter
the scenario file.
