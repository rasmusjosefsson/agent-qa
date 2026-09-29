# Verbs

## Bootstrap

| Command | Purpose |
| --- | --- |
| `init [dir] [--force]` | Bootstrap a directory for scenario authoring: writes `agent-qa.toml` (`scenarios_root = "./scenarios"`), a `scenarios/hello` smoke scenario, and `.gitignore` entries for run artifacts. |
| `crawl <url> [--session <n>] [--max N] [--sid <id>]` | Draft a coverage scenario from a live page: goto+shot pair per same-origin link. `replay` + `shot-accept` turns it into visual goldens. |

## Recording

| Command | Purpose |
| --- | --- |
| `start "<intent>" [--session <name>] [--profile <name> | --keep-session] [--source-ref <opaque-reference>]` | Starts a recording. Writes typed local recorder state. |
| `record continue <sid> [--session <n>] [--skip-replay]` | Extend an existing scenario: replays it to its end state, seeds the buffer with its steps; `flush` writes the extension back to the same sid. |
| `record pause \| resume \| status` | Freeze/resume capture while you set up page state; append paths drop steps while paused. |
| `buffer list \| edit \| move \| delete \| insert \| load \| check` | Inspect and edit the in-flight buffer; `load <sid>` pulls a saved scenario in, `check` validates as flush would. |
| `flush [--auto-shots]` | Validates and writes `scenario.json`; `--auto-shots` appends a `{"shot"}` visual claim after every do-step. |
| `shot-accept <sid> [--run <id>] [--steps s3,s5]` | Mint/re-mint shot baselines from a run's screenshots — see `visual.md`. |
| `browser <args...>` | Passthrough exec of the pinned `agent-browser` binary. Use for gestures (`open`, `click`, `type`, `snapshot`, ...) instead of a bare `agent-browser` shell command — see `gotchas.md`. |
| `aria-snapshot [--session <name>] [--interactive]` | agent-qa's own read-only ARIA dump verb. Not a `browser` sub-verb — run it as `agent-qa aria-snapshot`, not `agent-qa browser aria-snapshot` (that's `Unknown command`). |
| `cdp-url [--json]` | agent-qa's own verb for the live session's CDP WebSocket endpoint. Also not a `browser` sub-verb. |
| `record-setup '<env-op-json>'` | Appends one schema-valid generic `env.open` operation. |
| `record-step do '<draft-json>'` | Appends a `scenario/2` do draft without `id` or `kind`. |
| `record-step check '<draft-json>'` | Appends a `scenario/2` check draft without `id` or `kind`. |
| `smart-click "<accessible-name>"` | Clicks a target and appends a direct do draft. |
| `fill-unique <label> --template <template>` | Fills a unique value and appends a direct type draft. |
| `smart-fill "<accessible-name>" <value>` | Types a literal value into a field by accessible name and appends a direct `do/type` draft. |
| `verify [--fix]` | Checks the active recording buffer (dense ids + sidecar pairing); `--fix` renumbers ids and moves sidecars in place. |

Only `do` and `check` drafts are accepted.

## Replay

`replay <sid-or-path> [--session <name>] [--profile <name>] [--update-baselines]`
`[--keep-going] [--retry N] [--junit [path]] [--record-video [path]] [--freeze]`
`[--har] [--mock-from <runId>] [--offline] [--base-url <url>] [--auto-promote]`
`[--all [--shard k/n] [--filter <substr>] [--tags <a,b>] [--jobs N] [--report <path>]]`
replays a sealed `scenario/2` document. It writes its audit and sidecars below
`replays/`; `--update-baselines` mints `baselines/` PNGs from the run's
screenshots afterwards (visual baselines — see `visual.md`).`--auto-promote`
writes this run's locator-correction patches back into `scenario.json` when
the run passed — the self-healing loop that keeps a drifting app green
without a `heal-promote` step.

| Command | Purpose |
| --- | --- |
| `replay <sid> --from <stepId> [--until <stepId>]` | Partial replay over a step window (debugging one region). |
| `run-step <do\|check> '<draft-json>' [--session <n>]` | Dispatch one unsealed draft against the live session — probe a step before recording it. |
| `diff <sid> <runA> <runB>` | Diff two runs of a scenario (steps, timings, screenshots). |
| `scenario check <file>` | Schema + lint validation of a scenario doc. |
| `scenario insert <sid> --step '<json>' [--after <stepId> \| --at <index>]` | Splice a validated step into a saved scenario. |
| `scenario extract <sid> [--run <id>] [--through <stepId>] [--to <new>]` | Clone a scenario truncated at the run's first failing step — the repro unit. |
| `scenario coverage <file>` / `scenario coverage-all` | Per-scenario and whole-suite do→check coverage rolls. |
| `scenario lint <file> [--strict]` / `scenario lint-all [--root <dir>]` | Common lints (duplicate ids, bare do, …); `--format github` emits CI annotations. |
| `audit health` | Suite rollup: flaky + slow + heal-chronic step ids per scenario. |
| `audit flaky <sid> [--min-runs N]` / `audit slow <sid>` | Steps whose pass/fail interleaves; steps whose duration regressed. |
| `audit show <sid> <runId\|latest>` / `audit list <sid>` | One run's audit.json; every recorded run. |
| `audit explain <sid> [runId\|latest]` | Failure digest for a run: verdict + failed steps + heals + console errors + bad requests + next commands. Exit mirrors `audit verdict`. |
| `heal-list <sid>` / `heal-chronic <sid>` / `heal-promote <sid> --apply` | Pending suggested patches; steps re-healing every run; absorb patches into `scenario.json`. |
| `doctor` | Environment sanity (binary, browser, config). |
`replay <sid-or-path> [--session <name>] [--profile <name>] [--update-baselines]`
`[--keep-going] [--retry N] [--junit [path]] [--record-video [path]] [--freeze]`
`[--har] [--mock-from <runId>] [--offline] [--base-url <url>] [--auto-promote]`
`[--all [--shard k/n] [--filter <substr>] [--tags <a,b>] [--jobs N] [--report <path>]]`
replays a sealed `scenario/2` document. It writes its audit and sidecars below
`replays/`; `--update-baselines` mints `baselines/` PNGs from the run's
screenshots afterwards (visual baselines — see `visual.md`).


## Connection settings (BYO only — skip this by default)

Only needed when attaching to an external, already-running Chrome. If you did
not start one yourself with `--remote-debugging-port`, do not set these; a
port nothing is listening on makes every command fail with connection-refused.

Use the same resolved external CDP connection for direct browser commands
(`agent-qa browser <args>`, not bare `agent-browser`) and agent-qa commands.

```bash
export AGENT_BROWSER_CDP=9223
export AGENT_BROWSER_PIN_TAB=1
```

You can also set `cdp` and `pin_tab` in `[browser]` in `agent-qa.toml`.
`start` freezes the resolved values in local recorder state. They do not enter
the scenario file.
