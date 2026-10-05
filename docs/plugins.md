# Plugin author guide

Every app-specific concern — auth, session policy, setup hooks, GraphQL
discovery defaults — enters via a **plugin**. This guide is the contract for
writing one.

This document is the wire contract.

## Model

Plugins are out-of-process subprocesses that speak JSON over stdio. agent-qa
spawns the plugin binary with positional args, writes a JSON request on
stdin, and reads a single JSON response from stdout.

```
agent-qa  ──spawn──▶  <plugin-binary> <kind> [<op>]
   │                       │
   │  ──stdin JSON──▶      │
   │  ◀──stdout JSON──     │
```

Plugin authors can use any language that can read stdin, write stdout, and
parse JSON. A 10-line shell script is enough to be a valid plugin (see
`examples/plugins/noop-auth/`).

## Invocation

```
<plugin-binary> <kind> [<op>]
```

- `<kind>` — the extension point. The universal `ping` kind must be
  implemented by every plugin. Other kinds: `auth`, `session-policy`,
  `setup-hook`, `heal-strategy`, `discovery-defaults`, `resolve`,
  `triage`.
  (Per-kind payload shapes land alongside each verb's port.)
- `<op>` — optional sub-operation for kinds that have multiple verbs (e.g.
  `auth probe` vs `auth login`). Currently no kind uses `<op>`; reserved for
  future use.

## Request envelope (stdin)

```json
{
  "protocolVersion": 1,
  "request": { /* kind-specific payload */ }
}
```

## Response envelope (stdout)

Exactly one of `response` or `error` is set:

```json
{ "ok": true,  "response": { /* kind-specific */ } }
{ "ok": false, "error": { "code": "...", "message": "..." } }
```

Process exit status MUST be `0` on `ok: true` and SHOULD be `0` on
`ok: false` too (the JSON `error` is how we surface protocol-level
failures). Non-zero exit is treated as a plugin crash and surfaced with
stderr to the user.

## The `ping` kind

Every plugin MUST handle `ping`. Used by `agent-qa plugins doctor` to
verify a plugin is alive and to learn which kinds it serves.

Request payload: `{}`

Response payload:

```json
{
  "protocolVersion": 1,
  "name": "noop-auth",
  "kinds": ["auth"]
}
```

A plugin that speaks a protocol version higher than the host's MUST refuse
with `error.code = "protocol-version"`.

Flip side: the host (this build of agent-qa) refuses any plugin whose
`ping` returns a `protocolVersion` higher than agent-qa's own
`PROTOCOL_VERSION` (currently `1`). The user sees a clear error
pointing at upgrading agent-qa or downgrading the plugin.

## The `resolve` kind

Optional **authoring-time** element resolution. When `smart-click` /
`smart-fill` (or `agent-qa resolve`) miss every deterministic locator
strategy, the page's interactive elements are lifted from the ARIA
snapshot and handed to the plugin with the author's description.

Request payload:

```json
{
  "description": "the save button",
  "role": "button",
  "candidates": [
    {"ref": "e13", "role": "button", "name": "Save draft",
     "line": "button \"Save draft\" [disabled, ref=e13]"}
  ]
}
```

- `role` — the caller's preferred role (null when unspecified).
- `candidates` — interactive snapshot nodes (preferred role first, capped
  at 100). `ref` is the only handle guaranteed to hit that exact node;
  `line` carries state flags (`[disabled]`, `[checked=true]`) as context.

Response payload:

```json
{"ref": "e13", "confidence": 0.83}
{"ref": null}
```

`ref` MUST echo one of the request's candidate refs — unknown refs are
treated as no pick. `confidence` is optional and informational only; the
plugin applies its own threshold and returns `{"ref": null}` below it.

Batch form — speculative fan-out, one plugin call for N questions:

```json
{
  "descriptions": [
    {"id": "q0", "description": "the save button", "role": null},
    {"id": "q1", "description": "the code box", "role": "textbox"}
  ],
  "candidates": [ ...shared list... ]
}
→ {"answers": {"q0": {"ref": "e5", "confidence": 0.9},
               "q1": {"ref": null, "confidence": 0.1}}}
```

`agent-qa resolve --batch "desc a" "desc b" ...` uses this shape — one
snapshot enumeration + one API round-trip for every question (~3x faster
than sequential on a live page; Jev fans the questions dict out in one
System One call). A plugin that doesn't understand `descriptions` should
answer each id as a no-pick; single-question callers are unaffected.

Resolution never changes what lands in the scenario: the recorded step
keeps the picked element's concrete role+name locator, so a scenario
authored with a resolver replays identically without one. With no
`resolve` plugin configured the fallback rung is skipped entirely.

One opt-in exception at replay: **auto-heal**. When a recorded step's
locator misses AND the deterministic strategy ladder finds no unique
candidate, the plugin gets a last rung — it sees the live candidates of
the step's role and may pick the element the recorded name meant. A
plugin pick heals like any strategy (`strategy: "plugin-resolve"` in
`heal.jsonl`, plus the `heal-promote` patch), is retried once, and is
disabled together with the rest of auto-heal under `AGENT_QA_NO_HEAL`.
A plugin that errors is treated as a no-pick, so a broken resolver can
never wedge or fail a replay.

A reference implementation lives in `examples/plugins/jev-resolve/` — it
answers via Jev (typesafe.ai)'s System One decision API out of the box,
and adapts to any compatible choice-question endpoint via env config.

### Enable in the workbench

The `jev-resolve` binary ships inside the `agent-qa` npm package, so the
workbench can wire it up with zero config editing:

1. Open **Extensions** in the workbench (`agent-qa web` → `/plugins`).
2. Under **Element resolution → Jev (typesafe.ai)**, paste your API key
   (console.typesafe.ai → API keys) and click **Save & enable**.

The server persists `{enabled, apiKey}` to `<root>/_config/jev.json`
(mode 0600) and injects the bundled binary into `AGENT_QA_PLUGINS` +
`TYPESAFE_API_KEY` for every CLI call it spawns — chat agents, the
editor, and replay all pick it up. The API key is write-only over HTTP
(the API reports `hasKey`, never the value).

Any `TYPESAFE_API_KEY` already in the environment **wins** over the
stored key, so CI and terminals keep working as before. Outside the
workbench the CLI path stays the canonical way:

```toml
# agent-qa.toml
[plugins]
resolve = "/path/to/agent-qa-plugin-jev-resolve"   # or a name resolved via $PATH
```

```bash
export TYPESAFE_API_KEY="…"   # shell, CI secret, or vault — never committed
```

The key can also come straight from GitHub (Actions secret → env var) —
nothing Jev-related ever needs to be committed or stored in git.

### Measured cost + accuracy

Battery over a live Jev `jev-latest` endpoint (local snapshot
enumeration + HTTP round-trip included):

| metric | measured |
| --- | --- |
| resolution latency | ~250–300ms per call (~40ms snapshot + ~130ms API + spawn) |
| smart-click resolved vs failed miss | ~2.9s vs ~1.9s — the plugin adds ~0.3s but turns a dead end into a pass |
| accuracy on fuzzy descriptions | 15/17 correct at conf 0.85–1.0 on pages with 30+ controls |
| nonsense / ambiguous | clean `{ref: null}` no-pick, no guessing |

What that buys an author: a description that missed every deterministic
locator previously cost a human retry loop (read candidates, guess an
exact name, re-run — tens of seconds per element); with a resolver it's a
one-shot ~3s pick. Jev's Choice primitive handles hundreds of options
per call, so the 100-candidate cap bounds payload, not the model.

## The `triage` kind

Optional **post-run** drift triage. `agent-qa triage <sid> [<runId>]`
assembles the run's evidence — `audit.json` (exitCode, summary,
autoHealed, quarantined), the failed rows from `events.jsonl`,
`heal.jsonl` rows, and the diff artifact paths — into one payload and
hands it to the plugin. The plugin answers with a triage summary plus
optional issues; agent-qa writes `triage.md` + `triage.json` into the
run dir and prints the summary. Intended use: a decision model that
reads the run and says "drift — promote these heals" vs "real
regression — file this".

