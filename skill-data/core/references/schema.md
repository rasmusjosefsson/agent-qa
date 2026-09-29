# `scenario/2` reference

`schema/scenario-schema.json` defines the scenario contract. `cli/src/scenario.rs`
defines the matching Rust types. The schema validates a scenario before replay.

A scenario has an `id`, an `intent`, replay steps, optional `env` setup and cleanup,
and optional provenance.

```json
{
  "schema": "scenario/2",
  "id": "open-users",
  "intent": "open the users page",
  "env": {
    "open": [{ "kind": "nav", "url": "https://example.com/users" }]
  },
  "steps": [
    {
      "id": "s0",
      "intent": "open users",
      "kind": "do",
      "verb": "goto",
      "value": { "from": "literal", "literal": "https://example.com/users" }
    },
    {
      "id": "s1",
      "intent": "users are visible",
      "kind": "check",
      "claim": {
        "subject": { "element": { "role": "heading", "name": "Users" } },
        "predicate": "isVisible"
      }
    }
  ],
  "producedBy": {
    "producer": "agent-recorder"
  }
}
```

Use `record-step do` and `record-step check` to create steps. The recorder assigns
`id` and `kind`. Do not hand-edit them into drafts.

`env.open` and `env.close` accept the existing generic `EnvOp` kinds. They are
`fresh`, `useProfile`, `nav`, `cookie`, `localStorage`, `gql`, and `flag`.
`record-setup` records one schema-valid `env.open` value.

### Mid-scenario state seeding

`env.open` seeds at bootstrap; a `state` do-step does it mid-scenario —
flip a flag between pages, expire a session mid-flow, pre-seed storage
before a `goto`/`reload`:

```json
{
  "id": "s2", "intent": "seed an authenticated cart", "kind": "do", "verb": "state",
  "params": {
    "localStorage": { "token": "eyJ…" },
    "cookies": [{ "name": "session", "value": "abc", "path": "/" }],
    "clearCookies": true
  }
}
```

Recognized `params` keys: `localStorage`, `sessionStorage`, `cookies`
(each entry `{"name","value","path"?,"domain"?,"maxAge"?,"secure"?,"sameSite"?}`),
`clearCookies`, `clearLocalStorage`, `clearSessionStorage`, and `indexeddb`
(an array of `{"db","store","keyPath"?,"clear"?,"put":[…]}` — with `keyPath`
the `put` entries are full records, without it `{"key","value"}` pairs stored
under out-of-line keys; missing stores are created via a db version bump).
Cookies go through `document.cookie`, so `httpOnly` values cannot be seeded —
auth plugins cover that. Assert the result with `{"storage": "key"}` /
`{"storage": {"key": "k", "scope": "session"}, "path": "$.json.path"}`,
`{"cookie": "name"}`, or `{"indexeddb": {"db","store","key"?}, "path"?}`
claims (without `key` the subject is the object store itself).

### Iframes

A `frame` do-step switches the session's frame context — every locator on
later steps resolves inside the selected iframe until you switch back:

```json
{
  "id": "s2", "intent": "enter the editor iframe", "kind": "do", "verb": "frame",
  "params": { "selector": "#editor-frame" }
},
{
  "id": "s3", "intent": "type inside it", "kind": "do", "verb": "type",
  "on": { "raw": { "kind": "css", "value": "body" }, "reason": "frame body" },
  "value": { "from": "literal", "literal": "hello" }
},
{
  "id": "s4", "intent": "back to the top document", "kind": "do", "verb": "frame",
  "params": { "main": true }
}
```

`params.selector` is a CSS selector matched against the *current* document
(contexts do not nest — `frame` + `frame` enters a sibling, not a child);
`params.main: true` always returns to the top document. Strings substitute
`{{var}}`. Cross-origin iframes work — frame context is a CDP-level switch,
not a DOM read. Note: claims and screenshots keep evaluating against the
selected frame, so switch back to `main` before asserting on outer chrome.

### Viewport resizing

Resize the browser viewport mid-scenario with a `viewport` do-step:

```json
{
  "id": "s1",
  "intent": "switch to mobile size",
  "kind": "do",
  "verb": "viewport",
  "params": { "width": 375, "height": 812 }
}
```

Both `width` and `height` are required positive integers (CSS pixels); string
values like `"{{mobileWidth}}"` are resolved through scenario vars. Combine with
an element claim afterwards to assert responsive behaviour (e.g. a hamburger
menu that only renders below a breakpoint).

### Emulation

An `emulate` do-step changes the browser's emulation layer mid-scenario —
device preset, geolocation, media prefs, network state, request headers,
HTTP auth. Keys map onto `agent-browser set …`:

