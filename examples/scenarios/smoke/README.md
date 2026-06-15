# Demo scenario

A minimal `scenario/2` document you can replay against any HTTP page.
Useful as a smoke test after `npm i -g agent-qa`.

## Try it

```bash
# Point AGENT_BROWSER_BIN at your agent-browser install or rely on
# the umbrella npm package's pre-resolved sibling.
agent-qa scenario validate examples/scenarios/smoke/scenario.json

agent-qa replay examples/scenarios/smoke/scenario.json
agent-qa list <sid>
agent-qa compare <sid>
```

The scenario navigates to `https://example.com/`, asserts the URL is set,
and finishes. No form fills, no auth, no plugin required. Replay it
twice and `compare` to see a successful diff outcome.

## What's in here

- `scenario.json` — the sealed contract. Schema-valid; can be replayed
  any number of times.

## Not in here (yet)

A neutral demo app + recorded fixture corpus that exercises every verb
(\`smart-click\`, \`fill-unique\`, heal cycle, etc.) lives on the
roadmap as part of v0.1 polish. For now the smoke scenario above plus
the unit tests under \`cli/src/\` give end-to-end coverage of every
verb's wire format.