Request payload:

```json
{
  "scenarioId": "checkout",
  "runId": "2026-10-05T06-17-48-630Z__4b65d4df",
  "exitCode": 1,
  "summary": "SUMMARY: 8/9 (FAIL)",
  "quarantined": false,
  "autoHealed": ["s4"],
  "failures": [
    {"id": "s9", "intent": "check home matches golden", "kind": "check",
     "error": "shot 's1' differs …"}
  ],
  "heals": [{"stepId": "s4", "strategy": "plugin-resolve"}],
  "diffs": ["shots-diff/s1.diff.png"],
  "runDir": "/abs/path/to/scenarios/checkout/replays/<runId>"
}
```

`failures` and `heals` are truncated to the last 50 rows each — the
plugin can always read the full files from `runDir`.

Response:

```json
{
  "summary": "Locator drift only — promote s4, s7; s9 is a real regression.",
  "issues": [
    {"title": "Shot golden drifted", "detail": "s1 diff is a hero-banner rebrand"}
  ]
}
```

`summary` is required text; `issues` is an optional list of `{title,
detail}`. With no `triage` plugin configured the verb errors with setup
instructions.

## Discovery

agent-qa locates plugins in this priority order (first match wins per
binary; duplicates de-duplicated by canonical path):

1. `--plugin <path>` CLI flag (may be repeated; absolute or relative to cwd).
2. `agent-qa.toml` walked from cwd up to root:
   ```toml
   [plugins]
   auth = "/abs/path/to/binary"               # absolute path
   session-policy = "./tools/my-policy"       # relative to the toml file
   setup-hook = "agent-qa-plugin-acme"        # resolved via $PATH

   # Optional: override the scenarios + record roots. Env vars
   # (AGENT_QA_SCENARIOS_DIR / AGENT_QA_RECORD_DIR) still win.
   [paths]
   scenarios_root = "./tmp/scenarios"
   record_root = "./tmp/record"
   ```
