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

When a do-step's role+name locator misses, replay auto-heals: it collects
the live role candidates and walks an ordered strategy ladder
(whitespace → digit-tolerant → digit-anywhere → generated-suffix →
name-prefix), retrying once only when exactly one candidate matches —
ambiguous drift fails the step rather than guessing. A successful heal
appends a `locator-correction` row to `<run>/heal.jsonl` and writes
`<run>/diffs/<stepId>.patch.json`, which `agent-qa heal-promote` can apply
back into the contract. Set `AGENT_QA_NO_HEAL` to disable the loop entirely,
or `AGENT_QA_HEAL_STRICT` to fail any run that needed a heal.

A role+name locator can carry `scope`: an array of locators resolved
outermost-first, each narrowing to a DOM subtree. The role+name match then
runs strictly inside the innermost scope — it never falls back to the whole
document. Use scope to disambiguate repeated controls (the same button label
inside several cards). Scope levels accept raw css/testId/xpath/text
locators or role+name locators. A scope miss fails the step naming the
level that matched nothing. Scoped locators are resolved via DOM evaluation,
so the auto-heal ladder does not engage on them.

`name` may also be `{"i18nKey": "…"}` — resolved through a flat
`i18n.json` map (`{"key": "accessible name"}`) beside `scenario.json`.
Store translated labels there instead of hard-coding display strings in the
contract; a missing file or key fails the step with the available keys.

If replay fails because a captured value changed (a value rejection, not a
locator miss — auto-heal never retries those), use the audited correction
flow. Do not mutate a scenario during replay.

```bash
agent-qa heal-respond <sid> --run <failed-run> --step <step> --value <corrected>
agent-qa replay <sid> --heal-from-run <failed-run>
agent-qa heal-promote <sid>
```
