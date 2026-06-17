---
name: core
description: CDP-driven UI inspection and record/replay CLI. Two modes share the same plumbing — (a) record/replay a scenario when you need a replayable contract with an audit trail (the dominant flow in this file); (b) inspect a live page (perf, React tree, network, ARIA snapshot, asserts) when you need to debug or validate without producing a scenario. Use when asked to "record a scenario", "capture a flow", "replay this", OR when asked to debug/inspect/validate a live page via BYO. Scenario outputs land under `tmp/agent-qa-scenarios/<scenarioId>/` (override with `AGENT_QA_SCENARIOS_DIR`); inspection outputs go to stdout/stderr or `--out` paths per verb.
allowed-tools: Bash(agent-qa:*), Bash(yarn agent-qa:*), Bash(agent-browser:*)
---

# agent-qa core

Drive the tab via CDP. Two complementary modes:

- **Scenario mode** — record each step + per-step page capture, emit a `scenario.json` the orchestrator (or any consumer) can replay later. The recorder OBSERVES; the agent EXECUTES the actions. This is what the rest of this file covers in depth.
- **Inspection mode** — debug or validate a live page without producing a scenario: `perf-snapshot`, `inspect`, plus `agent-browser` primitives (`snapshot`, `react tree --json`, `react inspect <id> --json`, `network har`, `network requests`, `trace`). Most commonly paired with `--byo` to drive the user's own browser. See [`references/inspect.md`](references/inspect.md) for the inspection-mode catalogue and `agent-qa skills get byo` for the BYO bridge.

> **STOP — this skill is the complete source of truth. READ THIS PARAGRAPH BEFORE ANY TOOL CALL.**
>
> If the user asked you to RECORD a scenario, the ONLY exploration permitted between "user request" and "first `start` call" is:
>
> 1. `agent-qa skills get core` (this file).
> 2. `agent-qa skills get core --full` (this file plus every reference + template).
> 3. `agent-qa skills get pages` (route lookup).
>
> That's it. **Do not** do any of the following before recording, even if it "feels" responsible:
>
> - ❌ `ls`, `glob`, or `find` for `examples/scenarios/`, `tmp/agent-qa-scenarios/`, or any other scenario directory. Existing scenarios are NOT reference material — every flow is different and copying patterns from old ones leads to wrong choices.
> - ❌ `grep` / `rg` for `"type": "assert"`, `record-step`, `fill-unique`, `heal-respond`, or any verb name across the repo. The CLI doc above is the syntax reference.
> - ❌ `read` of `src/**` source files, `AGENTS.md`, `examples/scenarios/**/scenario.json`, `.agents/gotchas/`, `docs/scenario-system/`, or `currentState/`. None of those will tell you anything that isn't in `agent-qa skills get core --full`.
> - ❌ Re-reading `agent-qa skills get core` looking for a section you "missed" (e.g. assert syntax). The doc has cross-references; load the matching `references/<topic>.md` via `agent-qa skills get core --full` instead of grepping the source.
> - ❌ Running `cat`, `head`, `tail`, or `read` on `tmp/agent-qa-scenarios/<sid>/` to "see what a finished scenario looks like" before starting your own. You don't need to.
> - ❌ **Copying the output of `agent-qa skills get core` to a file** (e.g. `tmp/skill-core.md`) so you can `read` it in chunks. The CLI command IS the canonical reader — re-run it, scroll your terminal, or page through it. Don't materialise a duplicate.
>
> **If you genuinely cannot find something, ask the user — then fix this file** (`skill-data/core/SKILL.md`) or the relevant `references/<topic>.md` so the next agent finds it. Treating the skill as a hint and "verifying" against the source is the failure mode this rule exists to stop.
>
> **Don't narrate plumbing.** Network-block routes, `agent-browser session list` — these are setup. Run them silently. Skip straight to user-meaningful actions: navigated, opened modal, filled, created.
>
> **Ignore "user opened file" reminders unless clearly relevant.** A reminder like _"the user selected lines 10-11 from scenario.json"_ is a hint, not a task. If the file isn't part of what was just asked, don't stop to read it.
>
> **HARD BOUNDARY — agent-qa is self-contained. DO NOT load, read, or follow any other skill while operating agent-qa, ever, without explicit user consent.** Sibling skills describe different pipelines with different conventions (different env vars, different auth flows, different state machines); applying their guidance to an agent-qa failure produces silent corruption, wrong credentials, and wasted hours. The only skills you may load are those returned by `agent-qa skills list` (i.e. `agent-qa skills get core` / `core --full` / `pages` / `profiles`). If an agent-qa command misbehaves and the answer is not in those, **STOP and ASK THE USER** — do not cross the boundary on your own initiative. If the user grants consent, narrate which skill you are about to load and why. If something is genuinely missing from agent-qa's own skills, ask the user, then update the agent-qa skill files so the next agent finds it.

