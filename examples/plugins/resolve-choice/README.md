# resolve-choice — reference `resolve` plugin

Answers the `resolve` kind's question — "which of these page elements did
the author mean?" — by POSTing a single choice question to a decision
endpoint configured entirely through environment variables.

## Wire it up

```toml
# agent-qa.toml
[plugins]
resolve = "/path/to/examples/plugins/resolve-choice/agent-qa-plugin-resolve-choice"
```

```sh
export RESOLVE_ENDPOINT="https://<decision-api>/v1/systemone"
export RESOLVE_API_KEY="…"          # or: RESOLVE_KEY_ENV=MY_KEY_VAR
export RESOLVE_MODEL="default"      # optional, default "default"
export RESOLVE_MIN_CONFIDENCE=0.5   # optional, default 0.5
```

Then, during a recording session:

```sh
agent-qa resolve "the save button"            # probe: prints the pick
agent-qa smart-click "save it"                # last fallback rung resolves
agent-qa smart-fill "the username field" alice
```

## Endpoint contract

`POST <RESOLVE_ENDPOINT>` with `Authorization: Bearer <key>` and body:

```json
{
  "state": "Elements on the page:\n- button \"Save draft\" [e13]\n…",
  "model": "<RESOLVE_MODEL>",
  "questions": {
    "pick": {
      "type": "choice",
      "instructions": "Pick the element the author means: <description>",
      "criteria": {"e13": "button \"Save draft\" — <snapshot line>", "…": "…"}
    }
  }
}
```

Expected response: `{"answers": {"pick": {"choice": "<ref>", "confidence": 0.83}}}`.
Below `RESOLVE_MIN_CONFIDENCE` (or an unknown ref) the plugin answers
`{"ref": null}` and the caller falls back to a normal miss.

## Determinism

Resolution is **authoring-time only**: smart-click/smart-fill record the
picked element's concrete role+name into the scenario, so replays never
invoke the plugin. Removing the plugin changes nothing at replay time —
misses just fail the way they always did.