```json
{
  "id": "s1",
  "intent": "run as a tagged mobile client",
  "kind": "do",
  "verb": "emulate",
  "params": {
    "device": "iPhone 12",
    "geo": { "lat": 37.7749, "lng": -122.4194 },
    "colorScheme": "dark",
    "reducedMotion": true,
    "headers": { "X-QA-Suite": "golden" },
    "credentials": { "user": "admin", "pass": "{{adminPass}}" },
    "offline": false
  }
}
```

| key             | effect                                                        |
| --------------- | ------------------------------------------------------------- |
| `device`        | `set device <name>` — UA + viewport + scale preset            |
| `geo`           | `set geo <lat> <lng>` — geolocation override                  |
| `offline`       | `set offline on|off` — toggle offline mode                    |
| `colorScheme`   | `set media dark|light`                                        |
| `reducedMotion` | adds `reduced-motion` to the media call when true             |
| `headers`       | `set headers {json}` — extra headers on subsequent requests   |
| `credentials`   | `set credentials <user> <pass>` — HTTP auth for this + new tabs|

At least one key is required; unknown keys fail at dispatch. Strings go
through `{{var}}` substitution. Apply BEFORE the `goto`/`reload` you want
to observe — emulation set mid-page doesn't retroactively change requests
already made. Timezone/locale emulation isn't reachable through
agent-browser's `set` surface yet.

### Waits

`do/wait` picks its semantics from `params`:

- `{"ms": 250}` — fixed delay.
- `{"until": "load" | "networkidle" | "domcontentloaded"}` — wait for a load
  state.
- `{"url": "*/api/users*", "timeoutMs": 8000}` — poll the Resource Timing API
  until a request whose URL matches the glob completes (`*` = wildcard; plain
  substring also works). Use this to gate a check on the XHR that feeds the
  UI instead of guessing a fixed delay — e.g. click a filter, wait for the
  results call, then assert the table. `timeoutMs` defaults to 10s and the
  step fails when nothing matching lands in time.
- `{"idle": true}` or `{"idleMs": 500, "timeoutMs": 10000}` — wait for
  network quiescence at the session level: zero in-flight requests for
  `idleMs` (default 500) straight. This is the stability gate before a
  screenshot or DOM read after async work — unlike `url` it waits for
  silence rather than one named call. Fails at `timeoutMs` listing what's
  still pending (long-polls/websockets will hit this — give them `url`
  waits instead).
- No `params` — soft `networkidle` wait (use `url`/`idle` when the app keeps
  long-lived connections open and never goes idle).

### Per-step retry

Any do-step may carry `params.retry` (attempt count) and
`params.retryMs` (delay between attempts, default 300): the runner
re-dispatches the step on failure before the run's failure handling kicks
in. Use for one known-flaky interaction; whole-run flake belongs to
`replay --retry`. Caveat: a partially-dispatched retry re-fires the side
effect — prefer idempotent verbs (click, select) over append-style ones
(type).

### Native dialogs (alert/confirm/prompt)

Resolve a pending native dialog with a `dialog` do-step:

```json
{
  "id": "s2",
  "intent": "accept the alert",
  "kind": "do",
  "verb": "dialog",
  "params": { "action": "accept" }
}
```

`params.action` is `"accept"` or `"dismiss"`; `params.text` optionally carries
prompt input. Assert on a pending dialog with the `{"dialog": true}` check
subject — `exists`/`notExists` for presence; string predicates like `contains`
match the dialog's message.

Scenarios containing dialog steps replay with `AGENT_BROWSER_NO_AUTO_DIALOG=1`
set automatically (otherwise agent-browser auto-accepts `alert` before the
`dialog` step can observe it). Record with the same env var so the dialog
stays pending between the click that opens it and the resolving step —
prefer it over `agent-browser --no-auto-dialog`, which can swallow the first
navigation when the flag triggers the browser launch. `eval`, `snapshot`, and
screenshots cannot run while a dialog is pending, so per-step sidecars are
skipped for those steps.

### Native file chooser

Clicking an `<input type="file">` — directly, via `<label for>`, via
`input.click()`/`showPicker()`, or via `window.showOpenFilePicker()` — opens
the OS picker, which replay cannot drive and headless mode leaves hanging.
Arm interception with a `fileChooser` do-step BEFORE the click that would
open the picker:

```json
{
  "id": "s1",
  "intent": "next chooser gets these files",
  "kind": "do",
  "verb": "fileChooser",
  "params": { "files": ["uploads/report.pdf", "uploads/logo.png"] }
}
```

The next chooser that opens (whichever path triggered it) resolves with the
given files — real `File` objects land on `input.files` (or `getFile()`
handles for `showOpenFilePicker`) with `input`+`change` dispatched, so the
page's own handlers run normally. Paths resolve like `upload`'s (cwd, then
the scenario dir, then `$AGENT_QA_REPO_ROOT`). `params.files` may also be a
single `{{var}}`-resolved string array; an empty array simulates cancelling
the picker. The hook lives in the page — re-arm after navigations. Once
installed it also captures an UNARMED chooser-open (input click or
`showOpenFilePicker`) as "pending", so a click followed by a `fileChooser`
step resolves it — but only if an earlier `fileChooser` step already ran in
the same document.

