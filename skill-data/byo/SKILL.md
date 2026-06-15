---
name: byo
description: BYO (bring-your-own) browser mode — drive the user's real Chromium-based browser (Chrome, Brave, Brave Nightly, Edge, Arc, Vivaldi, etc.) instead of a canned QA profile. Use ONLY when the user explicitly asked, in the current turn, to drive their own browser/session. Never default into this mode; never infer it from context.
---

# byo — drive a user-owned browser session

> **HARD RULE (R2). Read this paragraph before ANY tool call.**
>
> Before driving the user's real browser by ANY means — `agent-qa <verb> --byo`, `agent-qa <verb> --byo --launch <vendor>`, OR raw `agent-browser --cdp <port>` against a port the user owns — you MUST post ONE R2 prompt and WAIT for an explicit approval / choice:
>
> > Base prompt when exactly one safe action exists: "About to drive your real browser via BYO. Doctor says: \<paste the relevant lines from `agent-qa byo-doctor`\>. Confirm?"
>
> If the host supports structured choices (for example OpenCode's `question` tool), the R2 prompt MUST use that UI even when there is exactly one safe drive action: present an approve option and a stop/cancel option. Plain-text `Confirm?` is only a fallback for hosts without structured choice UI.
>
> If the doctor output exposes an unresolved choice (no attachable browser, multiple attachable browsers, multiple tabs, requested vendor already running, `--clone-profile` needed, different launch vendors available, etc.), DO NOT ask the bare confirmation first. Combine the safety confirmation and the resolution request in one prompt: summarize the doctor facts, list the allowed options, and tell the user that choosing one drive option is explicit consent for that option. The user's reply must name the option / vendor / port / tab / clone choice; a generic "yes" is only enough when there is exactly one safe action and the host has no structured choice UI.
>
> This is required **every single user turn**, even if the user used BYO five minutes ago. The confirmation is not a courtesy — it is the only thing standing between the agent and the user's real browser session. There is no exception for "obvious" cases, no exception for "they already said yes once today," no exception for `--launch` (which is MORE dangerous than `--byo` alone because it spawns a process with the user's profile dir), and **no exception for "I've already attached, this is just a follow-up `reload` / `open` / `click`."** Every new user turn that wants to drive their browser resets the consent clock.
>
> ### Per-turn re-consent: what counts as "driving" vs "observing"
>
> Within a single confirmed turn, ALL agent-browser verbs against the user's port are permitted. The next user turn STARTS WITHOUT consent. In the new turn:
>
> | Verb shape against user's browser                                                 | Needs fresh R2?                                           |
> | --------------------------------------------------------------------------------- | --------------------------------------------------------- |
> | `agent-browser --cdp <port> snapshot` / `react tree --json` / `eval <read-only>`  | **NO** — read-only inspection is safe; just don't mutate. |
> | `agent-browser --cdp <port> network har start/stop` / `network requests`          | **NO** — passive observers.                               |
> | `agent-browser --cdp <port> click` / `fill` / `open` / `reload` / `eval <writes>` | **YES** — these mutate page state.                        |
> | `agent-qa <verb> --byo` / `agent-qa <verb> --byo --launch <vendor>`               | **YES** — always.                                         |
>
> "Read-only" means: the eval / inspect call cannot mutate page state, cannot trigger a navigation, cannot fire a fetch, and cannot rewrite DOM. If you're unsure, treat it as mutating and ask.
>
> If the user's message is even slightly ambiguous about whether they mean BYO, ASK. Do not guess. The standard managed-session flow (no `--byo`) is always the safe fallback.

## When this skill applies

Only when the current-turn user message explicitly references their own browser. Examples:

| Message                                      | Apply this skill? |
| -------------------------------------------- | ----------------- |
| "use my own browser"                         | yes               |
| "byo"                                        | yes               |
| "attach to my Chrome"                        | yes               |
| "test it in my Brave Nightly"                | yes               |
| "use the QA admin profile"                   | NO                |
| "the bug only happens for me — reproduce it" | ASK — ambiguous   |
| "run agent-qa against the sequences page"    | NO                |
| (no mention of browser/session at all)       | NO                |

When in doubt: ask. The standard managed-session flow (no `--byo`) is always the safe fallback.


## The flow (every BYO turn)

