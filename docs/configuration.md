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
