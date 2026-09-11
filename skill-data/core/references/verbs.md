# Verbs

## Recording

| Command | Purpose |
| --- | --- |
| `start "<intent>" [--session <name>] [--profile <name> | --keep-session] [--source-ref <opaque-reference>]` | Starts a recording. Writes typed local recorder state. |
| `browser <args...>` | Passthrough exec of the pinned `agent-browser` binary. Use for gestures (`open`, `click`, `type`, `snapshot`, ...) instead of a bare `agent-browser` shell command — see `gotchas.md`. |
| `aria-snapshot [--session <name>] [--interactive]` | agent-qa's own read-only ARIA dump verb. Not a `browser` sub-verb — run it as `agent-qa aria-snapshot`, not `agent-qa browser aria-snapshot` (that's `Unknown command`). |
| `cdp-url [--json]` | agent-qa's own verb for the live session's CDP WebSocket endpoint. Also not a `browser` sub-verb. |
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

## Connection settings (BYO only — skip this by default)

Only needed when attaching to an external, already-running Chrome. If you did
not start one yourself with `--remote-debugging-port`, do not set these; a
port nothing is listening on makes every command fail with connection-refused.

Use the same resolved external CDP connection for direct browser commands
(`agent-qa browser <args>`, not bare `agent-browser`) and agent-qa commands.

```bash
export AGENT_BROWSER_CDP=9223
export AGENT_BROWSER_PIN_TAB=1
```

You can also set `cdp` and `pin_tab` in `[browser]` in `agent-qa.toml`.
`start` freezes the resolved values in local recorder state. They do not enter
the scenario file.
