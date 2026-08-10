<img width="2912" height="1464" alt="Gemini_Generated_Image_9pyfu69pyfu69pyf" src="https://github.com/user-attachments/assets/529aa43e-ff10-4c37-98d3-fa83f8d7ac42" />


# agent-qa

Record a user scenario in a real browser. Replay it later. See exactly what
changed.

agent-qa drives the browser and handles record, replay, and diff. App-specific
concerns — authentication, session policy, setup hooks — live in plugins you
add for your target app.

## Install

```bash
npm i -g @rasmusjosefsson/agent-qa
agent-qa --help
```

You will need at least one plugin for auth (e.g. `agent-qa-plugin-<vendor>`)
unless your target app needs no authentication. See
[`docs/plugins.md`](docs/plugins.md).

## Use with AI coding assistants

Install the discovery stub so your agent knows about agent-qa:

```bash
npx skills add rasmusjosefsson/agent-qa
```

This works with Claude Code, Codex, Cursor, Gemini CLI, GitHub Copilot,
Goose, OpenCode, Windsurf, and Pi. It adds a thin `SKILL.md` stub at
`.claude/skills/agent-qa/SKILL.md` (or the equivalent location for your
tool). The stub is intentionally minimal — it points the agent at
`agent-qa skills get core` to load the actual workflow content at runtime,
so instructions always match the installed CLI version instead of going
stale between releases.

Available skills (served by the CLI, embedded in the binary):

- **core** — record/replay + live-page inspection
- **byo** — bring-your-own-browser bridge
- **profiles** — profile bootstrap / status

```bash
agent-qa skills list
agent-qa skills get core           # required before recording
agent-qa skills get core --full    # + references + templates
```

### Adding vendor-specific behavior

Drop an `agent-qa.toml` in your repo to register plugins:

```toml
[plugins]
auth = "./plugins/acme-auth/agent-qa-plugin-acme-auth"
```

Then `agent-qa plugins doctor` to verify. See
[`docs/plugins.md`](docs/plugins.md).

## End-to-end record → replay → compare

```bash
# 1. Begin a recording
agent-qa start "open the homepage and check the URL"

# 2. Drive the page
agent-qa record-step do '{"intent":"land","verb":"goto",
  "value":{"from":"literal","literal":"https://example.com/"}}'
agent-qa fill-unique Email --template 'qa-{{vars._unique}}@example.com'
agent-qa smart-click "Save"
agent-qa record-step check '{"intent":"url is set",
  "claim":{"subject":{"url":true},"predicate":"exists"}}'

# 3. Inspect the buffer + flush to scenario.json
agent-qa verify
agent-qa flush

# 4. Replay later (same scenario, fresh `_unique` token each run)
agent-qa replay <sid>

# 5. Diff two replay runs
agent-qa list <sid>
agent-qa compare <sid>

# 6. After inspecting a failed run, record a string correction and re-execute
agent-qa heal-respond <sid> --run <failed-run-id> --step s1 --value qa-other@example.com
agent-qa replay <sid> --heal-from-run <failed-run-id>
```

## Verbs

The full verb set ships, plus a few inspection verbs.

| Group | Verbs |
| --- | --- |
| Recording | `start`, `record-step`, `fill-unique`, `smart-click`, `truncate`, `flush`, `verify` |
| Replay | `replay` (`--profile`/`--session`/`--param`/`--heal-from-run`/`--dry-run`/`--no-sidecars`/`--runs`/`--quiet`/`--tag`/`--output-audit`), `list` (`--json`/`--filter`/`--limit`), `compare` (alias `diff`), `audit` (`show`/`list`/`stats`/`stats-all`/`diff`/`summary`/`exit-code`/`field`/`count`/`duration`) |
| Heal | `heal-respond`, `heal-promote`, `heal-apply`, `heal-list` (`--mode`/`--applied`/`--unapplied`) |
| Profiles | `profile-add`, `profile-status`, `profile-bootstrap`, `profile-list` |
| Diagnostics | `doctor`, `info`, `config show`, `byo-doctor`, `perf-snapshot` |
| Operational | `version` (`--json`), `skills` (`list`/`get`/`path`, `--json`), `plugins` (`list`/`doctor`/`path`, `--json`), `scenario` (`validate`/`validate-all`/`summary`/`inputs`/`new`/`diff`/`hash`/`id`/`intent`/`step-ids`/`field`/`ls`/`latest`/`count`/`rename`/`copy`/`delete`/`prune-replays`/`prune-all`/`coverage`/`lint`/`lint-all`/`check`/`check-all`) |

The `do` verb dispatcher implements every scenario verb except
`upload`, which is blocked on agent-browser exposing a file-staging
primitive. Control-flow (`group`, `loop`, `useTemplate`) and the
`callGql` http verb are wired through the runner's flatten/dispatch
layer. Run any verb with `--help` for full flags.

## Architecture

```
agent-qa (Rust binary)
  ├── spawns agent-browser   ← drives Chromium via CDP
  └── invokes plugins        ← per-vendor auth / session policy / hooks
```

See [`docs/architecture.md`](docs/architecture.md), the
plugin contract in [`docs/plugins.md`](docs/plugins.md), the
config surface in [`docs/configuration.md`](docs/configuration.md),
the full verb reference in [`docs/verbs.md`](docs/verbs.md), and the
lint rule catalogue in [`docs/lint-rules.md`](docs/lint-rules.md).

## Plugin protocol

Plugins are subprocesses that speak JSON over stdio. agent-qa invokes
the plugin binary as `<plugin> <kind> [<op>]`, writes a JSON request
on stdin, reads a JSON response on stdout. Discovery looks at the
`--plugin <path>` CLI flag, then `agent-qa.toml`, then
`AGENT_QA_PLUGINS` env var, then `$PATH` (any binary named
`agent-qa-plugin-*`). The reference plugin is 10 lines of POSIX shell:
[`examples/plugins/noop-auth/agent-qa-plugin-noop-auth`](examples/plugins/noop-auth/agent-qa-plugin-noop-auth).

## Development

```bash
cd cli
cargo build
cargo test       # 422 tests, zero warnings
```

Single Rust crate in `cli/`. Cross-compile + npm packaging in
[`docs/releasing.md`](docs/releasing.md).

## Repo layout

| Path | What |
| --- | --- |
| `cli/` | Rust crate — produces the `agent-qa` binary |
| `cli/src/` | One module per verb plus shared infrastructure |
| `npm/agent-qa/` | Umbrella npm package (Node launcher + shim) |
| `schema/` | `scenario-schema.json` — the contract |
| `skill-data/` | Embedded agent runbooks (markdown), served by `agent-qa skills get` |
| `skills/` | Pi/Claude-Code discovery stubs (installed via `npx skills add`) |
| `test-fixtures/` | Golden scenario corpus |
| `docs/` | Architecture, plan, plugin author guide |
| `examples/plugins/` | Reference plugins |