> **BYO escape valve.** If — and ONLY if — the user explicitly asked, IN THE CURRENT TURN, to drive their own browser ("use my browser", "byo", "attach to my Brave Nightly", etc.), load `agent-qa skills get byo` and follow it. Never invoke `--byo` without first posting the R2 confirmation prompt the byo skill mandates. Never infer BYO from context, never carry it across turns, never use it as a debugging convenience.

## Bundled verbs (high-level)

20 verbs grouped by lifecycle stage. Invoke them via `agent-qa <verb> [args]`. Do NOT rebuild them as ad-hoc bash. Run `agent-qa` (no args) for the grouped help screen with one-liners; `agent-qa skills get core --full` includes the full per-verb reference (`references/verbs.md`).

| Stage         | Verbs                                                                                                                                                    |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Recording** | `start`, `smart-click`, `fill-unique`, `record-step`, `truncate`, `flush`, `verify` ([recovery](references/recovery.md))                                 |
| **Inspect**   | `inspect`, `list`, (internal: `probe-step`, `parse-probe`)                                                                                               |
| **Heal**      | `detect-failure`, `quarantine`, `heal-promote`, `heal-respond`, `heal-apply` ([reference](references/heal.md), [`heal-apply`](references/heal-apply.md)) |
| **Replay**    | `replay`, `diff`, `compare`                                                                                                                              |
| **Profiles**  | `profile-bootstrap`, `profile-status`                                                                                                                    |
| **Skills**    | `skills` (list / get / path)                                                                                                                             |

## The recording loop (read this carefully)

Recording is **strictly serial**. One step at a time, in this exact order:

Use one browser session for the whole recording. If you drive any action manually with `agent-browser --session <name>`, that `<name>` must match the session passed to `agent-qa start --session <name>`; otherwise `record-step` and `smart-click` will observe/click a different tab.

1. **Snapshot first** to see the page's accessibility tree and pick a target by `[ref=eXX]` or by role + accessible name:
   ```bash
   agent-browser --session <name> snapshot | less
   ```
2. **Drive the action.** Use `agent-qa smart-click "<accessible name>"` / `agent-qa fill-unique` when they apply (one call: drives the browser AND records). `fill-unique` only applies to newly-created values whose template contains `{{vars._unique}}`; fixed credentials and existing values are plain fills. When helpers don't apply (e.g. fixed username/password, role+name lookup misses, click by snapshot ref, etc.), drive the browser yourself with `agent-browser <verb>` then call `record-step` immediately afterwards.
3. **Wait for `record-step` to return successfully** before driving the next action.

`record-step` validates the trigger payload up front (allow-listed methods, required fields), so typos fail at the record site with a copy-pasteable hint. It does NOT drive the browser — it only writes the row + the per-step ARIA snapshot + screenshot sidecars.

Returning failure (or any non-OK outcome from `smart-click` / `fill-unique`) means the scenario is in a bad state — **stop, do not plow forward, and run the heal loop** (see [`references/heal.md`](references/heal.md)).

**NEVER batch multiple `record-step` calls into one tool message.** A flock guards the steps file and the script will refuse to overwrite an existing keyframe at the same index, so a parallel batch will fail loudly rather than silently corrupt. The OBSERVATION the recorder makes is of the live tab — if you start the next action before the prior keyframe captured, the keyframe captures post-action state, not pre-action.

