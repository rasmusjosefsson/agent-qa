# agent-qa verb reference

The full set of CLI verbs at a glance. Every verb also responds to
`--help` for inline usage and flag detail.

## Recording

| Verb | What it does |
| --- | --- |
| `start` | Mint a new scenario directory + skeleton `scenario.json` |
| `record-step` | Append one step to the in-flight scenario via the recorder |
| `run-step <kind> <payload>` | Dispatch ONE trigger payload against the live session for author-time feedback, without recording. Same payload shapes as `record-step`; prints a `{ok,…}` JSON line. `--session`. |
| `aria-snapshot` | Dump the live page's accessibility tree as structured picker rows (a thin adapter over `agent-browser snapshot`). Flags: `--interactive`, `--session`. |
| `cdp-url [--session] [--json]` | Print the live session's CDP WebSocket endpoint. Powers the editor's inline live-browser pane (screencast + click-to-record). Read-only. |
| `buffer list \| delete <i> \| move <from> <to> \| clear` | Inspect / reorder / delete rows in the in-flight buffer; delete + move re-index `s0,s1,…` so `flush` stays clean. `list --json`. |
| `fill-unique` | Locator-uniqueness helper for `type`/`fill` style do steps |
| `smart-click` | High-level click that resolves a label to a unique locator |
| `truncate` | Drop the trailing N steps from the in-flight scenario |
| `flush` | Persist the recorder buffer to `scenario.json` |
| `verify` | Cross-check the on-disk scenario against recorder sidecars |

> `run-step`, `aria-snapshot`, and `buffer` are the primitives the local
> **authoring editor** (`agent-qa web` → the *Editor* tab) shells
> to build a scenario by targeting the UI. The editor is hosted by the
> Node launcher; the Rust CLI still owns every record/run/flush mutation.

## Replay

