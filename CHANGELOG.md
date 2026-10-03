# Changelog

All notable changes to agent-qa are documented here. This project follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5] - 2026-10-03
### Added

- Replay video overlay: with `--record-video`, each step paints a
  Cypress-style step rail (pending dimmed, current highlighted, pass/fail
  colored) plus a highlight ring on the target element. Hidden during
  sidecar screenshot capture so `shot` goldens never diff the overlay.
  (0.1.5)

## [0.1.4] - 2026-10-03
### Added

- `agent-qa init --store <local|github|turso>` writes an active
  `[baselines]` table (github detects `owner/name` from `git remote`,
  turso takes `--url`), and `--ci` now emits the full golden loop:
  replay gate publishing baseline/actual/diff images + replay video to
  a `shot-diffs` branch with an inline PR comment, an environment-gated
  `goldens apply` job, the comment-triggered `goldens apply` mint, and
  a `mint_on_push` job that adopts a merged drift into the store on
  default-branch replay failures.

### Changed

- `--record-video` bare flag writes `<run>/run.mp4` (was `run.webm`) —
  MP4 renders inline in GitHub comments; `=<path>` still honors its
  extension.

### Fixed

- Emitted workflow: the replay job no longer runs on `issue_comment`
  events (every `goldens apply` comment burned a full replay on the
  default branch), and the `goldens apply` env gate is only offered
  when shot claims actually drifted — a non-visual failure no longer
  offers to overwrite goldens with broken-render bytes.

## [0.1.3] - 2026-10-03
### Added

- Pluggable golden storage: `[baselines] store` in agent-qa.toml —
  `local` (default, committed with the repo), `github` (a second repo
  via the contents API), `turso` (libSQL db). Replay pulls remote
  goldens into `<sid>/baselines/` before the step loop
  (`--no-baseline-sync` opts out); `shot-accept`, `domshot-accept` and
  `replay --update-baselines` push after minting. A `.store.json`
  manifest tracks local/remote hashes so only changed files move;
  sync is additive (nothing is ever deleted remotely). New
  `agent-qa baselines pull|push|status` verb for manual sync, and a
  workbench Settings "Golden storage" card to pick the backend and
  run sync (`/api/baselines/config|status|sync`).
- Workbench chat prompts are annotated with the page the chat's browser
  pane is currently on (`[workbench context: ...currently on <url>]`), so
  "record this page" works without re-stating the URL. The chat primer
  also teaches the agent to read the live URL itself
  (`agent-browser get url`) instead of asking.
- The Chrome extension is downloadable without a repo checkout: the
  workbench's Extensions page serves it as `/api/extension.zip` (the
  sources ship inside the npm tarball), and each GitHub release attaches
  `agent-qa-extension.zip`.