Equally, **do not batch `agent-browser <action>` and the corresponding `record-step` into one tool message** for the same reason — the action must complete (and the page must settle) before the recording fires.

### `record-step` kinds and payload shapes

Four trigger kinds; `flush` translates each row into a scenario/2 step.

| Kind         | Payload shape                                                              |
|--------------|----------------------------------------------------------------------------|
| `navigation` | `{"route":"<url>"}`                                                        |
| `action`     | `{"method":"<m>","args":[…],"intent":"<optional>"}`                          |
| `wait`       | `{"condition":{"kind":"duration\|selector\|selectorAbsent\|text\|url",…}}`     |
| `assert`     | `{"kind":"present\|absent\|url","args":[…],"intent":"<required>"}`             |

**Allow-listed action methods:** `clickRole`, `clickByText`, `clickByLabel`, `clickSelector`, `fillByLabel`, `fillBySelector`, `pressKey`, `submit`, `selectByRole`, `scrollIntoViewByText`, `navigate`. Anything else fails at record time.

**Allow-listed assert kinds:** `present`, `absent`, `url`. (Replay-side dispatch for `text`/`count`/`value` isn't wired yet — they're refused at the boundary to avoid emitting self-failing scenarios.)

**Examples:**

```bash
agent-qa record-step navigation '{"route":"https://example.com/"}'
agent-qa record-step action     '{"method":"clickRole","args":["link","Sign in"]}'
agent-qa record-step action     '{"method":"fillByLabel","args":["Email","a@b.com"]}'
agent-qa record-step wait       '{"condition":{"kind":"url","pattern":"/dashboard"}}'
agent-qa record-step assert     '{"kind":"url","args":["/dashboard"],"intent":"signed in"}'
```

Prefer `agent-qa smart-click "<accessible name>"` over hand-rolled `clickRole` payloads — it records the step AND runs the click in one go, with built-in retry/heal hooks.

## Reading non-zero exits — NEVER pipe through `| tail -10`

If a `agent-qa <verb>` (or `yarn agent-qa <verb>`) command exits non-zero with an empty `Output:` block in yarn's footer, do NOT trust that "no output" means "no information". yarn classic's "Command failed" footer is ~10 lines on its own; if you piped through `| tail -10` (or any small tail) you cut off the actual error / stack trace.

```bash
agent-qa smart-click "Add user" 2>&1 | tail -10
# → "error Command failed with exit code 1" + "Output:" (empty)
```

The verb's stack trace IS in the stream — it's just above yarn's footer. **Re-run without the pipe** (or use `tail -200`) to read the real error:

```bash
agent-qa smart-click "Add user" 2>&1
```

When `smart-click` (or any verb that issues CDP eval calls) fails with an internal error mid-poll, the click syscall has typically already fired in the browser — **snapshot the page first** before retrying. If state changed, treat the click as successful and proceed with `record-step`; do NOT blindly re-click. `smart-click` exits code **4** specifically for "internal error during verification" with a structured message and a recovery hint; codes 1/3 are "click landed but verification disagreed" / "submit rejected"; code **5** is `not-found` / `ambiguous` / `ref-mismatch` (the verb refused to click — re-snapshot, fix the args, retry).

**The same `tail` rule applies to OK-exit verbs.** `agent-qa verify` prints the OK status as the FIRST line followed by 2 path lines, so `| tail -2` HIDES the OK / FAIL verdict. `agent-qa flush` prints a JSON summary then a path line. `agent-qa list` is variable-length. **Default to no tail; if you must tail, use `-200` or read the verb's known line count first.** Skill files for each verb document their output shape — check before trimming.

## Prerequisites

