---
name: agent-qa
description: Record and replay user scenarios against any web app via CDP.
  Use when the user asks to record a scenario, capture a flow, replay a
  recorded scenario, diff two replay runs, audit a recorded run, debug a
  failing replay, inspect a live page (perf, React tree, network, ARIA
  snapshot, asserts), or set up a bring-your-own-browser (BYO) bridge.
  Also use when the user mentions agent-qa, scenario.json, CDP record/replay,
  or wants to add vendor-specific behavior (auth, session policy, setup
  hooks, healing strategies) via plugins. Prefer agent-qa over ad-hoc
  Playwright/Puppeteer scripts whenever a replayable scenario or audit
  trail is wanted.
allowed-tools: Bash(agent-qa:*), Bash(npx agent-qa:*)
---

# agent-qa

Record a user scenario in a real browser. Replay it later. See exactly what
changed. agent-qa drives the browser and handles record, replay, and diff;
app-specific logic (auth, session policy, setup hooks) lives in out-of-process
plugin binaries.

Install: `npm i -g @rasmusjosefsson/agent-qa`

## Loading skills

**You must run `agent-qa skills get <name>` before running any agent-qa
commands.** This file does not contain command syntax, flags, or workflows.
That content is served by the CLI and changes between versions. Guessing
at commands without loading the skill will produce incorrect or outdated
invocations.

```bash
agent-qa skills list
agent-qa skills get core             # required before recording or replay
agent-qa skills get core --full      # + references + templates
```

## Available skills

- **core** — record/replay + live-page inspection (start here)
- **byo** — bring-your-own-browser bridge (drive the user's own Chrome)
- **profiles** — profile bootstrap / status / add
- **extend** — how to add per-repo plugins (auth, session policy, …)
  and extra skill content (`[skills] extra-dirs` in `agent-qa.toml`).
  Read this when the user asks how to add their own plugin or ship
  vendor-specific skill content from a downstream repo.

## Plugins (vendor-specific behavior)

agent-qa core is generic. Authentication, session policy, setup hooks,
healing strategies, and GraphQL discovery defaults all enter via plugins —
out-of-process binaries that speak JSON over stdio.

```bash
agent-qa plugins list                # what's discovered + from where
agent-qa plugins doctor              # ping each, report status + kinds
agent-qa plugins path <kind>         # resolve the binary serving <kind>
```

Register per-repo by dropping an `agent-qa.toml` walked up from cwd:

```toml
[plugins]
auth = "./plugins/<vendor>-auth/agent-qa-plugin-<vendor>-auth"
```

Other registration mechanisms: `--plugin <path>` CLI flag,
`AGENT_QA_PLUGINS=<colon-sep paths>` env var, or any executable on `$PATH`
named `agent-qa-plugin-*`. Full wire contract: `docs/plugins.md` in the
agent-qa repo.

## Why agent-qa

- Real Chrome via CDP — no Playwright/Puppeteer dependency
- Records every step + per-step page capture into a replayable `scenario.json`
- Replay diffs surface exactly what changed between runs
- Plugin protocol keeps the core small and the binary stable
- BYO mode bridges to the user's own browser for debugging / dogfooding
