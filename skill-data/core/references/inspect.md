# Inspection mode — debug or validate a live page without a scenario

agent-qa is two tools fused into one CLI. The dominant flow in `SKILL.md` is recording a scenario (replayable contract, audit trail, cross-profile compare). This reference covers the OTHER flow: pointing the same CDP plumbing at a live page (most commonly via `--byo`) to **debug or validate**, with no `scenario.json` produced.

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
| "Reproduce this bug in my own browser session"                 | Inspection (BYO)                                                            |

If you start in inspection and the user later says "turn this into a regression test", that's the cue to switch to Scenario mode with `agent-qa start "<instruction>" --byo` and follow the recording loop in `SKILL.md`.

## Inspection-mode verbs

Three categories: agent-qa primitives, agent-browser primitives (always available when agent-qa is installed — `agent-browser` is a hard dep), and external tools you call on the artefacts.

### agent-qa primitives

| Verb                           | What it does                                                                                                                                                                                                                          |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `agent-qa inspect`             | Post-failure diagnostic ladder (rungs 1-4) — most useful after a recording fails, but also works standalone for a live page. Supports `--byo`, `--launch`, `--open <url>`, and `--snapshot` for one-off live-page inspection.         |
| `agent-qa perf-snapshot --byo` | Core Web Vitals + Suspense boundary classifier. Add `--record-renders <ms>` for a React commit profile, `--cpu-profile <ms>` for V8 sampling, `--trace <ms>` for a Chrome timeline. Writes to `<sid>/perf/` (or a tmp dir if no sid). |
| `agent-qa byo-doctor`          | Read-only enumeration of BYO state; safe to run any time.                                                                                                                                                                             |

### agent-browser primitives (always available when BYO-attached or against a registered profile session)

| Verb                                                          | What it does                                                                                                                                                                                                        |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `agent-browser snapshot [-i]`                                 | ARIA tree of the current page. `-i` is interactive (picker). Plain output is text; pipe through `--json` for structured.                                                                                            |
| `agent-browser react tree --json`                             | React component tree. **TEXT MODE IS BROKEN** on agent-browser 0.27.0 — always use `--json`. Parse the result; nodes have `{id, depth, parent, name, key}`.                                                         |
| `agent-browser react inspect <id> --json`                     | Props/hooks/state/source for one React node. Returns `{source: [file, line, col], text: "..."}`. Deep object values and hook values are truncated — for those, use `agent-browser eval` with a custom fiber walker. |
| `agent-browser network requests [--filter X]`                 | Recent network activity. `--clear` zeroes the buffer. `--filter <substr>` matches URL or operationName.                                                                                                             |
| `agent-browser network har start <path>` + `network har stop` | Full HAR recording. Output path is reported on `stop`; parse with `node -e 'const h = JSON.parse(fs.readFileSync(...))'`.                                                                                           |
| `agent-browser trace start [path]` + `trace stop`             | Chrome DevTools trace. Open the resulting file in chrome://tracing or DevTools Performance tab.                                                                                                                     |
| `agent-browser tab list`                                      | List page-type targets on the attached CDP browser. Use `tab <id>` to switch focus. **Always run this when BYO-attached to a multi-tab browser** — `byo-doctor` only reports one URL per port.                      |
| `agent-browser eval <expression>`                             | Run a JS expression in the page. Expression form, not arrow function — wrap in `(() => { ... })()` for statements.                                                                                                  |
| `agent-browser get url` / `get cdp-url`                       | Quick state probes.                                                                                                                                                                                                 |

### External (compose with above)

Use `node`/`jq` to slice HAR files, `grep` over React tree dumps, etc. There is no agent-qa wrapper for these; just pipe.

## Worked example — "why is the activity feed slow on reload?"

```bash
# 1. confirm tab focus
agent-browser tab list
agent-browser tab t2          # if needed

# 2. record HAR around the reload
agent-browser network har start /tmp/activity.har
agent-browser reload
sleep 8                       # let progressive loaders settle
agent-browser network har stop

# 3. slice it
node -e '
  const h = JSON.parse(require("fs").readFileSync("/tmp/activity.har","utf8"));
  const t0 = Math.min(...h.log.entries.map(e => new Date(e.startedDateTime).getTime()));
  h.log.entries
    .filter(e => /ActivityFeed|graphql/.test(e.request.url))
    .forEach(e => console.log(`+${new Date(e.startedDateTime).getTime()-t0}ms dur=${Math.round(e.time)}ms ${e.response.status} ${e.request.url.split("/").pop()}`));
'

# 4. correlate with React commits
agent-qa perf-snapshot --byo --record-renders 5000   # writes renders.json + vitals.json

# 5. zero in on the loader component
agent-browser react tree --json | grep -i 'activit\|loader\|infinit'
agent-browser react inspect <id> --json
```

No scenario was recorded. No `scenario.json` exists. The artefacts (HAR, perf JSON, react inspect output) are inspection-mode outputs; you hand them to the user as-is.

## One-off BYO snapshot

Use this when the user says "open my Brave browser and take a snapshot". Do NOT run `start`; no scenario is needed.

```bash
agent-qa byo-doctor
# After the combined R2 consent / choice prompt:
agent-qa inspect --byo --launch brave-nightly --clone-profile \
  --open 'https://app.example.com/?deployment=Staging' \
  --snapshot
```

The command launches / attaches through BYO, opens the URL, runs the inspect probe, and writes a text ARIA snapshot under `tmp/agent-qa-inspect-snapshots/` unless `--snapshot-out <path>` is passed.

## When inspection turns into a scenario

Two triggers:

1. The user explicitly says "now turn this into a regression test" / "record this so it doesn't break again."
2. You found a reproducible failure and want to bind it to a contract that fails loud the next time it regresses.

At that point: switch to `agent-qa start "<one-line assertion>" --byo`, follow the recording loop in `SKILL.md`. The inspection artefacts are not converted — they were diagnostic-only.

## Don't

- Don't run `agent-qa start` for a debug-only ask. It mints a SID, primes sidecars, and writes to `tmp/agent-qa-scenarios/` — pure overhead when you're not going to flush a scenario.
- Don't try to coerce inspection outputs (HAR, perf JSON) into the scenario schema. They're separate artefact families on purpose.
- Don't skip the R2 confirmation just because the task is "only" inspection. BYO inspection still drives the user's real browser; tab focus changes, evals, and navigations are all observable. R2 applies to every BYO turn.
