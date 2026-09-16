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

Use one browser session for the full journey.

**Default case — no external browser involved.** Do nothing here. Do not export
`AGENT_BROWSER_CDP` or set `[browser]` in `agent-qa.toml`. `agent-browser` and
`agent-qa start` manage the browser process themselves. Setting `AGENT_BROWSER_CDP`
to a port nothing is listening on makes every subsequent command fail with a
connection-refused error — if you did not deliberately start an external Chrome
with `--remote-debugging-port`, do not set this variable. Go straight to
[Record a replayable scenario](#record-a-replayable-scenario) below.

**Only if the user explicitly asked to attach to their own already-running
browser (BYO)**, set the connection before every direct browser command and
every agent-qa command:

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

`start` also takes `--open <url>`, which navigates the session so you have a
page to work against. It drives the browser only — it records nothing. Neither
an `env.open` nav nor a `do`/`goto` step appears in `scenario.json` because you
passed it, so a scenario started that way and flushed without an explicit first
step replays against whatever page the session happens to be on. Record the
navigation yourself, the same way you would without `--open`: `record-setup`
with a `nav` op for setup, or a `do`/`goto` step when the navigation is part of
the flow under test.

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
agent-qa browser --session qa-run open https://example.com/users
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

Fetch any of these with `agent-qa skills get core --full` (inlines every file
below), or one at a time via `agent-qa skills path core` + read on disk when
running from a repo checkout.

- `references/gotchas.md` — known footguns (env vars, daemon recovery,
  smart-click limits). Read this first if a command result looks wrong.
- `references/verbs.md` lists the recording and replay commands.
- `references/schema.md` describes `scenario/2`.
- `references/scenario-authoring.md` describes recorded setup.
- `references/anatomy.md` — what a recorded `scenario.json` looks like end to end.
- `references/asserts.md` — `record-step check` claim JSON grammar (role/name,
  raw text locator, `isVisible`).
- `references/prep.md` — `env.open`/`env.close` seeded setup and cleanup ops.
- `references/replay.md` describes deterministic replay and manual healing.
- `references/unique-tokens.md` describes unique replay values.
- `references/heal.md`, `references/heal-apply.md`, `references/heal-opt-out.md`
  — manual correction flow and its limits.
- `references/recovery.md` — recording/replay recovery paths.
- `references/inspect.md` — live-page debug mode with no `scenario.json` produced.
- `references/perf-snapshot.md` — opt-in performance sidecar, orthogonal to recording.
- `references/compare.md` — the diff verb (recording vs replay, cross-profile).
- `references/design-review.md` — the design-fidelity lane: reference images at
  `<sid>/designs/<stepId>.png`, verdicts, and when to stop and ask. Separate
  gate from behaviour; judges layout, never text content.
