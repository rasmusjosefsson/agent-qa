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

## Claim subjects

Every claim is `{"subject": <one of>, "predicate": <pred>}`.

| Subject shape | Asserts on |
| --- | --- |
| `{"element": {"role": ..., "name"?}}` or `{"element": {"raw": ...}}` | An accessible element (visibility, text, count, attributes). |
| `{"url": true}` | The current page URL (`equals`/`contains`/`matches`/`startsWith`/`endsWith`). |
| `{"file": "downloads/x.pdf"}` | A file saved into the scenario dir by a `download` step (`exists`, `equals` on name, `gt` on byte size, `contains` on content). |
| `{"dialog": true}` | A pending native dialog (`exists`/`notExists`; string predicates match its message). |
| `{"shot": "<stepId>"}` | `screenshots/<stepId>.png` pixel-diffed vs `baselines/<stepId>.png` (`matches` + optional `tolerance.pixels`) — see `visual.md`. |
| `{"domshot": "<stepId>"}` | `snapshots/<stepId>.txt` (the per-step ARIA tree) text-diffed vs `baselines/<stepId>.snap.txt` (`matches`; optional `skip` regex list drops volatile lines) — see `visual.md`. |

Predicates (camelCase): `isVisible`, `isHidden`, `isEnabled`, `isDisabled`,
`isChecked`, `isUnchecked`, `exists`, `notExists`, `equals`, `contains`,
`matches`, `startsWith`, `endsWith`, `gt`, `gte`, `lt`, `lte`, `countEquals`.

`"attribute": "<name>"` on an `element` subject reads one attribute per
poll: on raw css/testId locators `text`/`value`/`checked`/`disabled`/
`selected`/`readOnly`/`required`/`focused`/`style:<prop>` read the live
IDL property (or getAttribute otherwise); on `role` locators the a11y
snapshot carries `checked`/`disabled`/`required`/`expanded`/`pressed`/
`selected`/`readonly`/`current`/`level`/`orientation`/`valuemin`/
`valuemax`/`valuenow`, plus `value` (the node's `: tail`) and `text`
(the accessible name). Attributes the a11y tree doesn't carry (`href`,
`data-*`, ...) need a raw locator.
