# agent-qa verb reference

The full set of CLI verbs at a glance. Every verb also responds to
`--help` for inline usage and flag detail.

## Recording

| Verb | What it does |
| --- | --- |
| `init [dir] [--force] [--ci]` | Bootstrap a directory: `agent-qa.toml` (`scenarios_root = ./scenarios`), a `scenarios/hello` smoke scenario, `.gitignore` for run artifacts. `--ci` also writes `.github/workflows/agent-qa.yml` (a PR gate running `replay --all`). Idempotent. |
| `start` | Mint a new scenario directory + skeleton `scenario.json` |
| `record-step` | Append one step to the in-flight scenario via the recorder |
| `record pause \| resume \| status` | Freeze capture while you set up page state — record append paths (`record-step`, `smart-click`, `fill-unique`, the editor's auto-record) drop steps while paused instead of writing them. `status --json` emits `{sid, intent, session, paused, steps, startedAt}`. |
| `record continue <sid>` | Extend an existing scenario: replay it to its end state in `--session` (default `default`), seed the buffer with its steps, keep recording — `flush` writes the extended scenario back to the same sid. `--skip-replay` when hand-driving to the end state. |
| `run-step <do|check> <draft-json>` | Dispatch ONE trigger payload against the live session for author-time feedback, without recording. Same direct draft shapes as `record-step`; prints a `{ok,…}` JSON line. `--session`. |
| `aria-snapshot` | Dump the live page's accessibility tree as structured picker rows (a thin adapter over `agent-browser snapshot`). Flags: `--interactive`, `--session`. |
| `cdp-url [--session] [--json]` | Print the live session's CDP WebSocket endpoint. Powers the editor's inline live-browser pane (screencast + drive-to-record: clicks, typing, select commits, checkbox/radio toggles, and named-key presses, right-clicks, file uploads, dialogs, page state, iframe moves, viewport resizes, gestures, and downloads all land as steps). Read-only. |
| `buffer list \| delete <i> \| move <from> <to> \| edit <i> <draft-json> \| load <sid> [--force] \| clear` | Inspect / reorder / rewrite / delete rows in the in-flight buffer; delete + move re-index `s0,s1,…` so `flush` stays clean. `edit` re-validates the draft and preserves the step's id/kind. `load` copies a saved scenario's steps into the buffer for editing — `flush` then writes back to the same `<sid>` and keeps fields the buffer doesn't model (`id`, `tags`, `inputs`, `templates`, `env.close`). `list --json` (includes `paused`, `editing`). |
| `fill-unique` | Locator-uniqueness helper for `type`/`fill` style do steps |
| `smart-click` | High-level click that resolves a label to a unique locator |
| `smart-fill` | Type a literal value into a field by accessible name (default role `textbox`, `--role` to override) + auto-record a `do/type` step. `fill-unique` is the minted-per-run variant. |
| `truncate` | Drop the trailing N steps from the in-flight scenario |
| `flush` | Persist the recorder buffer to `scenario.json` |
| `verify [--fix]` | Check the active recording — dense step ids + paired snapshot/screenshot sidecars. `--fix` renumbers to dense s0.. ids, rewiring `{"from":"step"}` refs and moving sidecar files to match. |

> `run-step`, `aria-snapshot`, and `buffer` are the primitives the local
> **authoring editor** (`agent-qa web` → the *Editor* tab) shells
> to build a scenario by targeting the UI. The editor is hosted by the
> Node launcher; the Rust CLI still owns every record/run/flush mutation.

## Replay

| Verb | What it does |
| --- | --- |
| `replay <sid \| path>` | Run a scenario. Flags: `--profile`, `--session`, `--param name=value` (repeatable), `--heal-from-run <runId>`, `--dry-run`, `--no-sidecars`, `--runs <N>`, `--quiet`/`-q`, `--tag <label>`, `--output-audit <path>`, `--from <stepId>` / `--until <stepId>` (partial replay over a step window — needs a warm session at that state), `--keep-going` (dispatch every step past the first failure), `--retry <N>` (re-run until pass, keeping flake evidence), `--junit [path]` (JUnit XML; bare flag writes `<run>/junit.xml`), `--record-video [path]` (film the run as webm), `--freeze` (pin clock + RNG for deterministic goldens), `--har` (write `<run>/network.har`), `--mock-from <runId>` (hermetic replay from a recorded HAR), `--offline` (reject unmatched fetch/XHR), `--base-url <url>` (retarget onto another deploy), `--auto-promote` (write this run's locator heals back), `--update-baselines` (mint `baselines/` after the run), `--persona <id>` (replay as a `_personas/<id>` record — profile + credentials, vault refs resolved) + `--environment <id>`/`--env <id>` (replay against a `_environments/<id>` record — params, baseUrl, auth config; defaults to the `default` env when a persona is picked), `--fresh-browser` (close the session's browser before the run — the only reset for daemon-side `set` state like viewport/device/geo/credentials/media that has no off-switch). With `--all`: replay every scenario under the root — `--shard k/n` splits the sorted sid list for CI matrices, `--filter <substr>` narrows it, `--tags <a,b>` keeps scenarios declaring any of the named `tags`, `--jobs <N>` runs scenarios in parallel, `--report <path>` writes a markdown verdict table for a PR comment. |
| `list` | Enumerate scenarios (root mode) or one scenario's replays. Flags: `--json`, `--filter <substr>`, `--limit <N>` |
| `compare <a> <b>` | Diff two replay run directories. Alias `diff`. |
| `crawl <url>` | Coverage scaffolding — enumerate same-origin links + interactive elements and write a draft scenario (goto + shot claim per route) to `<scenarios_root>/crawl-<host>/` plus a `crawl-report.json` authoring inventory. Flags: `--session`, `--out`, `--max`, `--sid`, `--depth <N>` (BFS past the entry page), `--mint-baselines` (capture `baselines/` PNGs while drafting). |
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
| `audit flaky <sid>` | Flag steps whose outcome interleaves pass/fail across runs — the flake signature (vs `heal-chronic`, which flags locator churn). Flags: `--min-flips N` (default 2), `--min-runs N` (default 3), `--json` |
| `audit slow <sid>` | Flag steps whose duration regressed — every one of the last `--recent` pass runs (default 2) exceeds the earlier-run median by `+--pct%` (default 50) and `--min-ms` (default 250). Pass rows only; a fail's `ms` is the timeout budget, not step cost. |
| `audit health` | Cross-scenario rollup of `flaky` + `slow` + `heal-chronic` at their defaults — one row per scenario with silent degradation, none when the suite is quiet. `--json` emits one compact line (the workbench consumes it to badge scenario rows). |
| `audit trend <sid>` | Outcome + duration trend over the scenario's runs — pass%, median secs, a `✓/✗` outcome line, and a duration sparkline. `--limit N` (default: all runs) windows to the latest N; `--json` emits the same data structured. |
| `audit trend --all` | Suite board: one trend row per scenario that has replays (sid, pass%, run count, median, outcomes, sparkline). `--limit N` windows each row to the latest N runs; `--json` emits the rows array. |
| `audit trend <sid>` | Outcome + duration trend over the scenario's runs — pass%, median secs, a `✓/✗` outcome line, and a duration sparkline. `--limit N` (default: all runs) windows to the latest N; `--json` emits the same data structured. |
| `audit cluster` | Group step failures across every scenario's runs by normalized error signature (quoted literals + digit runs stripped) — one root cause across N runs reads as one cluster with its member list. `--min-size N` (default 2) hides lone failures; `--json` for the structured list. |
| `audit verdict <sid> <runId \| latest>` | One-word triage for the run: `PASS` (exit 0) green and clean, `FIX` (exit 2) green but self-corrected (auto-heals or value-rejections — review heal.jsonl + promote), `BLOCK` (exit 1) failed. `--json` for the structured verdict incl. the offending stepIds. |
| `audit explain <sid> [runId \| latest]` | One-block failure digest for a run: verdict + failing steps (error + screenshot pointer) + heal trail + console errors + failed/pending requests, plus a `next:` list of the follow-up commands that fit the evidence. Exit code mirrors `audit verdict` (0/1/2). `--json` prints the same digest structurally — the single read an agent needs to repair a broken run. |



## Plans

Plans/cases/sets are the workbench's run-scope records under `<root>/_plans`, `_sets`, `_cases` (created/edited in the UI); `plan` is their terminal + CI twin.

| Verb | What it does |
| --- | --- |
| `plan list` | Every plan with its resolved member count. `--json` emits one compact line. |
| `plan cases <planId>` | Resolved member cases (caseId → scenario sid → title), in run order. `--json`. |
| `plan run <planId>` | Replay every member case's linked scenario and roll up pass/fail/skip — the PR/CI gate entrypoint (`plan run regression --json`). Cases with no recorded scenario or a missing scenario dir are `SKIP`ped. Flags: `--profile <p>`, `--param k=v` (repeatable), `--headed`/`--headless`, `--dry-run`, `--no-sidecars`, `--json` (single-line rollup as the last line). Exit 0 iff every started run passed. |




## Heal

| Verb | What it does |
| --- | --- |
| `heal-respond` | Record an authoring decision against a failed step |
| `heal-promote <sid>` | Apply replay-side patches into `scenario.json` (rebase-guarded) |
| `heal-apply <sid>` | Mark a heal-response as consumed |
| `heal-list <sid>` | List heal-responses. Flags: `--run <runId>`, `--mode value-correction\|reject`, `--applied`, `--unapplied`, `--json` |
| `heal-chronic <sid>` | Flag steps that auto-healed in ≥ `--min-runs` distinct runs (default 2) — silent locator debt. Prints the `heal-promote` command per step. Flags: `--min-runs N`, `--json`, `--issue` (paste-ready markdown issue body for the handoff) |
| `shot-accept <sid>` | Mint screenshot baselines for `{"shot"}` claims: copies the run's per-step PNGs into `<sid>/baselines/`. Flags: `--run <runId>` (default `latest.txt`), `--steps <csv>` (default every captured shot), `--json` |

### `{"shot"}` claims — visual diff vs a baseline

A check step `{"check": {"shot": "<stepId>"}, "predicate": "matches"}` pixel-compares
the current run's `screenshots/<stepId>.png` against `<sid>/baselines/<stepId>.png`.
It passes when the differing-pixel fraction ≤ `tolerance.pixels` (default `0.01` = 1%);
on a miss the claim fails and a red delta map lands at `<run>/shots-diff/<stepId>.diff.png`.
Size changes fail outright — re-mint with `shot-accept` when the change is legitimate.

`mask` lists CSS selectors to hide (`visibility:hidden`) around every step
screenshot — the ignore-regions escape hatch for volatile UI like timestamps,
live badges, or user avatars. Masks union across all shot claims in the
scenario and apply to baselines and replays alike, so masked regions can never
flake the diff:

```json
{"check": {"shot": "s3", "mask": ["[data-qa-volatile]", "time"]}, "predicate": "matches"}
```

A scenario with shot claims opts out of the warm-page `goto` skip: the step
always navigates, so the diff compares a fresh document — a reused session can
otherwise hold a stale DOM (an older bundle or settled live data) that no
amount of masking fixes. `goto` to a `file://` URL also always reloads — the
fixture may have been edited since the warm load — and any `goto` can force a
reload with `params.reload: true`.

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
| `scenario validate-all` | Schema-validate every scenario under the root. Flags: `--root <dir>` (overrides the configured root), `--json`, `--format text\|json\|github`. |
| `scenario summary <file>` | Per-step summary. Flags: `--filter <substr>`, `--json`. |
| `scenario inputs <file>` | List declared inputs. `--json`. |
| `scenario new <file>` | Scaffold a minimal valid scenario. Flags: `--force`, `--url`, `--intent`. |
| `scenario insert <file> <do\|check> <draft-json>` | Splice a validated step into a saved scenario. `--after <stepId>` or `--at <index>` sets the position (default: append); the new step's id is the first free `s<n>`. |
| `scenario diff <a> <b>` | Unified diff between two scenario JSONs |
| `scenario hash <file>` | SHA-256 of scenario bytes (rebase-guard hash) |
| `scenario id <file>` | Print the scenario's id field on one line |
| `scenario intent <file>` | Print the scenario's intent field on one line |
| `scenario step-ids <file>` | Print every step id on its own line |
| `scenario field <file> <name>` | Print any top-level scenario field (scalars verbatim; object/array as compact JSON) |
| `scenario rename <sid> <new>` | Rename a scenario directory + id field |
| `scenario copy <sid> <new>` | Copy a scenario (replays not copied) |
| `scenario extract <sid>` | Clone a scenario truncated at a run's first failing step — a minimal standalone repro. Defaults to the latest run; `--run <runId>` picks another, `--through <stepId>` cuts at an arbitrary step, `--to <new-sid>` names the clone (default `<sid>-extract`). Baselines carry over; the extract gets an `extract` tag. |
| `scenario tag <sid>` | List or mutate a scenario's `tags[]` — the field `replay --tags` selects on. Flags: `--add <a,b>`, `--remove <c,d>`, `--json` |
| `scenario delete <sid>` | Remove a scenario directory. `--yes` / `-y` confirms; otherwise dry-run. |
| `scenario prune-replays <sid> --keep N` | Keep most recent N replays. `--yes` / `-y` confirms. |
| `scenario prune-all --keep N` | Same across every scenario. `--yes` / `-y` confirms; `--root <dir>` overrides the root. |
| `scenario coverage <file>` | Per-step check coverage ratio. `--json`. |
| `scenario coverage-all` | The same do→check ratio rolled up across every scenario in the root — rows sorted worst-first + OVERALL rollup. `--filter <substr>`, `--root <dir>`, `--json`. |
| `scenario lint <file>` | Common-smell linter. Flags: `--json`, `--format text\|json\|github`, `--strict`, `--rule <code>` (repeatable), `--exclude-rule <code>` (repeatable), `--list-rules`. |
| `scenario lint-all` | Same across every scenario under the root. Flags: `--root <dir>` (overrides the configured root), `--json`, `--format`, `--strict`, `--rule`, `--exclude-rule`. |
| `scenario check <file>` | Schema validate + lint in one pass. Flags: `--strict`, `--format`. |
| `scenario check-all` | Same combo across every scenario under the root. Flags: `--root <dir>` (overrides the configured root), `--strict`, `--format`. |

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