0. **MANDATORY PRE-FLIGHT — `agent-browser close --all`.** This is not optional and not a diagnostic-only step. `agent-browser` keeps `default` + named persistent sessions alive across CLI invocations; any of them can hijack a `--cdp` call to the user's port and silently drive a different tab (often with stale auth cookies that redirect a fresh nav to `login.*` etc.). Close them BEFORE reading the doctor, every turn. There is no downside — agent-qa verbs re-establish their own sessions on demand. **If you skip step 0 and the user says "I don't see it", you skipped step 0.**
1. Run `agent-qa byo-doctor` (read-only — no risk).
2. Decide whether the doctor leaves exactly one safe action or requires a choice.
3. Post ONE R2 prompt with the doctor's relevant lines pasted in.
4. If exactly one safe action exists, use structured choices when available: one approve option plus one stop option. If a choice is required, list the allowed options in user-meaningful language and state that picking one option is consent for that option.
5. WAIT for explicit yes / option. Anything vague -> abort, ask again with the same option list.
6. Run the requested verb with `--byo` (and `--launch <vendor>` / `--port <n>` / `--tab <id>` / `--clone-profile` only if the user explicitly picked that exact option).
7. Read the safety banner that prints on stderr — confirm the browser/page identified there matches what the user expected before proceeding to the next verb.
8. **Post-drive sanity check.** After every `open` / `click` / `fill` / `reload` against the user's port, run `agent-browser --cdp <port> tab list` and confirm the listed URL matches what you just drove. If it doesn't (e.g. you opened `https://app.example.com/...` and the listed URL is `https://login.example.com/...` on a tab that the doctor previously said was `chrome://newtab/`), the call was hijacked by a stale persistent session. Go back to step 0, then redo from step 6.

Every subsequent BYO verb invocation in the same turn ALSO prints the banner. Don't suppress it. Don't pipe stderr to /dev/null.

### Tripwire — "user says they don't see the page"

If at any point in a BYO turn the user reports they cannot see the page you claim to have driven:

1. DO NOT speculate about window focus, Dock, Mission Control, or which profile the user is "really" looking at.
2. Run `agent-browser --cdp <port> tab list` first — if the URL doesn't match what you drove, this is the stale-session hijack (see step 8 above and "Diagnosing 'agent-browser disagrees with the visible tab'" below).
3. Run `agent-browser close --all` and redo the drive from step 6. This resolves it >90% of the time.

### Use structured choice UI when available