- A live `agent-browser` session pointed at the target app. If the scenario starts after login, use a registered authenticated profile: run `agent-qa profile-list`, then `agent-qa profile-status <profile>`, and bootstrap only registered profiles with `agent-qa profile-bootstrap <profile>` when needed. If the scenario itself starts at a public login page, do **not** bootstrap a profile; open the URL and record the username/password/login gestures as ordinary steps. `agent-browser doctor` lists every live daemon under `Daemons`; if a session is already running, REUSE IT.
- If `profile-list` says no profiles are registered, do not run `profile-bootstrap default-user`. Either record from the public entry URL the user gave you, or ask before creating a profile. `profile-bootstrap` requires a prior `profile-add`/plugin registration.
- Known target routes — if a route-catalog skill is registered (e.g. a vendor `pages` skill via `[skills] extra-dirs`), load `agent-qa skills get pages` and use the registered URLs instead of guessing. If your target route isn't in that catalog — or no such skill is registered — just use the URL the user gave you and move on.

## What ships with agent-qa

- **`agent-browser` is a hard dep**, pinned in `package.json`. The native binary is resolved from `node_modules/agent-browser/bin/` — agent-qa **ignores `$PATH`**. Don't `npm install -g agent-browser` separately; bumping agent-qa's pin is the only way to change the bound version. If a verb fails preflight ("agent-browser package not found"), reinstall agent-qa.
- **React DevTools hook is on by default.** `applyAgentBrowserEnv` in `cli/src/start.rs` adds `react-devtools` to `AGENT_BROWSER_ENABLE`, so the hook installs on first launch. This means `agent-browser react tree --json` / `react inspect <id> --json` / `react renders start|stop` / `react suspense` work on every the target app page out of the box. Harmless on non-React pages — the hook just registers `window.__REACT_DEVTOOLS_GLOBAL_HOOK__` with zero renderers.
- **Some apps run TWO React renderers** (React 17.0.2 PROD + React 18.3.1 DEV side-by-side). `react tree` returns the PROD tree; if you need the DEV one, filter by renderer id. Component names match `displayName ?? type.name`.
- **`react tree` text mode is broken** on agent-browser 0.27.0 — exits "✓ Done" with empty body. **Always use `--json`.**
- **`react inspect <id> --json`** returns `{source, text}`. `source` is `[file, line, col]`. `text` is a React-DevTools-style multi-line string with: prop names + primitive values (`taskQueueReady: true`), top-level object keys (`ownerState: {variant: "body1", ...}`), hook names + subscription counts (`UserValue: undefined (3 sub)`), useState primitive values (`[44] State: false`), and the full `rendered by: Header > Layout > Main > …` ancestry chain. **What's NOT in there**: deep object/array contents (`{...}` / `[{…}]` truncation) and hook values (always show as `undefined`, even when they aren't). Enough for structural + shallow-primitive assertions; for deep data-layer values, drive `agent-browser eval` with a custom fiber walker.
- **`agent-qa perf-snapshot [--sid <sid>] [--profile <p>]`** — opt-in performance sidecar (verb is in the `inspect` group). Writes to `<sid>/perf/`: `vitals.json` (Core Web Vitals + React hydration phases), `suspense.json` (boundary classifier). With `--record-renders <ms>` adds `renders.json` (React commit profile). With `--cpu-profile <ms>` adds `cpu.cpuprofile` (Chrome V8 sampling profile, openable in DevTools Performance tab). With `--trace <ms>` adds `trace.json` (Chrome timeline, also openable in DevTools). **`--cpu-profile` and `--trace` are mutually exclusive** — Chrome only allows one tracing/profiling session at a time. Use cases: "debug perf on /sequences" (no flags), "profile this flow" (`--record-renders 5000 --cpu-profile 5000`). Recording verbs do NOT call `perf-snapshot` — perf is orthogonal to scenario content. Full reference: [`references/perf-snapshot.md`](./references/perf-snapshot.md).
- **`agent-qa replay <sid | path-to-scenario.json> [--profile <p>] [--session <name>] [--param <name>=<value> ...]`** — active replay CLI. Walks the scenario end-to-end against a live agent-browser session. Single-profile replay is supported via `--profile`; use `compare --profiles ...` for serial profile comparison. `--param` (alias `-p`) overrides a scenario-declared parameter at replay time (repeatable; coerced per declared `type`; sensitive values redacted in `replay.json` + stderr; duplicate keys are last-wins). The CLI writes its current replay sidecars/manifest under `<scenarioDir>/replays/<replayId>`; the internal v1 runner/evidence helpers use stepId-keyed paths, but the public replay verb is not yet fully wired to that layout. Exit 0 = all steps passed, 1 = a step failed, 2 = bad args / schema validation / undeclared `--param` / type mismatch. Full reference: [`references/replay.md`](./references/replay.md).