For plain visible file inputs the direct `upload` verb is simpler; use
`fileChooser` when the input is hidden/transient (created on click), styled
behind a button or label, or driven through `showOpenFilePicker`.

### Drag and drop

`do/drag` drags `on` (the source locator) onto `params.to` (the drop-target
locator — same locator shapes as `on`):

```json
{
  "id": "s4",
  "intent": "move Card A into Done",
  "kind": "do",
  "verb": "drag",
  "on": { "role": "listitem", "name": "Card A" },
  "params": { "to": { "role": "list", "name": "Done" } }
}
```

One eval resolves both endpoints (scope chains included) and dispatches the
full gesture on the nodes: `pointerdown`/`mousedown` on the source, a shared
`DataTransfer` `dragstart → dragenter/dragover → drop → dragend` chain for
HTML5 `draggable` dnd, plus `pointermove`/`mousemove`/`pointerup`/`mouseup`
for pointer- and mouse-driven sortable libraries. Because the events land on
the nodes directly (no coordinates), overlay interception can't swallow them.
In the workbench, drag an element to its target on the live canvas while
recording — the gesture is recorded as a `drag` step automatically (both
endpoints must have an accessible role + name).

### Touch gestures

`do/hold` presses `on` (required locator) for `params.ms` (default 500) without
releasing: `pointerdown`/`mousedown`/`touchstart` on the element, a wait, then
`pointerup`/`mouseup`/`touchend`. No `click` event is dispatched, so hold-only
handlers (long-press menus) trigger while tap handlers do not.

`do/swipe` dispatches a `touchstart → touchmove×8 → touchend` gesture
(pointer/mouse fallback included): `params.direction` (required, one of
`up|down|left|right` — the direction the finger travels, so `up` scrolls a page
down) and `params.distance` (default 300 px). Optional `on` starts the gesture
at the element's center; omit it for a viewport-centered swipe.

```json
{ "id": "s4", "intent": "long-press the row", "kind": "do", "verb": "hold",
  "on": { "raw": { "kind": "css", "value": ".row" }, "reason": "row" },
  "params": { "ms": 800 } },
{ "id": "s5", "intent": "swipe the card away", "kind": "do", "verb": "swipe",
  "on": { "raw": { "kind": "css", "value": ".card" }, "reason": "card" },
  "params": { "direction": "left", "distance": 200 } },
{ "id": "s6", "intent": "scroll the feed", "kind": "do", "verb": "swipe",
  "params": { "direction": "up" } }
```

Both gestures are synthesized in-page (eval), so they work on desktop
headless too — no mobile emulation needed. Pages that only listen to
`click`/`scroll` won't see them; a real `scrollTo`/`click` verb is still the
right tool there.

### Secondary click (rightclick)

`do/rightclick` fires a secondary-button pointer+mouse chain on `on`
(`pointerdown`/`mousedown` `button: 2` → `pointerup`/`mouseup` →
`contextmenu`) — the gesture that opens context menus. Never fires a primary
`click`, so handlers keyed on `click` stay quiet. It accepts every locator
shape (role/css/testid/xpath/text, scope chains included). If the page opens a
native dialog from the contextmenu handler, resolve it with the usual
`dialog` step.

```json
{ "id": "s3", "intent": "open the context menu", "kind": "do", "verb": "rightclick",
  "on": { "role": "button", "name": "Actions" } }
```

### Downloads and file claims

`do/download` clicks a trigger (`on` locator) and saves the browser download
to `value` — a path relative to the scenario dir:

```json
{
  "id": "s3",
  "intent": "download the report",
  "kind": "do",
  "verb": "download",
  "on": { "raw": { "kind": "css", "value": "[data-testid=dl-json]" }, "reason": "download anchor" },
  "value": "downloads/qa-data.json"
}
```

Assert on the saved file with the `{"file": "<path>"}` check subject:
`exists`/`notExists` for presence, `gt`/`gte`/`lt`/`lte` compare size in
bytes, and string predicates match the file name — or the file's UTF-8
text when `"attribute": "content"` is set (files over 1 MiB are rejected
for content claims). Relative paths resolve against the scenario dir.

### Network mocks

`do/mock` stubs matching `fetch` and `XMLHttpRequest` calls in the live
page — edge cases (5xx, error payloads, latency) that the real backend
won't produce on demand. `params.url` is a glob matched on the request
URL (`*` is the only wildcard):

```json
{
  "id": "s4",
  "intent": "the API is down",
  "kind": "do",
  "verb": "mock",
  "params": { "url": "*/api/users*", "status": 503, "json": { "error": "unavailable" }, "delayMs": 50 }
}
```