- Workbench Goldens page (Test pipeline nav): every scenario's baselines
  — `shot` PNGs as thumbnails, `domshot`/text files as entries — with
  the configured store in the header, a per-golden enable switch
  (`POST /api/goldens/<sid>/<name>/enabled`), a `diff` badge when the
  latest run produced a shots-diff map, and a click-to-compare dialog:
  draggable before/after split (baseline vs the latest run's screenshot)
  plus the red-on-faded diff overlay. `GET /api/goldens` and
  `/api/goldens/<sid>/<name>[/{actual,diff}]` serve the data.
- Revertible goldens (turso store): `remote_put` logs every pushed
  version to an append-only `baseline_history` table, and new verb
  `agent-qa baselines revert <sid>|--all` rolls each file back one
  version and pulls the restored state. `local`/`github` stores bail
  with a pointer to git history, which already versions goldens.
- `Step::Check` gains optional `enabled` — `"enabled": false` in
  scenario.json skips the claim at replay (`status: "skip"` event +
  `<skipped/>` junit case). The Goldens page switch writes this.

### Fixed

- Chat agent reliability: the primer now front-loads the rules that were
  previously easy to miss — load `agent-qa skills get core` before the
  first agent-qa command, use `smart-click`/`smart-fill`/`fill-unique`
  (they perform AND record) instead of performing a gesture with
  `agent-browser` and then guessing a `record-step` draft, and the
  `record-step` draft grammar (`on` locator, not `target`/`element`).
- Run dirs minted without `status.json` (a replay that died during
  startup) resolved `state: null`, so the Runs page showed "in flight"
  — and the detail pane showed a phantom "Signing in…" banner —
  forever. `runState` now falls back to freshest-artifact freshness:
  nothing written within the staleness window → `stale` (interrupted),
  and the sign-in banner is gated to genuinely live runs.
- Workbench sidebar logo enlarged to 44px so the mark reads at the same
  height as the title text.
- Chat right column: the recording steps panel now has a draggable divider
  against the live browser pane (was a fixed 60/40 split), so the steps
  list can be grown to most of the column.
- "No sign-in" now survives a page refresh: the persona select no longer
  re-seeds a profile while the chat is bound guest.

## [0.1.2] - 2026-10-02
### Fixed

- Launcher `agent-qa update` now verifies the platform binary after
  reinstalling and warns when npm silently skipped the
  optionalDependencies (launcher newer than `cli`, or binary missing)
  instead of reporting a clean update.
- Publishing: new `prepublishOnly` guard
  (`scripts/check-umbrella-deps.js`) fails `npm publish` when the
  own-scope platform optionalDependencies don't match the umbrella
  version OR when the pinned platform versions aren't on the registry
  yet — a manual publish can no longer ship a launcher with no
  resolvable binary. `release.yml` also verifies the published
  umbrella's registry optionalDependencies match the tag.

## [0.1.1] - 2026-10-02

### Added

- Workbench chat header shows which backend, model, and cumulative
  usage is active (`pi · claude-haiku-4-5 · $0.0123`). `GET
  /api/chat/c/:id/state` now reports `usage` — pi reads
  `getSessionStats()` (`cost` + `tokens`), opencode sums per-message
  `info.cost`/`info.tokens` — and the badge refreshes on rehydrate,
  model/thinking changes, and after each turn (`agent_end` triggers
  a state re-fetch).
- The opencode backend defaults new sessions to the cheapest chat
  model — `AGENT_QA_CHAT_MODEL` substring match first, else the
  first `/haiku/` entry in the model list (pi already applied it on
  every create).
- Workbench sidebar uses the Agent Spark mark (theme-aware light +
  dark variants) instead of the old placeholder icon.
- Workbench chat: "No sign-in" guest mode. The sign-in select now offers
  "No sign-in", which clears any connected persona binding
  (`POST /api/chat/c/:id/disconnect`) and marks the chat as guest so the
  background default-persona auto-connect never re-fires for it.
  `GET /api/chat/c/:id/connection` reports `"guest": true` in that
  state, and the chat agent primer tells the agent to browse and record
  anonymously — a sign-in screen on the site under test is the page,
  not a problem. Connecting a persona later clears guest mode.
- `docs/process-hygiene` — new docs page covering `agent-qa ps`
  (live/orphan/stale sessions, unowned Chrome + daemons, stray
  profile dirs) and `agent-qa cleanup` (`--all`, `--session`,
  `--older-than`, `--dry-run`, `--json`), wired into the site sidebar
  and synced via `sync-docs.mjs`.
- Docs-site visual goldens (`evals/docsite` + `docs-goldens.yml`): the
  docs site is shot-claimed end-to-end — every built page in light and
  dark — using the same baseline/diff/accept loop as the workbench
  goldens. `bun evals/docsite/gen.ts` regenerates the scenario from the
  built page list (CI fails when it drifts); `/docs-goldens accept`
  mints baselines on the PR head with CI rendering.
- `agent-qa ps` + `agent-qa cleanup` — visibility and reaping for
  agent-browser zombies. `ps` correlates the socket-dir session
  registry with the process table into `live`/`orphan`/`stale` rows
  (daemon pid, age, tree RSS, last URL) plus unowned Chrome processes
  and stray `/tmp/agent-browser-chrome-*` profile dirs. `cleanup`
  removes stale/orphan registry files, kills unowned Chrome, and
  deletes stray profile dirs; `--all` also closes live sessions
  gracefully, `--session <n>` scopes, `--older-than <dur>` filters,
  `--dry-run` prints the plan, `--json` for automation.
- `scripts/cut-release.js` — release prep + cut in one step: bumps
  `npm/agent-qa/package.json`, stamps `## [Unreleased]` into a dated
  `## [<v>]` section, commits, tags, and pushes (`--push`, `--dry-run`,
  `--allow-empty`; bails on a dirty tree, non-main branch, empty
  changelog, or an existing tag). The tag push is what runs the
  `release` workflow — cross-build, npm publish, and a GitHub release
  whose notes are the `## [<v>]` section it just stamped.
- Golden sweeps on material.angular.dev (CDK portals: select,
  autocomplete, datepicker, dialog, menu) and demowebshop.tricentis.com
  (nopCommerce: ajax cart, `{{vars._unique}}` registration, six-step
  guest-checkout accordion), and blazedemo.com (select-driven flight
  search, full purchase flow, malformed-HTML row cells via
  `nth-of-type`, canned receipt echo, positive `pageError` claim for
  the site's broken JS).

### Fixed

- Workbench: a dead replay no longer hangs on "IN FLIGHT" forever.
  `status.json` has no heartbeat, so a killed run still read
  `state: "running"`; a run whose status mtime is older than
  `AGENT_QA_RUN_STALE_MS` (default 5 min) now reports `state:
  "stale"`, which the Runs UI renders as "interrupted" (sidebar
  badge, run banner, non-terminal no more).
- Workbench chat keeps its history across the 15-minute idle
  teardown: the pi backend now persists sessions as JSONL under
  `<recordDir>/agent-session/` (`SessionManager.create/open` with a
  `current.path` marker), and the opencode backend reuses its
  `sessionID` across recreates since the serve child outlives the
  adapter. "New chat" sets a one-shot `fresh` flag so it still
  starts from scratch instead of resuming.
- Workbench live pane no longer goes blank ("Browser ready") while
  the agent is recording — the blank-tab guard (`blankTab`) now
  stays live when `autoRecording` is on, so the canvas shows the
  recorded browser during the pre-navigation phase.
- Recorded-step keyframes no longer capture a black frame:
  `capture_recording_sidecars` waits 350ms after the recorded action
  before `browser::screenshot`, letting SPA paints land.
- `agent-qa cleanup` no longer leaks a wedged session: when `close
  --session` fails, the recorded daemon pid is terminated directly
  instead of only deleting the registry files (which orphaned the
  whole Chrome tree). `ps` now also reports **unowned agent-browser
  daemons** — bare daemon procs whose registry entries vanished — and
  `cleanup` reaps them. Stray profile dirs are now dirs no Chrome
  process uses at all (an orphan Chrome's own dir no longer
  misreports as stray), and `cleanup` does a post-close rescan to
  sweep Chrome children orphaned mid-sweep, plus their profile dirs.
- `frame` step can enter iframes addressed by non-`#id` selectors —
  falls back to the nth `Iframe` snapshot ref when `agent-browser
  frame` only resolves `#id`/refs.
- Trusted `drag` inside an iframe now lands on the widget: endpoint
  coords are offset by the iframe's rect into top-viewport space (and
  the hit-test subtracts it back) instead of pressing dead space.
- Record-side click probe no longer false-warns "no observable effect"
  on `check`/`uncheck`/option picks — it counts capture-phase
  `input`/`change`/`submit`/`toggle` events like the replay probe.
- `start --mock-from`/`--offline` no longer clobbers a caller-set
  `AGENT_BROWSER_INIT_SCRIPTS` — the mock init script is merged into
  the path list like replay does.

### Performance

- Post-click settle is now adaptive: an always-on page init script
  (`window.__aqNet`) counts in-flight fetch/XHR requests, so a click
  that loads nothing skips the `wait --load networkidle` floor
  (~700ms) after a short grace window — replaying DOM-only clicks
  ~2x faster. Navigation, ajax, or untapped (warm) sessions fall back
  to the same `networkidle` wait as before.
- Post-navigation `networkidle` waits (`goto`, the settle fallback, and
  a bare `wait` step) are now bounded to 5s — an analytics/beacon-heavy
  page that never goes idle no longer stalls a run ~30s per navigation.

## [0.1.0] - 2026-09-28

First tagged release. Everything under the pre-release `0.0.x` line lands
here; see the sections below grouped by surface.

### Added

**Visual golden testing**
- `{"shot": "<stepId>"}` claims pixel-diff the run's screenshot against a
  committed `baselines/` PNG; misses write a red delta map under
  `shots-diff/` and fail with the percentage. `shot-accept` mints baselines
  from a run (`--dry-run` previews new/identical/update), and lint warns on
  shot claims with no golden, scenarios with no visual check, and orphan
  baselines. `mask` hides volatile selectors during capture; `clip` crops a
  diff to one element box; a per-pixel antialias threshold filters font-AA
  noise. `replay --update-baselines` re-mints in-run; `crawl
  --mint-baselines` captures goldens while drafting; `record flush
  --auto-shots` appends a shot claim after every do-step.
- Workbench: diff maps render inline on failed steps, "Re-mint baseline"
  (per step or the whole run), a camera action inserts a visual check after
  any do-step, and a shot% badge rides the coverage column.

**Network-aware scenarios**
- `{"network"}` claims (`fired`, `status`, `responseJsonPath`,
  `postDataContains`), `do/wait-request`, `wait idle`, `do/mock` +
  `do/unmock` in-page stubbing with `abort`, `--mock-from` hermetic replay
  from a recorded HAR, `--offline` (unmatched fetch/XHR rejects), `--har`
  run capture, `<run>/network.json` + `<run>/console.json` artifacts,
  `console` claim subjects, entry-page network claims in `crawl` drafts,
  and a Network tab + network section in the compare view.

**Replay & suite execution**
- `--all`, `--shard`, `--filter`, `--tags`, `--jobs N` parallel runs,
  `--retry N`, `--keep-going`, `--from`/`--until` step windows,
  `--freeze` (pinned clock + RNG), `--auto-promote` (write heals back),
  `--base-url` retargeting, `--junit` XML, `--report` markdown table,
  `--record-video`, and per-step `params.retry`.

**Authoring & editing**
- `buffer` ops: `load` (a saved scenario), `insert`, `check` (validate as
  flush would write it); `record` `pause`/`resume`/`edit`, `continue`
  (extend an existing scenario), and `flush` preserving unmodeled fields;
  `scenario` `insert`/`copy`/`tag`; `verify --fix`; `init` (+ `--ci`);
  `crawl` with `--depth` BFS and `--max`; step-id reference rewiring on
  buffer renumber.

**Audit & triage**
- `audit flaky` (outcome interleave), `audit slow` (duration regression),
  `audit verdict` (PASS/FIX/BLOCK), `audit cluster` (failure signatures),
  `audit trend`/`--all` (sparklines + suite board), `heal-chronic` (+`--all`),
  `health` rollup, `coverage`/`coverage-all` (+ shot%), `run-report` HTML,
  and `plan` from the terminal.

**Workbench**
- Chat recording controls (pause/resume, step edit/delete, buffer check),
  plan dashboard heal badges + per-case re-run, per-chat live badge on the
  tab strip, runs-pane compare view, heal-patch promote button, insert-check
  dialog covering every claim subject, run video playback, tags on Runs +
  Cases, trend chip, Settings tab (chat backend, headed default, paths),
  Crawl dialog, and the opencode chat backend (`AGENT_QA_CHAT_BACKEND`).

**CI & packaging**
- `action.yml` composite GitHub Action (replay-on-PR for any app repo),
  `qa-gate` + `ui-goldens` workflows, `init --ci` bootstrap, `/qa accept`
  PR-comment golden promotion, `qa-crawl` draft-coverage comments on UI PRs,
  nightly evals, `check-all` over committed examples, linux-x64 platform
  package, and an uncommitted `lib/public` bundle built by CI/release.

**Core surface**

- **Recording** — `start`, `record-step`, `fill-unique`, `smart-click`,
  `truncate`, `flush`, `verify`.
- **Replay** — `replay` with profile/session binding, parameter overrides,
  and heal-from-run; per-step ARIA snapshot + screenshot evidence.
- **Compare** — per-step ARIA snapshot diff + screenshot pixel diff.
- **Design review** — `design review`, `design verdict`: a design-fidelity lane
  with its own exit code, separate from the behavioural pass/fail. Reference
  images live at `<sid>/designs/<stepId>.png`; verdicts (`ok`, `accepted`,
  `fail`, `ask`) are committed and expire when the design file changes.
- **Heal** — `heal-respond`, `heal-promote`, `heal-apply`.
- **Profiles** — `profile-add`, `profile-status`, `profile-bootstrap`,
  `profile-list`.
- **Diagnostics** — `doctor`, `info`, `byo-doctor`, `perf-snapshot`.
- **Plugin protocol** — subprocess + JSON-over-stdio; discovery via
  `--plugin`, `agent-qa.toml`, `AGENT_QA_PLUGINS`, or `$PATH`.

### Fixed
- `chatUnavailableFields` reading a stray `root` instead of the settings
  root; `Shot` pattern mismatches on `clip`; shot-claim step refs left stale
  after buffer renumber; `scenario copy` dropping `baselines/`; the
  `ci-browser-contract` awk race; sub-pixel AA jitter in shot diffs; and the
  crawl inventory string-decode bug that zeroed link discovery.
