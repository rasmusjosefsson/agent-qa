# Configuration

agent-qa pulls configuration from three places, in priority order:

1. **CLI flags** on the verb itself (`--profile`, `--plugin`, `--param`, …)
2. **Environment variables** (`AGENT_BROWSER_BIN`, `AGENT_QA_SCENARIOS_DIR`,
   `AGENT_QA_RECORD_DIR`, `AGENT_QA_PLUGINS`, …)
3. **`agent-qa.toml`** walked from cwd up to the filesystem root

Everything in this document is optional. agent-qa runs out of the box
with no config file, no env vars, and no plugins (modulo the verbs
that need agent-browser or an auth plugin).

A reference config lives at
[`examples/agent-qa.toml`](../examples/agent-qa.toml). Drop a copy
into your repo root and uncomment what you need.

## `agent-qa.toml`

Two top-level tables:

```toml
[plugins]
auth = "/abs/path/to/agent-qa-plugin-acme-auth"
session-policy = "./tools/session-policy"            # relative to the toml file
setup-hook = "agent-qa-plugin-setup"                 # resolved via $PATH

[paths]
scenarios_root = "./tmp/agent-qa-scenarios"
record_root   = "./tmp/agent-qa-record"
```

### `[plugins]`

Maps a plugin **kind** to a binary. Discovery (in priority order):

1. `--plugin <path>` CLI flag (highest)
2. `[plugins]` table from the active `agent-qa.toml`
3. `AGENT_QA_PLUGINS` env var (colon-separated list of binary paths)
4. `$PATH` — any executable whose filename starts with `agent-qa-plugin-`

The plugin protocol (subprocess + JSON over stdio) is documented in
[`plugins.md`](plugins.md).

### `[paths]`

Override the on-disk roots. Resolution per root:

| Setting          | Env var override          | Default                            |
| ---------------- | ------------------------- | ---------------------------------- |
| `scenarios_root`  | `AGENT_QA_SCENARIOS_DIR`   | `<cwd>/tmp/agent-qa-scenarios`      |
| `record_root`    | `AGENT_QA_RECORD_DIR`     | `<cwd>/tmp/agent-qa-record`        |

Relative paths in `[paths]` resolve against the toml file's
directory; absolute paths pass through.

Env vars always win over the toml. The toml always wins over the
default.

### `[baselines]`

Where shot/domshot goldens live — a sync layer over
`<sid>/baselines/`. `local` (the default) keeps goldens as committed
repo files. `github` and `turso` sync them to a remote store: replay
pulls remote files into the local dir before the step loop
(`--no-baseline-sync` opts out), and `shot-accept`,
`domshot-accept`, and `replay --update-baselines` push after
minting. `agent-qa baselines pull|push|status` syncs manually.

```toml
# Second repo via the contents API — keeps PNGs out of source history.
[baselines]
store = "github"
repo = "org/agent-qa-goldens"     # owner/name
branch = "main"                   # default "main"
prefix = "baselines"              # default "baselines"
token_env = "GOLDENS_TOKEN"       # default: AGENT_QA_GH_TOKEN → GITHUB_TOKEN → GH_TOKEN
```

```toml
# Turso/libSQL — free tier, one table auto-created per db.
[baselines]
store = "turso"
url = "libsql://db-org.turso.io"  # https:// works too
token_env = "TURSO_AUTH_TOKEN"    # default TURSO_AUTH_TOKEN
```

Sync is **additive**: a file missing on one side is never deleted on
the other, and a `.store.json` manifest under `baselines/` tracks
local/remote hashes so only changed files move. The workbench
Settings page edits the same table under "Golden storage".

### `[mail]`

HTTP API of the test inbox the `{"mail": …}` scenario verb and the
`agent-qa mail` CLI read — a mailpit or mailhog server the app under
test sends mail to (OTP codes, verification links).

```toml
[mail]
url = "http://localhost:8025"   # mailpit's API base (mailhog works too)
```

A mail step polls the inbox for the newest message matching
`params.to` / `params.subject` (globs), extracts from the body, and
binds the hit for `saveAs`:

```json
{"id":"s9","kind":"do","verb":"mail",
 "intent":"wait for the verification mail",
 "params":{"to":"*@example.test","subject":"*verify*",
           "extract":"link","timeoutMs":30000},
 "saveAs":"verifyUrl"}
```

`extract`: `"link"` = first `https?://` URL, `"code"` = first 4–8
digit run, anything else = a regex (capture group 1 wins, else the
whole match). Without `extract` the bound value is the whole text
body. `timeoutMs` bounds the poll (default 30000); `to` and
`subject` are optional — no filters matches the newest message
(useful for a per-test mailbox). Both mailpit (`/api/v1`) and
mailhog (`/api/v2`) shapes are detected automatically.

### `[notify]`

Webhook alert sink — after each replay the runner POSTs the verdict
to `url` (Slack-compatible JSON: `{text, sid, runId, verdict,
summary, runDir, tag}`).