## Setup (once per session)

```bash
agent-qa profile-bootstrap default-user        # one call: probe → auth → policy → DEVTOOLS_SETTINGS
agent-qa profile-status default-user           # confirm: should print 'authenticated'
```

Success signal: bootstrap ends with `[bootstrap:default-user] ready (...)`; `profile-status` prints `authenticated`.

**Stderr line `[agent-browser] orphan daemon detected for session=<s>; running 'agent-browser close --session <s>' and retrying once` is the auto-recovery firing — NOT an error.** Every shell-out to `agent-browser` goes through a single chokepoint (`cli/src/browser.rs`) that detects the "daemon PID alive, child Chrome dead" failure (stderr matches `All CDP discovery methods failed` + `Auto-launch failed:`), runs `agent-browser close --session <s>`, and retries once. If the retry succeeds, the verb proceeds normally — keep going. If both attempts fail, see [`references/gotchas.md`](./references/gotchas.md) for the manual fallback ladder (`close --all` → `doctor --fix` → hand-clean sidecars). Opt out with `AGENT_QA_AGENT_BROWSER_NO_AUTO_RECOVER=1`.

## Runbook (the canonical recording loop)

```bash
# 1. Start (mints a scenario id, primes sidecars).
#    Setup/teardown setup (seed N entities, flip a feature flag, run a gql
#    mutation before step 0 / after the run) lives inline in `scenario.json` as
#    `setup` / `teardown`. The phase 7 cutover removed `--prep`.
SESSION=default-user-session
SID=$(agent-qa start "sign in and assert the dashboard loads" --session "$SESSION")

# 2. Navigate to the entry route. `record-step navigation` records the
#    intent; `agent-browser open` actually drives the tab.
agent-browser --session "$SESSION" open 'https://example.com/'
agent-qa record-step navigation '{"route":"https://example.com/"}'

# 3. Drive the form ONE GESTURE AT A TIME, recording after each.
#    smart-click both drives and records when role+name resolves cleanly.
agent-qa smart-click "Sign in"

# Unique newly-created values: use fill-unique. The template MUST contain {{vars._unique}}.
agent-qa fill-unique "Email" --template "qa-{{vars._unique}}@example.com" --save-as email

# Fixed credentials / existing values: drive the fill, then record a literal fillByLabel step.
agent-browser --session "$SESSION" fill "Password" "hunter2"
agent-qa record-step action '{"method":"fillByLabel","args":["Password","hunter2"]}'

agent-qa smart-click "Submit"     # the submit

# When smart-click can't resolve the target (e.g. role+name lookup misses
# on an aria-label only element), fall back to: snapshot to get the ref,
# drive with agent-browser, then record:
#   REF=$(agent-browser --session $S snapshot | awk '/link "Open menu"/{print}' | sed -E 's/.*ref=(e[0-9]+).*/\1/')
#   agent-browser --session $S click $REF
#   agent-qa record-step action '{"method":"clickRole","args":["link","Open menu"]}'

# 4. Assert the contract you set in `start`. See references/asserts.md.
agent-qa record-step assert \
  '{"kind":"url","args":["/dashboard"],"intent":"signed in and dashboard loaded"}'

# 5. Finalise + verify.
agent-qa flush
agent-qa verify          # post-flush sanity: 1 keyframe per step, all status=ok
agent-qa list            # pretty step→keyframe table; writes summary.md
```

## ALWAYS surface a complete closing summary to the user

Once a recording is `verify`-clean, your final response to the user
MUST include ALL of the following, in this order. Don't make the user
ask follow-ups for any of it.

