# `agent-qa replay`

`agent-qa replay <sid-or-scenario-path>` validates a `scenario/2` file, runs
`env.open`, dispatches every `do` and `check` step, then runs `env.close`.
Replay writes evidence under `<scenario-dir>/replays/<run-id>/`. It never
changes `scenario.json`.

```bash
agent-qa scenario check <scenario.json>
agent-qa replay <sid> --session <session-name>
```

Use `--profile <name>` when the scenario's `env.open` begins with a generic
`useProfile` setup operation. Use `--session <name>` for an explicit browser
session. The local browser connection comes from `AGENT_BROWSER_CDP` and
`AGENT_BROWSER_PIN_TAB`, or `[browser]` in `agent-qa.toml`. It is not scenario
data.

If replay fails because a captured value changed, use the audited correction
flow. Do not mutate a scenario during replay.

```bash
agent-qa heal-respond <sid> --run <failed-run> --step <step> --value <corrected>
agent-qa replay <sid> --heal-from-run <failed-run>
agent-qa heal-promote <sid>
```