| Verb | What it does |
| --- | --- |
| `replay <sid \| path>` | Run a scenario. Flags: `--profile`, `--session`, `--param name=value`, `--heal-from-run <runId>`, `--dry-run`, `--no-sidecars`, `--runs <N>`, `--quiet`/`-q`, `--tag <label>`, `--output-audit <path>` |
| `list` | Enumerate scenarios (root mode) or one scenario's replays. Flags: `--json`, `--filter <substr>`, `--limit <N>` |
| `compare <a> <b>` | Diff two replay run directories. Alias `diff`. |
| `audit show <sid> <runId \| latest>` | Pretty-print one replay's audit.json. `--json` for raw. |
| `audit list <sid>` | Table of every run (incl. \`dur(s)\` column). Flags: `--json`, `--passed` / `--failed`, `--tag <substr>`, `--profile <substr>`, `--limit <N>`, `--slow <secs>`, `--sort duration\|runId-desc`, `--since <iso-ts>`, `--until <iso-ts>` |
| `audit stats <sid>` | Pass/fail/tag rollup for one scenario, incl. avg duration. Flags: `--json`, `--since`, `--until`. |
| `audit stats-all` | Per-scenario + overall pass/fail rollup across the root, incl. avg duration. Flags: `--json`, `--since`, `--until`. |
| `audit count <sid>` | Number of runs under \`<sid>\` (one integer line). |
| `audit duration <sid> <runId \| latest>` | Run duration in seconds (3 decimals). |
| `audit summary <sid> <runId \| latest>` | Print just the audit.summary string |
| `audit exit-code <sid> <runId \| latest>` | Print just the run's exitCode (-1 if absent) |
| `audit field <sid> <runId \| latest> <name>` | Print any top-level audit field (scalars verbatim; object/array as compact JSON) |
| `audit diff <sid> <runIdA> <runIdB>` | Unified diff between two replays' audit.json. `latest` accepted for either side. |

## Heal

| Verb | What it does |
| --- | --- |
| `heal-respond` | Record an authoring decision against a failed step |
| `heal-promote <sid>` | Apply replay-side patches into `scenario.json` (rebase-guarded) |
| `heal-apply <sid>` | Mark a heal-response as consumed |
| `heal-list <sid>` | List heal-responses. Flags: `--run <runId>`, `--mode value-correction\|reject`, `--applied`, `--unapplied`, `--json` |

## Profiles

| Verb | What it does |
| --- | --- |
| `profile-add <id>` | Register a new profile under the profiles root |
| `profile-status <id>` | Probe a registered profile via the `auth` plugin |
| `profile-bootstrap <id> [--session <name>] [--headed\|--headless]` | Sign in a registered profile through its auth plugin; defaults to headless |
| `profile-list` | Enumerate registered profiles. `--json` for structured. |

## Diagnostics

| Verb | What it does |
| --- | --- |
| `doctor` | Probe local install: agent-browser, plugins, paths. `--json`. |
| `info` | Version + paths + scenario/profile counts (no external probes). `--json`. |
| `byo-doctor` | Read-only BYO browser enumeration via agent-browser. `--json`. |
| `perf-snapshot` | One-shot perf trace via agent-browser, persisted under `<sid>/perf/` |
| `config show` | Resolve the active `agent-qa.toml` + paths + plugin discovery |

## Operational

| Verb | What it does |
| --- | --- |
| `skills list \| get <name> \| path [name]` | Serve embedded agent runbooks. `list --json`. |
| `plugins list \| doctor \| path <kind>` | Manage plugin discovery. `list --json`, `doctor --json`, `--plugin <path>` overrides. |
| `scenario validate <file>` | Schema-validate one scenario. Flags: `--json`, `--format text\|json\|github`. |
| `scenario validate-all` | Schema-validate every scenario under the root. Flags: `--json`, `--format text\|json\|github`. |
| `scenario summary <file>` | Per-step summary. Flags: `--filter <substr>`, `--json`. |
| `scenario inputs <file>` | List declared inputs. `--json`. |
| `scenario new <file>` | Scaffold a minimal valid scenario. Flags: `--force`, `--url`, `--intent`. |
| `scenario diff <a> <b>` | Unified diff between two scenario JSONs |
| `scenario hash <file>` | SHA-256 of scenario bytes (rebase-guard hash) |
| `scenario id <file>` | Print the scenario's id field on one line |
| `scenario intent <file>` | Print the scenario's intent field on one line |
| `scenario step-ids <file>` | Print every step id on its own line |
| `scenario field <file> <name>` | Print any top-level scenario field (scalars verbatim; object/array as compact JSON) |
| `scenario rename <sid> <new>` | Rename a scenario directory + id field |
| `scenario copy <sid> <new>` | Copy a scenario (replays not copied) |
| `scenario delete <sid>` | Remove a scenario directory. `--yes` / `-y` confirms; otherwise dry-run. |
| `scenario prune-replays <sid> --keep N` | Keep most recent N replays. `--yes` / `-y` confirms. |
| `scenario prune-all --keep N` | Same across every scenario. `--yes` / `-y` confirms. |
| `scenario coverage <file>` | Per-step check coverage ratio. `--json`. |
| `scenario lint <file>` | Common-smell linter. Flags: `--json`, `--format text\|json\|github`, `--strict`, `--rule <code>` (repeatable), `--exclude-rule <code>` (repeatable), `--list-rules`. |
| `scenario lint-all` | Same across every scenario under the root. Flags: `--json`, `--format`, `--strict`, `--rule`, `--exclude-rule`. |
| `scenario check <file>` | Schema validate + lint in one pass. Flags: `--strict`, `--format`. |
| `scenario check-all` | Same combo across every scenario under the root. Flags: `--strict`, `--format`. |

## Top-level flags

| Flag | What it does |
| --- | --- |
| `--version` / `-V` / `version` | Print the binary's version (text by default; \`--json\` emits `{name, version}`) |
| `--help` / `-h` (top-level or any verb) | Show usage |

## Conventions

- Every enumeration verb that prints a table accepts `--json` to emit
  the same data structured for tooling.
- Every scenario inspection verb that reads a `<file>` accepts `-` to
  read from stdin. Affected: `validate`, `lint`, `check`, `summary`,
  `inputs`, `coverage`, `hash`, `id`, `intent`, `step-ids`, `field`.
- Substring filters (`--filter`, `--tag`, `--profile`) are
  case-insensitive.
- `--format text|json|github` is accepted by `scenario validate`,
  `validate-all`, `lint`, `lint-all`, `check`, `check-all`, plus
  `audit show` and `audit list`. `github` emits GitHub Actions
  workflow commands (`::error file=…::…`) so findings show up as
  inline PR annotations. See
  [`examples/github-actions-scenarios.yml`](../examples/github-actions-scenarios.yml)
  for a copy-paste workflow.
- `--since <iso-ts>` / `--until <iso-ts>` accept ISO-8601 timestamps
  (date-only `YYYY-MM-DD` ok via a `T00:00:00` suffix). Accepted by
  `audit list`, `audit stats`, `audit stats-all`.
- `--limit N` always means "keep the most recent N" when applied to a
  chronologically-sorted list (replays); means "first N alphabetical"
  for sid lists.
- Destructive verbs (`delete`, `prune-replays`, `prune-all`) are
  dry-run by default; `--yes` / `-y` confirms.
- `--strict` on lint promotes warnings to gating (default gates on
  errors only).
- Exit code 0 on success, 1 on failure, 2 on usage error, 3 on
  rebase-guard mismatch (only `heal-promote`).
