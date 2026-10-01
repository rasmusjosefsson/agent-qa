# Browser extension — one-click capture → ingest → replay

A Chrome (MV3) extension that records a web flow with a single button
press and exports everything — the steps you performed *and* the
traffic the page produced — as a single file. The CLI's `ingest` verb
turns that file into a runnable scenario plus a `network.har` sidecar
for hermetic replay.

This is the "user sends us their bug" loop:

1. They click **Record**, navigate the broken flow, click **Stop &
   export** — one file downloads.
2. You run `agent-qa ingest their-capture.json`.
3. `agent-qa replay <sid>` replays their flow live;
   `agent-qa replay <sid> --mock-from recorded --offline` replays it
   pinned to exactly the backend responses their browser saw.

## Install (dev / unpacked)

```
chrome://extensions → Developer mode → Load unpacked → extension/
```

Pin it to the toolbar for the one-button UX.

## Use

- Click the toolbar button → **Record** (badge shows `REC`).
- Navigate, click, type, select, check, press Enter — steps accrue in
  the popup counter.
- Click the button again → **Stop & export** → the save dialog writes
  `agent-qa-<host>-<timestamp>.json`.

What it captures per interaction:

| Event | Draft produced |
| --- | --- |
| click on link/button/submit | `do/click` on a stable css locator (`#id` → `[data-qa]` → class path, hash-noise classes skipped) |
| double-click | `do/dblclick` — the paired `click` drafts are debounced ~350ms and collapse into one dblclick |
| navigate (link nav, typed URL, SPA pushState, back/forward) | `do/goto` with the landing URL (worker-side `webNavigation` — survives the page teardown) |
| change on input/textarea | `do/type` with the committed value |
| change on select | `do/select` with the option value |
| checkbox/radio toggle | `do/check` / `do/uncheck` |
| Enter inside an input | `do/press` `Enter` |
| Escape (dismissal) | `do/press` `Escape` |
| arrow/Home/End/PageUp/PageDown on a slider/tab/listbox/menu/radio/range widget | `do/press` with the key |
| file picked in `<input type=file>` | `do/upload` with `files/<name>` refs; contents ≤256KB are inlined into the bundle and `ingest` materializes them under `files/` — bigger/unreadable files keep the name ref and are named in the warning |
| `alert()`/`confirm()`/`prompt()` | a `{dialog:true}` check pinning the message + `do/dialog` resolving it — confirm records your real accept/dismiss, prompt records the text you typed (wrapped in the MAIN world; the returned value is the user's actual answer) |
| scroll (debounced ~400ms) | `do/scrollTo` — window scrolls record `params.y` (or `to:"bottom"`); element scrolls record an `on:css` target and replay as `scrollIntoView` (exact `scrollTop` isn't expressible) |
| every fetch/XHR | `{url, method, status, body, postData, startedAt, durationMs}` (bodies capped at 256KB) |
| interactions inside **same-origin iframes** | `do/frame` enter/exit drafts wrap the in-frame steps (nested frames handled one level at a time) |
| interactions in a `window.open`/`target=_blank` popup | the popup joins the recording; first activity there emits `do/tab` `t2`, returning to the opener emits `do/tab` `t1` (the popup's opening nav is implied by the recorded opener click) |
| closing a tracked tab (`window.close()`, a finished popup) | `do/tab` `close tN` — replay closes it too instead of leaving a stale window; next step on a surviving tab re-emits the `tab t1` switch |
| reload (F5/Ctrl+R, form resubmit) | `do/reload` — `webNavigation` transitionType names it, so a real reload isn't deduped away by the same-URL filter |

**Not captured (v1):** `beforeunload` dialogs (no JS entry
point to wrap), cross-origin iframe internals (counted as a bundle
warning — the iframe element can't be located from inside it),
canvas drawing, real multi-touch, hover-only gestures, exact
element `scrollTop` (element scrolls replay as `scrollIntoView`).
Record those flows with `agent-qa record` instead.

## Ingest

```
agent-qa ingest <bundle.json> [--sid <name>]
```

writes:

```
<scenarios_root>/<sid>/scenario.json
<scenarios_root>/<sid>/replays/recorded/network.har
```

Every step goes through the same validation `record-step` applies —
malformed captures fail at ingest with the same errors, not at replay.
The scenario's `env.open` is `[fresh, nav <recorded url>]`, matching
what `agent-qa record` seals.

## Replay

```
agent-qa replay <sid>                                  # against the live app
agent-qa replay <sid> --mock-from recorded --offline   # pinned backend
```

The hermetic variant is the debugging combo: the app replays against
the captured responses while `--offline` rejects anything the user's
browser didn't see — divergences point at the bug.

## Bundle shape

```json
{
  "version": 1,
  "url": "https://app.example.com/login",
  "startedAt": "2026-09-30T08:00:00.000Z",
  "intent": "recorded via agent-qa extension on app.example.com",
  "steps": [
    {"kind": "do", "draft": {"intent": "click \"Go\"", "verb": "click", "on": "css:#go"}}
  ],
  "network": [
    {"url": "https://api.x/login", "method": "POST", "status": 200,
     "body": "{...}", "postData": "{...}", "startedAt": "...", "durationMs": 88}
  ]
}
```

Step drafts use the `record-step` draft shape; `on` accepts locator
shorthand (`css:`, `xpath:`, `testId:`, `text:`) and values are typed
(`{"from": "literal", "literal": "..."}`). Targets inside **open
shadow roots** emit a role locator instead (`{"role": {"role": ...,
"name": ...}}`) — a `css:` path can't pierce the boundary on replay,
where the a11y tree can. Closed roots record nothing resolvable (same
hard limit as everywhere).

## How the network capture works

The content script injects a small page-world patch over `fetch` and
`XMLHttpRequest` — response bodies included, no debugger banner, no
devtools window needed. It's inert until recording starts and resumes
automatically after full-page navigations.
