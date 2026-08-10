# Locator tolerance metadata is reserved

The scenario/2 schema accepts a `tolerate` object on role locators:

```jsonc
{
  "role": "button",
  "name": "Submit 42 items",
  "tolerate": {
    "digits": true,
    "reason": "the count is expected to vary"
  }
}
```

This metadata is reserved for the planned audited heal pipeline. The current
Rust runner parses it but does not use it when resolving or dispatching a
locator. It therefore cannot enable or disable digit-normalized matching,
generated-suffix matching, or any other fallback.

Do not rely on `tolerate` as a correctness boundary. When an accessible name is
load-bearing and fuzzy matching could select the wrong control, prefer a stable
raw locator (for example a test id) with a clear `reason`, or change the page to
expose a stable accessible name.

There is currently no `heal: { "mode": "off" }` locator field and no
`AGENT_QA_NO_HEAL` runtime switch.
