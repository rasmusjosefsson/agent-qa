# Checks

Use `record-step check` to record a scenario/2 claim after the browser reaches
the intended state. A check draft omits `id` and `kind`.

```bash
agent-qa record-step check '{
  "intent": "the editor is visible",
  "claim": {
    "subject": {"element": {"role":"dialog","name":"Edit user"}},
    "predicate": "isVisible"
  }
}'
```

Use a raw text locator only when the page has no stable accessible role.

```json
{
  "intent": "the confirmation is visible",
  "claim": {
    "subject": {
      "element": {
        "raw": {"kind":"text","value":"Saved"},
        "reason": "the page has no stable role for the confirmation"
      }
    },
    "predicate": "isVisible"
  }
}
```

`record-step` accepts direct `check` drafts only.