```toml
[notify]
url = "https://hooks.slack.com/services/…"   # any JSON webhook
on  = "failure"      # default: only when the gate fails;
                     # "always" posts every run
```

Posting is best-effort — a failed POST warns on stderr and never
changes the exit code. A quarantined failure passes the gate, so
under `on = "failure"` it stays silent; under `"always"` it posts
as `QUAR-FAIL`. Probe the wiring with `agent-qa notify test`.

## Personas and environments

Credentialed replays draw from two record kinds under the scenarios
root (the same files the workbench reads and writes):

```
<scenarios>/_personas/<id>/persona.json
<scenarios>/_environments/<id>/environment.json
```

```jsonc
// persona.json — an identity: which profile, which credentials
{ "schema": "persona/1", "profile": "admin", "default": true,
  "credentials": { "entries": {
    "ADMIN_USER": "admin@example.com",
    "ADMIN_PASS": "vault:kv/qa/admin:password" } } }

// environment.json — a target: base URL, params, how auth happens
{ "schema": "environment/1", "default": true,
  "baseUrl": "https://staging.example.com",
  "params": { "tenant": "acme" },
  "auth": { "plugin": "acme-auth", "loginUrl": "/login",
            "config": { "realm": "staff" },
            "creds": { "ADMIN_PASS": "vault:kv/qa/admin:password" } } }
```

`replay --persona <id>` loads the persona: its `credentials.entries`
land in the process env (literal values pass through; a `<scheme>:<ref>`
provider ref — e.g. `vault:kv/qa/admin:password` — is delegated to a
discovered `credentials` plugin; see `docs/plugins.md`. A value that is
a literal containing `:` can be forced with `literal:<v>`) and the persona's
`profile` becomes the run's profile (`--session` stays
`<profile>-session`, overridable as usual) so an
`env.open useProfile` op replays through that profile's auth
session.

`replay --environment <id>` (alias `--env`) layers on the
environment: `params` + `baseUrl` merge into the scenario's inputs
**under** any `--param`/`input_overrides`, `baseUrl` also lands on the
`baseUrl` input when the scenario declares it, `auth.config` lands
as `AGENT_QA_ENV_<KEY>` env vars, `auth.loginUrl` as
`AGENT_QA_ENV_LOGIN_URL`, and `auth.creds` merge under the persona's
credentials. With `--persona` and no `--environment` the
`default:true` (or sole) environment is picked up automatically —
`--environment <id>` pins one explicitly.

## Discovery

`agent-qa.toml` is found by walking from `cwd` up to the filesystem
root, looking for either `agent-qa.toml` or `.agent-qa.toml`. The
first match wins. There is no way to point at a specific config file
from the CLI today — set the cwd accordingly.

`agent-qa config show` prints the resolved values:

```
$ agent-qa config show
agent-qa.toml: /home/me/work/agent-qa.toml
scenarios_root: /home/me/work/tmp/agent-qa-scenarios
record_root:   /home/me/work/tmp/agent-qa-record

plugins (2):
  /usr/local/bin/agent-qa-plugin-acme-auth  [auth]  source=ConfigFile(...)
  /usr/local/bin/agent-qa-plugin-setup    [setup-hook]  source=ConfigFile(...)
```

`agent-qa doctor` runs the same resolution **plus** probes
agent-browser and pings each plugin.

## Environment variables

| Variable                   | Effect                                                              |
| -------------------------- | ------------------------------------------------------------------- |
| `AGENT_BROWSER_BIN`        | Absolute path to the agent-browser binary (set by the npm shim)     |
| `AGENT_QA_SCENARIOS_DIR`    | Override the scenarios root (relative paths resolve against `cwd`)   |
| `AGENT_QA_RECORD_DIR`      | Override the recorder workfile root                                 |
| `AGENT_QA_PLUGINS`         | Colon-separated list of plugin binary paths                          |
| `AGENT_QA_NO_AUTO_RECOVER` | `1` disables agent-browser orphan-daemon auto-retry (debug only)    |
| `AGENT_QA_NO_HEAL`         | Set disables the replay auto-heal loop entirely (CI fail-hard)      |
| `AGENT_QA_HEAL_STRICT`     | Set fails a run that needed any heal, even though every step passed |
| `AGENT_QA_CHAT_BACKEND`    | Workbench chat agent runtime: `pi` (default) or `opencode`          |
| `AGENT_QA_PI_SDK`          | Explicit path to the pi SDK (`@earendil-works/pi-coding-agent`)     |
| `AGENT_QA_OPENCODE_SDK`    | Explicit path to the opencode SDK (`@opencode-ai/sdk`, v2 surface)  |
| `AGENT_QA_NO_CHAT`         | `1` disables the in-app Chat tab entirely                           |

Credential provider variables (`VAULT_ADDR`, `OP_…`, `AWS_…`) belong to
the `credentials` plugin you install, not to agent-qa — see its README.

The `opencode` backend needs the `opencode` CLI on `PATH` (`npm i -g
opencode-ai`) — each chat spawns its own `opencode serve` process with that
chat's per-session env, while `pi` runs in-process via its SDK.