`params.abort: true` rejects instead of responding — the page sees a real
network failure (`fetch` rejects `TypeError: Failed to fetch`, XHR fires
`error`), modeling a backend that's unreachable rather than erroring.

Registered rules re-apply automatically after `goto`/`reload`/`back`/
`forward` (navigation wipes the page's JS world, so the runner reinstalls
the wrapper). A click that navigates still drops them — put `mock` steps
after unpredictable navigations. `do/unmock` removes a rule
(`params.url` = the same glob) or clears all without it. Combine with
`{"network": {"urlMatches": ...}}` claims to prove the stub fired, or a
`{"shot": ...}` claim to golden the error UI.

For hermetic replay, `agent-qa replay <sid> --mock-from <runId>` seeds
stubs from a previous run's `network.har` (recorded via `--har`): every
URL the page fetched gets its recorded status+body back — no backend
needed. Seeded rules install as a page init script on a fresh session,
covering even the page's load-time fetches.

Add `--offline` for the strict form: any fetch/XHR that matches NO rule
rejects with a network error instead of reaching the real backend —
with `--mock-from`, only the recorded traffic replays; alone, every
request is stubbed (static pages replay with zero network).

### Console claims

Assert on messages the page logged this session with the `{"console"}`
subject — the canonical use is gating a golden path on "no JS errors":

```json
{
  "id": "s9",
  "intent": "the page logged no console errors",
  "kind": "check",
  "claim": {
    "subject": { "console": { "type": "error" } },
    "predicate": "notExists"
  }
}
```

The matcher filters which messages count: `{"console": true}` → all,
`{"type": "error"|"warn"|"log"|...}` → that level, `{"text": "<substring>"}`
→ text contains it. `exists`/`notExists` check presence of a matching
message, `countEquals`/`gt`/`gte`/`lt`/`lte` compare the count to `value`,
and text predicates (`equals`/`contains`/`matches`/`startsWith`/`endsWith`)
pass when ANY matching message's text satisfies them.

Messages accumulate for the whole session — on a reused session, entries
from earlier steps count too.

### Accessibility claims

`{"a11y": true}` runs an axe-core audit (`agent-browser a11y`) and counts
violations; a matcher narrows which findings count:

```json
{
  "id": "s10",
  "intent": "no serious accessibility violations",
  "kind": "check",
  "claim": {
    "subject": { "a11y": { "impact": "serious" } },
    "predicate": "notExists"
  }
}
```

Matcher fields: `impact` (a floor — `serious` also counts `critical`),
`rule` (one axe rule id), `within` (CSS selector, scopes the audit to a
subtree), `incomplete` (also count axe's `incomplete` results). Predicates:
`exists`/`notExists` on presence, `countEquals`/`gt`/`gte`/`lt`/`lte` on the
count. A failing check lists the offending rule ids.

### Network claims

Assert on the browser's captured request log with the `{"network"}`
subject — matcher fields AND together:

- `urlMatches` — regex on the request URL
- `operationName` — substring on the URL (GraphQL-style operation names)
- `method` — `"GET"`/`"POST"`/`"PUT"`/`"PATCH"`/`"DELETE"`/`"HEAD"`/`"WS"`
  (`"WS"` selects captured sockets)
- `postDataContains` — substring on the request's POST body (fetches the
  request detail per candidate — keep a url/method matcher alongside so the
  narrowing runs on a small set)
- `wsPayloadContains` — substring on any WebSocket frame payload; narrows
  to `cdpws-*` socket entries. Sockets appear as `method: "WS"`,
  `status: 101`, `resourceType: "WebSocket"` with `wsFrames[]`
  (`{dir, opcode, payload}`); `EventSource` streams appear as GETs with
  `resourceType: "EventSource"`.

`ofKind` picks what the predicate applies to:

- `fired` (default) — `exists`/`isVisible` pass once a matching request
  occurred; `notExists`/`isHidden` pass while none has.
- `status` — predicates on the latest matching response's HTTP status
  (string predicates on `"200"`, `gt`/`gte`/`lt`/`lte` numerically).
- `responseJsonPath` — `path` + predicates evaluate inside the latest
  match's JSON response body (e.g. `$.data.user.name`).

```json
{
  "id": "s7",
  "intent": "the save call carried the new title",
  "kind": "check",
  "claim": {
    "subject": {
      "network": { "urlMatches": "/api/save", "method": "POST", "postDataContains": "Quarterly" }
    },
    "predicate": "exists"
  }
}
```

Every completed run also writes `replays/<run>/network.json` — the whole
request list — and `agent-qa compare <sid> <runA> <runB>` diffs it (the
`## network` section in compare.md).

Use `agent-qa scenario check <scenario.json>` before replay. It validates the
schema and runs the scenario linter.
