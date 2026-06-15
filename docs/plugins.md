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
  `setup-hook`, `heal-strategy`, `discovery-defaults`. (Per-kind payload
  shapes land alongside each verb's port.)
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