When your agent host supports a structured question / choice UI (for example OpenCode's `question` tool), use it for EVERY R2 prompt. Do not print a plain-text `Confirm?` and wait for a free-form reply if a clickable choice UI is available.

Choice UI requirements:

- The question text must include the BYO safety summary, relevant doctor facts, and the target URL/action.
- Option labels must be human-readable decisions, not raw ids. Good: `Use blank New Tab (Recommended)`. Bad: `FD400182`.
- Option descriptions must include the raw id/command and the consequence. Example: `Tab id FD400182, currently chrome://newtab/. Opens the target URL here without navigating existing work.`
- Put the safest option first and label it `(Recommended)` when there is one.
- Include a stop/cancel option when the user might reasonably decline or relaunch manually.

For exactly one safe action, the choice UI still has at least two options:

| Label                | Description                                                            |
| -------------------- | ---------------------------------------------------------------------- |
| Drive this Brave tab | Tab id `12999664`, currently `about:blank`. Opens the target URL here. |
| Stop here            | Do not drive BYO in this turn.                                         |

Do not emit this pattern in OpenCode or any host with structured choices:

```text
About to drive your real browser via BYO... Confirm?
Awaiting BYO confirmation before driving your Brave Nightly tab.
```

That plain-text hold pattern hides the clickable UI and gives the user no clear action surface.

Example OpenCode-style choices for a multi-tab attach:

| Label                           | Description                                                                                  |
| ------------------------------- | -------------------------------------------------------------------------------------------- |
| Use blank New Tab (Recommended) | Tab id `FD400182`, currently `chrome://newtab/`. Opens the target URL without touching work. |
| Reuse the target app tab         | Tab id `ED0279A1`, currently `/home?...`. This will navigate that tab away.                  |
| Launch cloned Brave instead     | Uses `--launch brave-nightly --clone-profile`; leaves existing tabs untouched.               |
| Stop here                       | Do not drive BYO in this turn.                                                               |

Only fall back to a plain text prompt when the host has no structured choice UI.

## Two modes

BYO is a **bridge**, not a recording-only mode. Once attached, two things you can do:

- **Inspect / debug / validate** a live page — most common BYO use. No scenario produced. See "What to do once attached" below.
- **Record a scenario** against the user's own session — useful when reproducing a flow the canned QA profiles can't reach (custom org settings, in-progress features under a flag the user has flipped locally, etc.).

### Attach (`--byo` alone)

Attaches to an already-running Chromium-based browser that exposes CDP on ports 9222–9229 (or `AGENT_QA_BYO_CDP_PORT`). The user is responsible for having launched that browser themselves:

```bash
"/Applications/Brave Browser Nightly.app/Contents/MacOS/Brave Browser Nightly" --remote-debugging-port=9222 --remote-allow-origins='*'
# then, after R2 confirmation:
agent-qa start "<instruction>" --byo
agent-qa replay <sid> --byo
```

Attach is **browser-agnostic** — works for any Chromium-based browser the user starts with `--remote-debugging-port`.

Use attach when the user has already started their real browser with BOTH `--remote-debugging-port=<port>` and `--remote-allow-origins='*'`, and the doctor shows it as attachable. If the user's Brave/Chrome/etc. is running normally without CDP, attach is not available.

### Attach with multiple tabs (`--tab <id>`)

When the chosen CDP browser has multiple tabs, the user must choose which tab to drive. Do NOT ask using only raw target ids like `ED0279A1` / `FD400182`; those names mean nothing to a human. Translate each option into the consequence:

1. **Use the blank/new tab (Recommended)** — e.g. `New Tab (id FD400182, currently chrome://newtab/)`. This is usually safest for `--open <url>` because it does not navigate away from the user's existing work.
2. **Reuse an existing app tab** — e.g. `Existing the target app tab (id ED0279A1, currently /home?tasksHome=...)`. Say clearly that this will navigate that tab away from its current page.

Good R2 wording:

```text
About to drive your Brave Nightly via BYO. Doctor says port 9223 has two tabs. Target URL will be https://app.example.com/?deployment=Staging. Choose one:
1. Use the blank New Tab (recommended) — id FD400182, currently chrome://newtab/.
2. Reuse the existing the target app tab — id ED0279A1, currently /home?...; this will navigate it away.
Reply with 1 / 2, or the tab id. Choosing one is consent for me to drive that tab.
```

Bad R2 wording:

```text
Choose ED0279A1 or FD400182.
```

If there is a blank tab and an existing work tab, recommend the blank tab first. If all tabs contain user work, say that explicitly and offer launch/clone as a safer option instead of forcing a tab id choice.

If you need to send a follow-up "still waiting" message after the R2 prompt, keep the same human labels. Do NOT collapse it to raw target ids.

Good waiting message:

```text
Waiting for your BYO tab choice:
1. Use the blank New Tab (recommended) — id FD400182.
2. Reuse the existing the target app tab — id ED0279A1; this will navigate it away.
```

Bad waiting message:

```text
Awaiting BYO consent and tab choice: reply with ED0279A1 or FD400182.
```

### Launch (`--byo --launch <vendor>`)

When `byo-doctor` shows no attachable CDP browser, or when the user explicitly chooses to launch a fresh browser, the user can opt agent-qa into spawning one. This is more dangerous than attach because agent-qa starts a process using the user's real user-data-dir (cookies, sessions, history). Only use after explicit R2 confirmation that NAMES the vendor:

```bash
# After: "yes, launch brave-nightly"
agent-qa start "<instruction>" --byo --launch brave-nightly
```

Vendor ids: `chrome`, `chrome-canary`, `brave`, `brave-nightly`, `edge`, `arc`, `vivaldi`. macOS only in v1.

`--launch <vendor>` means launch that vendor. It must not silently attach to any existing CDP browser, even if one appears between the doctor check and the command. If the user approved "clone Brave Nightly", run `--launch brave-nightly --clone-profile`; do not attach to Chrome, Vivaldi, or an already-open Brave CDP tab unless the user picked that attach option.

### Launch with conflict (`--clone-profile`)

If the requested vendor is already running, Chromium refuses to start a second instance against the same user-data-dir. `byo-doctor` flags this as `RUNNING (would conflict — needs --clone-profile)`. Three resolutions (ask the user which):

1. Have the user quit the running vendor, then retry.
2. Add `--clone-profile` to spawn a parallel copy with the user-data-dir cloned (auth cookies copied at clone time, the user's real browser untouched).
3. Pick a different vendor.

Never pick option 2 silently — it changes the meaning of "drive my real Brave" to "drive a clone of my Brave that won't see new logins or shared storage."

Ask for that resolution in the same R2 prompt that requests consent. Bad: "Confirm?" then user says yes, then ask which resolution. Good: "Doctor says Brave Nightly is installed but already running and no CDP browser is attachable. To drive BYO, choose one: quit Brave Nightly and relaunch it with CDP yourself; launch a cloned Brave Nightly with `--launch brave-nightly --clone-profile`; or pick another installed vendor. Reply with the option you approve; choosing the clone option is consent for me to drive that cloned browser."

## What to do once attached

BYO unlocks `agent-browser` primitives against the user's real tab. Recording is ONE option, not the only one. Common debug/validate workflows — pick the closest match to what the user asked for:

| User asked                                       | Right tool                                                                                                                                                           |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| "what network requests fire on reload?"          | `agent-browser network har start <path>` → reload → `network har stop` → parse with `node`/`jq`. Or `network requests --filter <substr>` for a quick read.           |
| "why is this page slow?"                         | `agent-qa perf-snapshot --byo` for Core Web Vitals + Suspense classifier. Add `--cpu-profile 5000` or `--trace 5000` for a Chrome timeline you can open in DevTools. |
| "what React component renders X?"                | `agent-browser react tree --json` (text mode is broken, ALWAYS use `--json`), grep the result, then `react inspect <id> --json` for props/hooks/source.              |
| "what's the ARIA snapshot of this page?"         | `agent-browser snapshot` or `snapshot -i` for the interactive picker.                                                                                                |
| "validate the live page looks right"             | Either compare an `agent-browser snapshot` against a known-good fixture, OR record a one-step scenario ending in an `assert` (`agent-qa start "verify X" --byo`).     |
| "what GraphQL ops fire for this flow?"           | `agent-browser network requests --filter graphql` (run, do the action, run again with `--clear` between).                                                            |
| "reproduce this bug in my session"               | Drive the gestures via `agent-browser open`/`click`/`fill` directly; no scenario needed unless the user wants a replayable artefact.                                  |
| "check which tab is focused on the attached CDP" | `agent-browser tab list` (or `tab` for the active one). With multi-tab BYO browsers the doctor only reports one URL; confirm tab before driving.                     |

If the user later says "now turn this into a regression test" — THAT's when you switch to `agent-qa start "<instruction>" --byo` and follow the recording loop from `agent-qa skills get core`. Don't pre-emptively record; recording only when there's a contract worth replaying.

## Hard rules

- **Never default**, never persist across turns, never include in `compare --profiles all` (impossible by construction — BYO sessions aren't in `~/.agent-qa/chrome/`).
- **Never combine `--byo` with `--profile` or `--session`** — the CLI rejects it; do not work around.
- **Never invoke `--launch <vendor>` without an explicit `--launch <vendor>` ack from the user in this turn.**
- **Never substitute attach for an approved launch.** Existing CDP attach and new browser launch are different user choices.
- **Re-read the banner every verb.** If `current URL` or `browser` doesn't match what the user expected, STOP and re-confirm.
- **No env-var binding.** `AGENT_QA_BYO_CDP_PORT` is the only BYO-related env var and it only adds an extra probe port — it does not activate BYO mode.

## Diagnosing "agent-browser disagrees with the visible tab"

Symptom: `agent-browser --cdp <X> eval`/`open`/`click` succeeds, but the visible `--cdp` tab is unaffected, OR `eval` returns a different `location.href` than the address bar.

There are two real causes and one phantom. Walk this checklist BEFORE blaming Chromium, DevTools, agent-browser, or filing an upstream issue:

1. **Stale `agent-browser` persistent session hijacking `--cdp` (most common — bites hard).** `agent-browser` keeps `default` and named sessions alive across CLI invocations. Without an explicit `--session <fresh>`, commands can route through the persistent session instead of the `--cdp` endpoint you named. **First diagnostic step: `agent-browser close --all`, then retry.** If the disagreement vanishes, that was it; stop. Do NOT proceed to (2) or (3) until you've ruled this out.
2. **Missing `--remote-allow-origins=*`** at browser launch — `byo-doctor` flags this as `wsHandshakeBlocked`. Real Chromium ≥ 111 requirement. `byo-doctor --launch` adds it automatically; manual launches must include it. If doctor's ⚠️ block fires, fix this before anything else.
3. **`/json/list[0]` is not pinned.** Chromium reorders entries by focus state; "the first page target" can change between two queries without any actual divergence. When asserting that two reads agree, compare against a **known target id**, not against `/json/list[0]`.

DevTools being open on the inspected tab is **NOT** a cause. Verified with both Brave Nightly and regular Chrome: DevTools attached does not affect `agent-browser --cdp` correctness as long as (1) and (2) above are clean. An earlier debugging session mistakenly blamed DevTools and shipped a refuse-to-attach branch; that was reverted in the same day.

## What's not in BYO

- No bootstrap (no OAuth, no network policy, no overrides). Your real browser has whatever state it has.
- No safe-mode / read-only / mutation refusal. Once the user confirmed, mutating verbs (`smart-click`, `fill-unique`, `heal-respond`, `heal-promote`, `replay` actions) run normally. The R2 confirmation IS the safety layer; we don't double-gate.
- No cross-profile compare. BYO is single-session by nature; `compare` requires registered profiles.
- No profile session prefix. BYO sessions are labelled `byo-<pid>-<ts>` so they cannot be confused with canned profiles in logs.

## When this skill does NOT apply

- The user wants to record/replay against the canned QA profiles → `agent-qa skills get core` and `agent-qa skills get profiles`.
- The user wants to capture state from their browser for codegen → `agent-browser` skill (separate pipeline), not agent-qa.
- The user is debugging a flaky test against staging → the standard managed-session flow is faster and safer; only switch to BYO if they explicitly say so.
