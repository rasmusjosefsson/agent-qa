## perf-snapshot — opt-in performance sidecar

Capture passive performance signals from the live tab. Strictly orthogonal to recording: `perf-snapshot` does NOT touch scenario content, does NOT advance steps, does NOT call `record-step`. It writes its output to a dedicated `<sid>/perf/` subfolder so multiple snapshots can attach to one recording without colliding.

### When to use it

- "**Debug perf on /users and /settings**" — call `agent-qa perf-snapshot` on each problem page; compare `vitals.json` and `suspense.json` reports across them.
- "**Profile this user-creation flow**" — between gestures, call `agent-qa perf-snapshot --record-renders 5000 --cpu-profile 5000` to capture both React commit details and Chrome's CPU sampling profile over the same window.
- "**Get a full timeline trace**" — `agent-qa perf-snapshot --trace 5000` writes a `.trace.json` openable in Chrome DevTools' Performance tab (drag-and-drop).

### When NOT to use it

- During an unrelated recording. Recording verbs (`start`, `record-step`, `replay`) already capture the artefacts they need; perf is bonus context the user has to explicitly ask for.
- For app-specific structural output. `perf-snapshot` reads `window.performance` only — it emits navigation + paint timings, not app-internal component/data extraction; that's a deliberate scope limit.

### Usage

```bash
agent-qa perf-snapshot [--sid <sid>] [--profile <p>] \
                       [--record-renders <ms>] \
                       [--cpu-profile <ms>] \
                       [--trace <ms>]
```

Defaults:

- `--sid` defaults to the active recording SID when available; pass it explicitly to attach to a specific recording.
- `--profile default-user` — same convention as the recording verbs.
- All time-windowed flags are opt-in. With NO flags, only `vitals.json` + `suspense.json` are written (point-in-time signals, ~5s wall).

### Output

```
tmp/agent-qa-scenarios/<sid>/perf/
  vitals.json        # always — Core Web Vitals + React hydration phases
  suspense.json      # always — Suspense boundary classifier
  renders.json       # only with --record-renders <ms>
  cpu.cpuprofile     # only with --cpu-profile <ms>
  trace.json         # only with --trace <ms>
```

| File             | What it carries                                                                                                                          | Open with                                                                                     |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `vitals.json`    | LCP, CLS, TTFB, FCP, INP. React hydration phases when a profiling build is detected.                                                     | Read directly; small (240 bytes).                                                             |
| `suspense.json`  | Per-boundary report: dynamic vs static classification, primary blocker, source location, suggested next step.                            | Read directly; small (~1 KB).                                                                 |
| `renders.json`   | React `onCommitFiberRoot` profile over the window: per-fiber mount counts, re-render counts, change details (which props/state changed). | Read directly; small to medium (10–50 KB depending on activity).                              |
| `cpu.cpuprofile` | Chrome V8 sampling CPU profile. Binary-ish.                                                                                              | Drag-and-drop into Chrome DevTools → Performance tab. ~3–5 MB for a few seconds on the target app.  |
| `trace.json`     | Chrome DevTools timeline trace (full instrumentation: paint, layout, scripting, rasterizer, GPU).                                        | Drag-and-drop into Chrome DevTools → Performance tab. ~5–10 MB for a few seconds on the target app. |

### Concurrent profilers and their limits

- `--record-renders` is **independent**: uses React DevTools' commit hook, no Chrome contention. Can run with anything.
- `--cpu-profile` and `--trace` **share Chrome's tracing infrastructure** and CANNOT run simultaneously. The verb refuses both flags together with exit code 2 and a clear message — split into two `perf-snapshot` invocations.
- All requested time-windowed profilers run **concurrently over the longest requested window**. `--record-renders 2000 --cpu-profile 5000` sleeps 5000ms; both stop signals fire at the end. No way to ask for staggered windows from one verb call.

### Defensive cleanup

The verb soft-stops any orphaned `profiler` / `trace` session before starting (errors swallowed). A previous `perf-snapshot` that exited mid-flow won't block the next one. Same protection isn't needed for `react renders` — that hook can be safely re-started without explicit stop.

### Why `perf-snapshot` is observability-only

`perf-snapshot` deliberately sticks to signals agent-browser exposes directly:
Core Web Vitals, render commits, Suspense boundaries, CPU sampling, and Chrome
traces. agent-qa's role is to drive the page and observe it from the outside —
it does not reach into the app to reconstruct component/data internals. Those
are app-specific concerns that belong to the app, not to a generic driver.

`perf-snapshot` survives because these observability layers have no in-page
equivalent — they're orthogonal to anything the page itself renders.

### Examples

Single-page perf check (point-in-time only):

```bash
agent-qa start "perf check"        # mints a SID
agent-browser --session default-user-session open 'https://app.example.com/dashboard?deployment=Staging'
agent-qa perf-snapshot
# → tmp/agent-qa-scenarios/<sid>/perf/{vitals.json,suspense.json}
```

Profile a flow's CPU + renders:

```bash
SID=$(agent-qa start "create user perf")
agent-browser --session default-user-session open 'https://app.example.com/users?deployment=Staging'
agent-qa smart-click "Add user"
# 5-second window covering the form interaction
agent-qa perf-snapshot --record-renders 5000 --cpu-profile 5000
agent-qa fill-unique "Email" --template 'qa-{{vars._unique}}@example.com'
agent-qa smart-click "Save"
# point-in-time check after submit
agent-qa perf-snapshot
```

Cross-page comparison:

```bash
for path in /home /users /settings; do
  agent-browser --session default-user-session open "https://app.example.com${path}?deployment=Staging"
  agent-qa perf-snapshot
  cp tmp/agent-qa-scenarios/<sid>/perf/vitals.json /tmp/vitals${path//\//-}.json
done
diff /tmp/vitals-home.json /tmp/vitals-users.json
```

### Exit codes

- 0 — succeeded (any individual non-fatal error like `vitals failed` is logged but doesn't change exit)
- 1 — fatal: session not on a captureable page (login or `about:blank`)
- 2 — bad CLI args (e.g. `--cpu-profile` and `--trace` both given, or non-positive duration)
