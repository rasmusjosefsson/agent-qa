# Locator tolerance metadata is advisory — runtime gates are env vars

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

`tolerate` is advisory metadata — it documents *which* drift the author
expects, nothing more. The Rust runner parses it but the auto-heal strategy
ladder does not consult it: every strategy still applies to every role+name
locator miss, and every strategy refuses to guess when more than one live
candidate matches. A step with `tolerate.digits` heals under `digits-tolerant`
the same as one without — the field records intent for reviewers, it does not
arm or disarm matching.

Two runtime gates control the whole pipeline:

- `AGENT_QA_NO_HEAL` — disables auto-heal entirely. Set it in CI runs that
  must fail hard on any accessible-name drift.
- `AGENT_QA_HEAL_STRICT` — a run that needed any heal exits non-zero even
  though every step passed. Drift surfaces for review instead of silently
  self-correcting.

There is no `heal: { "mode": "off" }` per-locator field. When a specific
accessible name is load-bearing and fuzzy matching could select the wrong
control, prefer a stable raw locator (for example a test id) with a clear
`reason` — raw locators never participate in the strategy ladder.
