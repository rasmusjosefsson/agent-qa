# `agent-qa replay`

Active CLI for deterministic replay. Reads a scenario JSON, walks every step against a live agent-browser session, substitutes templates where supported, writes replay sidecars/manifest under `<scenarioDir>/replays/<replayId>`, and halts at the first hard failure.

## When To Use

- You have a scenario/v1 JSON and want to re-execute it against a live target app.
- You want fresh replay artifacts for compare/diff.
- You want a single-profile smoke before running `compare --profiles ...`.

## Usage

```bash
agent-qa replay [<scenarioId> | <path-to-scenario.json>] [--profile <p>] [--session <name>] [--param <name>=<value> ...]
```

- `<scenarioId>` resolves to `<scenariosRoot>/<sid>/scenario.json`.
- `<path-to-scenario.json>` can point anywhere on disk.
- `--profile <p>` bootstraps the profile and uses its session.
- `--session <name>` bypasses profile resolution and drives that agent-browser session.
- `--param <name>=<value>` (alias `-p`) overrides a scenario-declared parameter at replay time. Repeatable. See [Overriding parameters](#overriding-parameters).

Unsupported flags are intentionally not documented here. Use `compare --profiles ...` for multi-profile analysis.

## Overriding parameters

Scenarios declare a top-level `inputs: Record<string, { type, default?, sensitive? }>` map (per `scenario-schema.json` §`InputDecl`). `agent-qa replay` seeds `ctx.inputs` from declared defaults, then applies any `--param <name>=<value>` flags on top. Use it to drive the same recorded scenario across emails, counts, feature toggles, or per-environment secrets without editing `scenario.json`.

```bash
# String override:
agent-qa replay <sid> --profile default-user --param userEmail=alice@example.com

# Number coercion (declared input.type === 'number'):
agent-qa replay <sid> --profile default-user -p count=12

# Sensitive override — value flows into inputs, redacted in audit.json:
agent-qa replay <sid> --profile default-user --param apiToken=EXAMPLE-TOKEN-1234
```

Rules:

- **Repeatable.** `--param a=1 --param b=2` is the canonical shape; the short alias `-p` is identical.
- **Split on first `=` only.** `--param query="a=b&c=d"` parses to key=`query`, value=`a=b&c=d`. A `--param` without `=` is exit 2.
- **Validated against the declared schema.** Unknown name → exit 2 (stderr lists the declared inputs so you spot typos). Type mismatch → exit 2 (`expected number, got 'hello'`). Both failures happen BEFORE bootstrap, so an unrunnable invocation pays no OAuth round-trip.
- **Coerced per `input.type`.** `string` returns the raw text; `number` uses `Number()` and rejects `NaN` / `Infinity` / non-numeric strings via `Number.isFinite`; `boolean` accepts `true|false|1|0|yes|no` (case-insensitive); `array` parses the value as JSON and rejects non-arrays; `object` parses as JSON and rejects non-objects (null/array/scalar).
- **Sensitive inputs are redacted in `audit.json`.** When `inputs[name].sensitive: true`:
  - `replays/<runId>/audit.json.parameters[].value` records the literal string `[REDACTED]`.
  - Type-mismatch stderr says `expected number, got [REDACTED]` instead of the raw value.
  - The substituted output (what the live page receives) is unchanged — the real value flows through `ctx.inputs` to the runtime scope. Only the persisted audit trail is scrubbed.
- **Duplicate `--param foo=...` is last-write-wins.** `--param foo=a --param foo=b` resolves to `b`; only the final value lands in `audit.json.parameters[]` as a single row. This matches Helm `--set` / another-tool `--env` semantics.

Audit shape in `replays/<runId>/audit.json` (`parameters[]` always present when the scenario declares any `inputs`; rows present for every input that received a value via default or cli):

```json
{
  "parameters": [
    { "name": "userEmail", "source": "cli", "type": "string", "sensitive": false, "value": "alice@example.com" },
    { "name": "count", "source": "default", "type": "number", "sensitive": false, "value": 1 },
    { "name": "apiToken", "source": "cli", "type": "string", "sensitive": true, "value": "[REDACTED]" }
  ]
}
```

## Scenario/V1 Contract

The root shape is:

```json
{
  "scenarioSchema": "scenario/v1",
  "id": "create-user",
  "intent": "create a user",
  "setup": {},
  "teardown": {},
  "steps": [{ "stepId": "open-users", "type": "goto", "url": "/users" }]
}
```

Every step has a stable `stepId`. Replay status, evidence, heal, and diff code prefer `stepId` before numeric index so inserted/deleted steps do not shift diagnostics onto the wrong row.

V1 step discriminators include navigation (`goto`, `reload`, `back`, `forward`), interaction (`click`, `type`, `clear`, `press`, `select`, etc.), assertions (`assertVisible`, `assertText`, `assertUrl`, etc.), network/GQL (`waitForNetwork`, `assertNetwork`, `gql`), dialogs/downloads, reads, and control flow (`if`, `forEach`, `while`, `group`, `runTemplate`).

## Setup / Teardown

Root `setup` runs before step 0. Root `teardown` runs after the scenario according to its policy. Supported setup/teardown channels are `nav`, `cookies`, `localStorage`, `gql`, `featureFlags`, and dialog policy where applicable.

GQL setup/teardown uses the raw GQL channel. Feature flags require a feature-flag adapter. Profile runs are serial; cross-profile `runAs` inside one serial run is rejected.

## Evidence Layout

The internal scenario/v1 runner/evidence helpers use stepId-keyed evidence paths:

```text
<scenarioDir>/replays/<replayId>/evidence/<channel>/<stepId>.*
```

Common channels are `snapshot` and `screenshot`:

```text
replays/<replayId>/evidence/snapshot/open-users.txt
replays/<replayId>/evidence/screenshot/open-users.png
```

The public `agent-qa replay` CLI is not yet fully wired to this layout. Do not assume `agent-qa replay` writes `evidence/<channel>/<stepId>.*` until the replay implementation is cut over. Existing replay sidecars may still use legacy snapshot/screenshot/keyframe locations under `replays/<replayId>/`.

## Templates

Reusable v1 templates live under:

```text
<scenarioDir>/templates/<name>.json
```

`runTemplate` loads by safe template name; `.json` is appended when omitted.

## Exit Codes

| Code  | Meaning                                                                                                                                        |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `0`   | every step passed                                                                                                                              |
| `1`   | a step failed; or strict-mode heal-drift detected (suggested-diff written)                                                                     |
| `2`   | bad arguments, missing scenario, parse error, or schema validation failure                                                                      |
| `7`   | caller-driven heal pending — a value-rejection step needs `heal-respond` + `replay --heal-from-run` to retry (only when `AGENT_QA_HEAL_LLM=1`) |
| `127` | agent-browser preflight failed                                                                                                                 |

## Heal at replay time

When a step fails, the runner invokes the scenario/2 heal pipeline before aborting. Rule-based locator strategies (whitespace / Unicode-punctuation collapse, digit-suffix tolerance) attempt to correct the locator against the live snapshot. On match, the runner retries the step once with the override and writes a suggested-diff file at `<scenarioDir>/replays/<runId>/diffs/<stepId>.patch.json` for operator review via `agent-qa heal-promote`. Value-rejection misses (recent GraphQL errors[]) trigger the caller-driven path when `AGENT_QA_HEAL_LLM=1` — see [`heal.md`](./heal.md) for the strategy registry, disposition table, the `heal-promote` round-trip, and the `heal-respond` + `--heal-from-run` caller-driven protocol.

### Heal env vars

| Env                    | Default                          | Effect                                                                                                                                                                                                                                                                                   |
| ---------------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AGENT_QA_HEAL_STRICT` | `1` iff `CI` is truthy; else `0` | When `1`, a `matched-drift-*` disposition forces non-zero exit even when every step passed. The suggested-diff still lands; only the exit code changes. Interactive recorder / ad-hoc replay default off; CI default on.                                                                 |
| `AGENT_QA_HEAL_LLM`    | `0`                              | When `1`, value-rejection misses with no rule-based match write a request file to `replays/<runId>/heal-requests/<stepId>.json` and exit 7 — the calling agent reads the request, runs `agent-qa heal-respond`, and re-invokes `replay --heal-from-run <runId>` to apply the correction. |

Surfaced at runner start: `[v2-replay] heal: strict=on|off`.

### --heal-from-run

```
agent-qa replay <sid> --heal-from-run <runId>
```

Loads `replays/<runId>/heal-responses/<stepId>.json` files emitted by a prior `heal-respond` round-trip and applies them as per-step value overrides. For `do` verbs with `step.value`, the override replaces the value. For `callGql`, an object override merges into `step.params.variables`. Each overridden step is annotated `[heal-override applied]` in stdout. Combine with the original CLI args (profile, params, etc.) — `--heal-from-run` is purely additive.

## Related Commands

```bash
agent-qa compare <sid>                                  # unified 1:1 or N-way diff
agent-qa diff <sid>                                     # alias of compare
agent-qa list <sid>                                     # inspect steps/replays/compare runs
agent-qa heal-promote <sid> [--apply]                   # promote replay-side suggested-diffs into scenario.json
agent-qa heal-respond <sid> --step <id> --value <X>     # caller-driven response to a value-rejection request
```
