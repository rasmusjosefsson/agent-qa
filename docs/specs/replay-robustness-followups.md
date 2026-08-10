# Replay robustness — status and follow-ups

Working notes for the replay-robustness push (headless control, click
robustness, auto-heal). Read this before resuming instead of re-deriving
context.

## Done (merged to `main`)

- **#49** `fix(workbench): inject environment creds into chat replay bootstrap`
  — chat replay now resolves environment shared creds (e.g. OAuth client id)
  under the persona's identity creds via `resolveRunAuthEnv`, same as
  `/connect` and the Runs tab. Previously chat replay only saw the persona's
  own entries and failed because a shared OAuth client id was unset.

- **#50** `feat(replay): robust ARIA-aware click activation for dropdowns,
  options, dialogs` (released as **v0.0.50**) — new `cli/src/dom_activate.rs`,
  shared by record (`smart_click.rs`) and replay (`verbs.rs`). Resolves a
  target by ARIA role + accessible name (or visible text), scrolls it into
  view, and dispatches the full `pointerdown → mousedown → pointerup →
  mouseup → click` chain on the node — beats coordinate clicks that overlays
  intercept and mousedown-bound component-library openers swallow. Includes:
  a broad set of interactive roles (not just button/link), `aria-labelledby`
  name resolution, digit-tolerant name matching, dialog/listbox scope
  preference, backdrop (`role=presentation`) exclusion, a `select` verb
  implementation for role+name comboboxes, and transient-popup recovery
  (re-fire the previous opener when an option/menuitem click finds its popup
  dismissed by the inter-step keyframe capture).

- **#51** `feat(cli): headless by default, --headed/--headless flag` —
  released in **v0.0.51**. `browser::set_headed_mode(headed)` toggles
  `AGENT_BROWSER_HEADED` for every agent-browser child; headless explicitly
  *removes* the var so an inherited value can never silently force a visible
  window. Wired into `agent-qa start [--headed|--headless]` (record) and
  `agent-qa replay [--headed|--headless]` (replay). CLI-core only — the
  workbench does not yet pass this flag anywhere (see below).

- **#52** `fix(cli): restore CI and repair manual heal flow` — fixed the Rust
  1.97 Clippy regression, repaired `heal-apply` so recorder-native value
  corrections survive `flush`, and aligned embedded docs with the current
  manual-heal surface. The `v0.0.51` release workflow published the umbrella
  and platform npm packages successfully.

## Left to do

### 1. Workbench headed toggle (UI)
#51 only wired the CLI flag. Still needed:
- Parse a headed boolean in `runOptsFromBody`, thread it through
  `deps.replay`/`makeReplaySpawner` for scenario and plan runs, and append the
  CLI flag directly in the chat replay route (which calls `deps.runCli`). For
  `handleConnect`, either add a matching profile-bootstrap CLI option or set
  the agent-browser mode env explicitly before bootstrap.
- Add a UI control (Runs tab, and/or Environments) to choose per-run,
  defaulting headless.
- Remember: agent-browser fixes the mode at **daemon launch** — a warm/reused
  session (e.g. `<profile>-session`) keeps whatever mode it launched in until
  closed. A toggle flip needs `agent-browser close --session <name>` (or an
  equivalent recycle) before the next launch to actually take effect.

### 2. Live-pane black-screen fix
The Runs tab's live browser view can show black even when a replay is
actively running. Root cause: `replayStreamSession()` in
`report-server.js` re-derives the session name from the run's `profile`
instead of reading the session the runner actually recorded. Fix: read
`audit.sessionName` from the run's `audit.json` (the runner already writes
this at start, see `cli/src/runner.rs` `RunAudit.session_name`) and attach
the live bridge to that, falling back to the profile-derived name only if
`audit.sessionName` is missing.

(Separately: if a stray **headed** window is left over from manual debugging
against a persistent session — e.g. `admin-user-session` — the live pane's
CDP screencast still can attach to it in principle since
`Page.captureScreenshot` polling works headless or headed; the "black"
symptom in the reported case was the wrong-session bug above, not headed vs
headless itself.)

### 3. Inline replay auto-heal loop — the big one
**Not built yet.** Current replay (`cli/src/runner.rs`) has no in-run
healing: a locator miss just fails the step. The only "heal" mechanism that
exists is `--heal-from-run <runId>`, which pre-loads *caller-supplied*
corrections from a **prior** run's `heal-responses/` directory — there is no
autonomous in-run retry today.

Design (generic ARIA-based, no reference to any other codebase in the
implementation):

- **Ordered strategy ladder** of pure functions
  `(recordedName, liveCandidateNames) -> Option<CorrectedLocator>`, strict to
  permissive, e.g.: whitespace-collapsed match → digit-tolerant match (already
  partially present in `dom_activate.rs`'s `__aqND` helper — reuse/extract
  it) → digit-anywhere → generated-suffix-tolerant → name-prefix. Each
  strategy must **reject on ambiguity** (≥2 live candidates match) rather
  than guess.
- **Failure classification** before attempting heal: distinguish a
  locator-miss (element not found / role+name doesn't resolve) from a
  value-rejection (the app rejected the submitted value — inspect
  post-submit DOM for an alert/toast/banner, and/or GQL response errors if
  the step is `callGql`). Only locator-misses get the strategy ladder;
  value-rejections get classified and surfaced (see note below — no
  auto-retry-with-a-different-value planned).
- **Retry inline, once, and continue**: on a strategy match, re-dispatch the
  same step with the corrected locator. Success → continue, logging that a
  heal occurred. Failure → the run aborts as it does today (a matched-but-
  still-failing retry is a hard failure, not silently swallowed).
- **Persist the heal event**: append a row to a `heal.jsonl` (or similar)
  under the run directory (audit trail — what drifted, which strategy
  matched, whether it changed the outcome), and optionally write a
  non-destructive suggested-patch file so a human/agent can promote the
  correction back into `scenario.json` later via a `heal-promote`-style verb
  (already exists: `cli/src/heal_promote.rs`, `cli/src/heal_respond.rs` —
  check whether the existing promote/respond verbs can be reused as the
  write side of this loop rather than building new ones).
- **Kill switch + strict mode** are cheap to add alongside: an env var to
  disable auto-heal entirely (for CI runs that must fail hard on any drift),
  and a strict mode where even a *successful* heal causes non-zero exit (so
  drift gets surfaced for review even though the run "passed").

Where to wire it in: the step-dispatch loop in `cli/src/runner.rs` (same
loop that already has the transient-popup re-fire logic from #50 —
`prev_click` / `is_popup_content_click`). The heal retry should slot in
next to that, structurally similar (on `dispatch_do` failure, attempt
recovery, retry once, then give up).

### 4. Validation/duplicate-value collisions — mostly a non-issue
Recordings that use `{{vars._unique}}` (via `fill-unique`) already mint a
fresh value on every replay, so "email already taken" collisions from
*replay reuse* shouldn't occur by design. No autonomous
regenerate-and-retry-on-collision is planned — the fix, if this ever
surfaces, is to make sure fields that can collide are recorded with
fill-unique, not to build fragile retry-with-a-different-value logic into
the replay engine. If a *genuine* app-side validation rejection needs
surfacing (not a collision, e.g. a real business-rule error), that's covered
by the value-rejection classification in item 3 above.

## Quick pointers for next session

- Work from the repository root. Any target-specific downstream package is a
  separate repo with separate PRs and is not part of this backlog.
- Test the workbench against a local build with:
  `cd cli && cargo build --release`, then from the repo root run
  `AGENT_QA_BINARY_PATH="$PWD/cli/target/release/agent-qa" agent-qa web`.
- Full CLI test suite: `cd cli && cargo test` (486 on current `main`).
  `cargo fmt --check` before every PR — CI enforces it.
- Branch protection on `main`: PR + squash merge only, no direct/force push.