### Required closing sections

1. **What the scenario proved** — one sentence stating the assertion
   that passed and the page/role/name it matched on. Example: "The
   `Add user` button is present on `/users` (role=button,
   name=\"Add user\")."
2. **SID** — the full scenario id on its own line.
3. **Where it lives on disk** — the absolute path to `scenario.json`
   (`tmp/agent-qa-scenarios/<SID>/scenario.json` or whatever
   `AGENT_QA_SCENARIOS_DIR` resolves to). Users open this file to read
   the recorded steps.
4. **Step recap** — bullet list, one line per step, in order. Format:
   `<n>. <kind> — <one-line description>`. This is what the user
   actually inspects to decide if the recording matches their intent.
 5. **Replay commands** — the block below, **WITH the inline comments
    intact**. The comments explain what each verb does; stripping them
    defeats the point. **Pick the profile flag from what's actually
    registered:** run `agent-qa profile-list` — if it lists profiles,
    use the one marked default (and mention the others as alternatives
    the user can swap in); if nothing is registered, drop `--profile`
    entirely (the runner uses its built-in default profile). Never
    hardcode a vendor profile name the user's setup doesn't have.

```bash
agent-qa replay <SID>                        # re-run the recorded flow on a fresh tab; writes the active replay sidecars under replays/<runId>/. Add --profile <name> if profile-list shows one.
agent-qa compare <SID>                       # 1:1 (or N-way) per-pair diff: ARIA snapshot + screenshot pixels. Writes <sid>/compare/<TS>__<labels>/{compare.md,compare.json,pairs/...}. Exits non-zero on any delta. --include snapshot,screenshot to scope; --pixel-threshold <0..1> (default 0) for pixel sensitivity. `diff` is a permanent alias.
agent-qa list <SID>                          # show what's in the scenario: every step, its keyframe, plus any past replays. Read-only inspector; writes <SID>/summary.md for humans
```

### Profile / setup caveats

If `agent-qa profile-list` shows registered profiles, name the default one
in the replay command and list the rest as swap-in alternatives (`--profile
<name>`). If nothing is registered, leave `--profile` off — the runner uses
its built-in default profile. If the scenario declares `setup` / `teardown`
(featureFlags, gql, cookies, localStorage, nav), the runner applies those
hooks automatically — no extra flag, no external file.

### Caveats NOT to surface

The framework handles the following automatically; flagging them as
"future-replay-fragile" in your closing summary is noise that wastes
the user's attention on a non-issue.

- **Numeric-suffix drift in accessible names** (e.g. `option "Submit
19987 items pending"`, `row "Item 654321"`, `text "Parent
abcdefgh"` from a generated suffix). The replay-side `clickRole` dispatcher
  has digit-tolerant + alphanumeric-suffix matchers — look for
  `[replay] digit-tolerant match: ...` / `[replay] generated-suffix
match: ...` lines in replay logs. The scenario is fine as recorded.
  Only flag accessible-name drift when the changed characters are
  NOT digits or generated suffixes (e.g. an entire word changed, i18n
  flipped). Full reference: `gotchas.md`. If the recorded name IS
  load-bearing for the test (clicking a different row is a false
  positive), set `heal: { "mode": "off" }` on the locator — see
  `references/heal-opt-out.md`.

### Why all five sections

A bare list of three commands tells the user **how** to interact with
the artefact but not **what they have**. Without (1) they don't know
what was asserted; without (3) they can't open the file; without (4)
they can't audit the recording without running `list`. The terse
"verified clean + SID + commands" form is a regression — don't ship
it.

For the multi-profile flow (`compare` across two registered profiles), see `agent-qa skills get profiles`.

## Success criteria

A scenario is "done" when:

1. `agent-qa verify` exits 0 with `OK (strict) — N steps, all keyframes status:ok`.
2. The on-disk `scenario.json` matches the user's intent (read it back, scan the steps).
3. The user-stated assertion (passed to `start "<instruction>"`) is captured as an `assert` step at the end of `scenario.steps[]` and reproduces on a fresh replay (`agent-qa replay $SID && agent-qa compare`, adding `--profile <name>` if `profile-list` shows one).
4. **Your final message to the user includes the full closing summary** (see "ALWAYS surface a complete closing summary to the user" above): assertion proved, SID, on-disk path, step recap, AND replay commands with inline comments. A bare "SID + 3 commands" message is a regression.

If any of those is false, you're not done — go heal (see [`references/heal.md`](references/heal.md)) or re-record.

## Deeper references (load with `--full`)

`agent-qa skills get core --full` returns SKILL.md plus every file below. Or load one file at a time from `skill-data/core/references/`:

| File                   | Covers                                                                                                                                                                                                                                                                                                                                    |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `verbs.md`             | Per-verb syntax, exit codes, options. The full reference for the table at the top of this file.                                                                                                                                                                                                                                           |
| `inspect.md`           | **Inspection mode** — use agent-qa as a live-page debugger/validator (perf, React tree, network, ARIA, asserts) without producing a scenario. Most commonly paired with `--byo`. The right starting point when the user's ask is debug/validate, not record/replay.                                                                        |
| `compare.md`           | The unified `compare` verb (and `diff` alias). 1:1 (recording vs replay) and N-way (cross-profile / cross-replay) per-pair diff with two subsystems (ARIA snapshot + screenshot pixels). Output layout under `<sid>/compare/<TS>__<labels>/`, exit codes, replay-reuse policy. |
| `heal.md`              | The canonical heal loop — replay-side locator-drift heal, `heal-respond` for caller-driven value corrections, `heal-promote` to bake locator hints. Start here for any non-OK outcome.                                                                                                                                                    |
| `heal-apply.md`        | `heal-apply` — record-time in-place patch of a recorded value template after `heal-respond`; never touches the live tab.                                                                                                                                                                                                                  |
| `recovery.md`          | The record-time recovery flow: `truncate <N>` for disk bookkeeping plus the agent-browser tab gestures the agent drives to re-position the page.                                                                                                                                                                                          |
| `heal-opt-out.md`      | Per-locator `heal: { "mode": "off" }` — opt a single `Locator` out of the resolver's drift-tolerant tiers (digit-tolerant, generated-suffix). Use when the recorded accessible name is load-bearing and a drifted match would be a false positive.                                                                                          |
| `asserts.md`           | All assert kinds (`url`, `present`, `absent`, `text`, `count`, `value`), the determinism-ordered ladder, exit codes, the bind-required rule for DOM-identity asserts.                                                                                                                                                                     |
| `unique-tokens.md`     | The `{{vars._unique}}` template grammar and `--save-as <name>` for downstream substitution.                                                                                                                                                                                                                                               |
| `prep.md`              | Scenario/v1 root `setup` / `teardown`: GraphQL fixtures, feature flags, cookies, localStorage, nav, and named bindings.                                                                                                                                                                                                                    |
| `schema.md`            | The scenario/v1 shape, control-flow step types (`if` / `while` / `forEach` / `group` / `runTemplate`) and where to find canonical sources (`schema/scenario-schema.json`, `cli/src/scenario.rs`). Read before hand-editing a scenario or building a template.                                                                                 |
| `anatomy.md`           | Short historical redirect for the removed external prep-file anatomy. Use `prep.md` and `scenario-authoring.md` for current scenario/v1 root setup/teardown guidance.                                                                                                                                                                       |
| `scenario-authoring.md` | Short v1 authoring redirect: keep seeded state in root `setup` / `teardown`, keep browser work in `steps`, and use `prep.md` for the current contract.                                                                                                                                                                                    |
| `gotchas.md`           | The list of things that will bite you exactly once each.                                                                                                                                                                                                                                                                                  |

## When this skill does NOT apply

- The user wants a one-off page capture, not a recording → `agent-browser snapshot -i` + the codegen pipeline, not `agent-qa`.
- The user wants to debug a single keyframe — see `agent-browser` skill, not this one.
- The user wants to author or edit a `scenario.json` by hand — that's a contract violation; the recorder is the only way to produce one.