3. `AGENT_QA_PLUGINS` env var (colon-separated list of binary paths).
4. `$PATH` — any executable whose filename starts with `agent-qa-plugin-`
   (mirrors the `gh` extension convention).

The `agent-qa.toml` `[plugins]` table is the recommended shape for project
repos; the env var and `$PATH` mechanisms are for global installs and dev.

## Credential preparation

An extension-provided environment may optionally declare an auth remediation
command. This is for a credential provider that needs an interactive session
before its credential references can resolve:

```json
{
  "auth": {
    "remediation": {
      "label": "Sign in to credentials provider",
      "argv": ["credential-login", "--browser"],
      "automatic": true
    }
  }
}
```

`argv` is executed directly, never through a shell. It is only accepted from a
trusted installed environment record and is never returned to the browser. With
`automatic: true`, a new chat runs it after an initial sign-in failure, then
retries the normal auth flow. Otherwise, the chat shows the label as an explicit
user action. Keep provider names, commands, and authentication details in the
downstream extension—not in agent-qa.

One built-in fallback exists: when an environment or persona uses `vault:`
credential refs that can't resolve, and no remediation is declared, the
workbench offers `vault login -method=oidc` itself (only when `VAULT_ADDR`
is set and a `vault` CLI is on `PATH`). A declared remediation always wins.

## Surface verbs

| Verb                                    | What                                            |
| --------------------------------------- | ----------------------------------------------- |
| `agent-qa plugins list`                 | Enumerate discovered plugins + their kinds.    |
| `agent-qa plugins doctor`               | Ping every plugin and report status.            |
| `agent-qa plugins path <kind>`          | Print the binary path serving `<kind>`.         |
| `agent-qa --plugin <path> plugins …`    | Inject an extra plugin (highest priority).      |

## Minimal example: noop-auth

A complete reference plugin in 10 lines of POSIX shell. See
[`examples/plugins/noop-auth/agent-qa-plugin-noop-auth`](../examples/plugins/noop-auth/agent-qa-plugin-noop-auth).

```sh
#!/bin/sh
KIND="${1:-}"
case "$KIND" in
  ping) echo '{"ok":true,"response":{"protocolVersion":1,"name":"noop-auth","kinds":["auth"]}}' ;;
  auth) cat >/dev/null
        echo '{"ok":true,"response":{"status":"authenticated","note":"noop"}}' ;;
  *)    echo "{\"ok\":false,\"error\":{\"code\":\"unsupported-kind\",\"message\":\"noop-auth does not handle ${KIND}\"}}" ;;
esac
```

## Versioning

The current protocol version is `1`. Bumps will be announced via release
notes and an upgrade section in this document. Plugins are expected to
implement at least the version they shipped against and one prior; the host
refuses to invoke plugins that speak a future version.
