# Inspection mode — debug or validate a live page without a scenario

agent-qa is two tools fused into one CLI. The dominant flow in `SKILL.md` is recording a scenario (replayable contract, audit trail, cross-profile compare). This reference covers the OTHER flow: pointing the same CDP plumbing at a live page to **debug or validate**, with no `scenario.json` produced.

Use this reference when the user's ask is "why is X slow", "what fires on reload", "what component renders this", "is the page in the right state right now", "snapshot the live ARIA tree" — i.e. anything that doesn't need a replayable artefact.

## When inspection mode applies (vs scenario mode)

| Symptom of the ask                                             | Mode                                                                        |
| -------------------------------------------------------------- | --------------------------------------------------------------------------- |
| "Record this so I can replay it later" / "make a scenario of X" | Scenario                                                                     |
| "Compare admin vs default user on this page"                   | Scenario (multi-profile replay)                                              |
| "Why does this break on staging?"                              | Inspection (probably; switch to Scenario if a regression test falls out)     |
| "What network calls fire when I reload?"                       | Inspection                                                                  |
| "What React component is responsible for this loader?"         | Inspection                                                                  |
| "Validate the page is in state X right now"                    | Inspection (or Scenario with one assert if user wants a regression artefact) |
| "Reproduce this bug in my own browser session"                 | Inspection (attach with `AGENT_BROWSER_CDP`; see the `byo` skill)          |

If you start in inspection and the user later says "turn this into a regression test", switch to Scenario mode with `agent-qa start "<instruction>" --session <name>` and follow the recording loop in `SKILL.md`. There is no `--byo`, `--launch`, `--port`, `--tab`, or `--clone-profile` flag.

## Drive the pinned browser

Every browser command in this reference goes through `agent-qa browser`. A bare `agent-browser` on `$PATH` can be a different version and will restart the session daemon mid-inspection. Pass `--session <name>` on each call. One session for the whole look.

```bash
agent-qa browser --session inspect open https://example.com/users
```

For an already-running Chrome, set `AGENT_BROWSER_CDP` and `AGENT_BROWSER_PIN_TAB=1` first, as the `byo` skill describes. Do not invent attachment flags.

## What to run

### agent-qa

| Verb | What it does |
| --- | --- |
| `agent-qa aria-snapshot --session <s>` | Structured picker rows for the live tree. `--interactive` limits the dump to controls. |
| `agent-qa perf-snapshot --session <s>` | Navigation and paint timings from `window.performance`. See [`perf-snapshot.md`](perf-snapshot.md). |
| `agent-qa byo-doctor` | Read-only list of attachable browsers. It does not drive one. |
| `agent-qa cdp-url --session <s>` | WebSocket endpoint of the live session. |

### agent-browser, via `agent-qa browser`

| Command | What it does |
| --- | --- |
| `snapshot -i -c` | Interactive, compact accessibility tree. Start here. |
| `snapshot --delta` | Full tree once, then `unchanged` or a structural delta on later calls in the same tab. Pass `--full` to refresh the baseline. Take a plain `snapshot` after a navigation; deltas are not scenario evidence. |
| `screenshot --annotate` | Screenshot whose `[N]` labels are snapshot refs `@eN`. Use it for icon buttons and other controls the tree does not name. `--if-changed` skips an unchanged image. |
| `react tree --json` / `react inspect <id> --json` | React component tree, then props, hooks, state, and source for one fiber. Prefer `--json`. |
| `network requests --filter <substr>` | Recent requests. `--clear` zeroes the buffer. |
| `network har start <path>` / `network har stop` | HAR around one reload. |
| `trace start [path]` / `trace stop` | Chrome DevTools trace. |
| `tab list` | Page targets. `tab <id>` switches. Run this when several tabs are open; a pinned session will not slide onto another tab. |
| `scrollintoview <sel>` | Bring a target into the viewport before `click` or `dblclick`. |
| `eval <expression>` | Expression form, not an arrow function. Wrap statements in `(() => { ... })()`. |
| `get url` | Where the tab is. Use it after `outcome_unknown` before repeating a navigation. |

Click and double-click fail when the target is hidden, disabled, covered, or outside the viewport. The error names a covering element. Dismiss it or scroll the target into view, then snapshot again before reusing refs. Refs survive same-document updates and are dropped on navigation. See [`gotchas.md`](gotchas.md) for `outcome_unknown` and `page_wedged`.

## Worked example — "why is the activity feed slow on reload?"

```bash
# 1. confirm tab focus
agent-qa browser --session inspect tab list
agent-qa browser --session inspect tab t2          # if needed

# 2. record HAR around the reload
agent-qa browser --session inspect network har start /tmp/activity.har
agent-qa browser --session inspect reload
agent-qa browser --session inspect wait --load load
agent-qa browser --session inspect network har stop

# 3. slice it
node -e '
  const h = JSON.parse(require("fs").readFileSync("/tmp/activity.har","utf8"));
  const t0 = Math.min(...h.log.entries.map(e => new Date(e.startedDateTime).getTime()));
  h.log.entries
    .filter(e => /ActivityFeed|graphql/.test(e.request.url))
    .forEach(e => console.log(`+${new Date(e.startedDateTime).getTime()-t0}ms dur=${Math.round(e.time)}ms ${e.response.status} ${e.request.url.split("/").pop()}`));
'

# 4. timings from the live tab
agent-qa perf-snapshot --session inspect

# 5. the loader component, when the page is a React app
agent-qa browser --session inspect react tree --json
agent-qa browser --session inspect react inspect <id> --json
```

No scenario was recorded. The HAR, perf JSON, and react inspect output are the artefacts. Hand them over as-is.

Reserve `wait --load networkidle` for a page that actually goes quiet. A feed that keeps a socket open will sit until the wait times out. Prefer `wait --load load`, a selector, or a URL.

## One-off snapshot of an attached browser

```bash
export AGENT_BROWSER_CDP=9223
export AGENT_BROWSER_PIN_TAB=1
agent-qa byo-doctor
agent-qa browser --session inspect open https://example.com/users
agent-qa browser --session inspect snapshot -i -c
```

`start` is not required. The snapshot is not a scenario sidecar.

## When inspection turns into a scenario

Two triggers:

1. The user explicitly says "now turn this into a regression test" / "record this so it doesn't break again."
2. You found a reproducible failure and want a contract that fails the next time it regresses.

Then `agent-qa start "<one-line assertion>" --session inspect` and follow the recording loop in `SKILL.md`. Keep the same session. Inspection artefacts are not converted.

## Don't

- Don't run `agent-qa start` for a debug-only ask. It mints a scenario id and a recorder buffer you will not flush.
- Don't pass `--byo`, `--launch`, `--port`, `--tab`, or `--clone-profile`. They are not flags.
- Don't call a bare `agent-browser`. Use `agent-qa browser`.
- Don't retry a click, fill, or check after a lost reply (`outcome_unknown`, or text `EOF while parsing a value`) until a snapshot or `get url` shows whether it landed. agent-qa retries `open` itself.
- Don't feed `snapshot --delta` or an annotated screenshot into a scenario. `record-step` captures its own full snapshot and full-page PNG.
- Don't raise `AGENT_BROWSER_DEFAULT_TIMEOUT` above `30000`.
