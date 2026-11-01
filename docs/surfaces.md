# Surface providers

A scenario's `surface` chooses who performs the steps.

| `surface` | Who replays it |
| --- | --- |
| omitted or `"web"` | The built-in browser runner. It drives `agent-browser` and never starts a surface provider. |
| `"desktop"`, `"ios"`, `"android"` | A provider process you configure. The browser is not launched, and browser flags (`--base-url`, `--har`, `--offline`, `--fresh-browser`, `--headed`, personas) are ignored. |

`desktop`, `ios`, and `android` share one protocol. Adding a platform is a new provider binary, not a new browser code path. This repository ships the host and a desktop reference provider. It does not ship iOS or Android drivers.

```json
{
  "schema": "scenario/2",
  "id": "save-note",
  "intent": "save the note",
  "surface": "desktop",
  "target": { "app": "TextEdit", "window": "Untitled" },
  "steps": [
    {
      "id": "s0",
      "intent": "press Save",
      "kind": "do",
      "verb": "click",
      "on": { "role": "button", "name": "Save" }
    },
    {
      "id": "s1",
      "intent": "Save is showing",
      "kind": "check",
      "claim": {
        "subject": { "element": { "role": "button", "name": "Save" } },
        "predicate": "isVisible"
      }
    }
  ]
}
```

`target.app` or `target.device` is required. `target.window` is a desktop window title. `env.open` is web setup and is not run.

The shared actions are `click`, `dblclick`, `rightclick`, `type`, `clear`, `press`, `hover`, `select`, `check`, `uncheck`, `scrollTo`, `focus`, `wait`, and `read`, addressed by role and accessible name. The shared checks are element state, `shot`, and `domshot`. A web-only verb (`goto`, `mock`, network, cookies, frames) fails that step and does not fall through to the browser.

Replay writes the same `replays/<runId>/` tree the browser runner writes (`audit.json`, `events.jsonl`, `snapshots/`, `screenshots/`), so compare and the audit verbs read a desktop run the same way.

`--dry-run` validates the scenario and writes the audit line `SUMMARY: 0/N (DRY-RUN)` without starting the provider.

## Configure

```toml
[surfaces]
desktop = "./examples/surfaces/desktop/provider.js"
```

A relative path is relative to the toml file. A bare name is resolved on `$PATH`. The environment variable `AGENT_QA_SURFACE_DESKTOP` (and `AGENT_QA_SURFACE_IOS`, `AGENT_QA_SURFACE_ANDROID`) overrides the table. Providers are not plugin binaries and are not discovered by `plugins doctor`.

## Protocol

The host runs `<provider> <op>` with one JSON object on stdin and reads one JSON object from stdout. The envelope matches plugins: `protocolVersion` `1`, and exactly one of `response` or `error`.

| op | Request | Response |
| --- | --- | --- |
| `ping` | `{}` | `{ "protocolVersion": 1, "name": "…", "surfaces": ["desktop"] }` |
| `open` | `{ "surface", "target" }` | `{ "ready": true }` |
| `act` | `{ "surface", "target", "verb", "on", "value", "params" }` | an observation |
| `observe` | `{ "surface", "target" }` | an observation |
| `close` | `{ "surface", "target" }` | `{}` |

`on` is `{ "role", "name?" }`. `value` is a string or null. An observation is:

```json
{
  "snapshot": "- button \"Save\"\n",
  "screenshotPng": "<base64, optional>",
  "elements": [
    {
      "role": "button",
      "name": "Save",
      "text": "Save",
      "value": null,
      "focused": false,
      "enabled": true,
      "checked": false
    }
  ]
}
```

`snapshot` is the structural text `domshot` compares, after `@…:eN` refs collapse to `@e`. `elements` is what element checks match. `screenshotPng` is standard base64 PNG bytes for `shot`.

Exit `0` and set `ok` to `false` with `{ "code", "message" }` for a refused action. A provider that speaks a higher `protocolVersion` is refused.

## Desktop reference

[`examples/surfaces/desktop/provider.js`](../examples/surfaces/desktop/provider.js) is the desktop provider. It shells the `agent-desktop` CLI (`AGENT_DESKTOP_BIN` overrides the binary). `type` is sent as `set-value`. `dblclick` and `hover` pass `--headed` because those gestures have no headless accessibility path. `AGENT_QA_SURFACE_HEADED=1` makes every desktop command headed.

`open` runs `launch` for `target.app`, then `wait --window` when `target.window` is set. Each call carries `target`, so the provider process itself stays stateless. `close` runs `close-app`.

iOS and Android use the same ops with their own binaries. Point `[surfaces] ios` and `[surfaces] android` at those providers when they exist.
