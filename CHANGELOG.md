# Changelog

All notable changes to agent-qa are documented here. This project follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
  guest-checkout accordion).

### Fixed

- `frame` step can enter iframes addressed by non-`#id` selectors —
  falls back to the nth `Iframe` snapshot ref when `agent-browser
  frame` only resolves `#id`/refs.
- Trusted `drag` inside an iframe now lands on the widget: endpoint
  coords are offset by the iframe's rect into top-viewport space (and
  the hit-test subtracts it back) instead of pressing dead space.
- Record-side click probe no longer false-warns "no observable effect"
  on `check`/`uncheck`/option picks — it counts capture-phase
  `input`/`change`/`submit`/`toggle` events like the replay probe.

### Performance

- Post-click settle is now adaptive: an always-on page init script
  (`window.__aqNet`) counts in-flight fetch/XHR requests, so a click
  that loads nothing skips the `wait --load networkidle` floor
  (~700ms) after a short grace window — replaying DOM-only clicks
  ~2x faster. Navigation, ajax, or untapped (warm) sessions fall back
  to the same `networkidle` wait as before.

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
