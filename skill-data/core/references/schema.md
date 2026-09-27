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

Use `agent-qa scenario check <scenario.json>` before replay. It validates the
schema and runs the scenario linter.
