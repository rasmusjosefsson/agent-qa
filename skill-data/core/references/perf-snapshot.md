## perf-snapshot — opt-in performance sidecar

Read `window.performance` from the live tab. It does not touch scenario content, advance steps, or call `record-step`. With `--sid` it writes one JSON file under `<sid>/perf/`. Without `--sid` it prints the JSON.

The verb is navigation timings and paint entries only. It does not record React commits, sample the CPU, or write a DevTools trace. Those questions go to the pinned browser, through `agent-qa browser`.

### When to use it

- "How long did this document take to load?" — `agent-qa perf-snapshot --session <name>` after the navigation has settled.
- Attach the same reading to a recording with `--sid <sid>`.

### When not to use it

- For a React component, render commit, or CPU sample. Use `agent-qa browser --session <s> react tree --json` and `react inspect <id> --json`.
- For a Chrome timeline. Use `agent-qa browser --session <s> trace start <path>` and `trace stop`.
- During an unrelated recording, unless the user asked for the timings.

### Usage

```bash
agent-qa perf-snapshot [--sid <sid>] [--session <name>] [--quiet]
```

`--session` defaults to `default`. `--quiet` with `--sid` prints only the output path.

### Output

```json
{
  "navigation": {
    "startTime": 0,
    "domContentLoadedEventEnd": 0,
    "loadEventEnd": 0,
    "responseStart": 0,
    "responseEnd": 0,
    "requestStart": 0,
    "domComplete": 0,
    "transferSize": 0,
    "type": "navigate",
    "redirectCount": 0
  },
  "paint": [{ "name": "first-contentful-paint", "startTime": 0 }],
  "collectedAt": "2026-01-01T00:00:00.000Z"
}
```

`navigation` is `null` when the document has no navigation entry (for example `about:blank`). With `--sid` the file is `<scenarios>/<sid>/perf/<timestamp>.json`.

### Example

```bash
agent-qa browser --session inspect open https://example.com/users
agent-qa perf-snapshot --session inspect
```

To keep the reading next to a recording:

```bash
agent-qa start "check users load" --session inspect --open https://example.com/users
agent-qa perf-snapshot --session inspect --sid <sid>
```

### Deeper looks, still on the pinned browser

```bash
agent-qa browser --session inspect react tree --json
agent-qa browser --session inspect react inspect <id> --json
agent-qa browser --session inspect trace start /tmp/users.trace.json
agent-qa browser --session inspect trace stop
```

Open the trace in the browser's Performance panel. `react inspect` is the component question. `perf-snapshot` stays the document-timing question.
