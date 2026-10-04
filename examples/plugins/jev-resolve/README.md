# jev-resolve — the `resolve` plugin backed by Jev (typesafe.ai)

Answers the `resolve` kind's question — "which of these page elements did
the author mean?" — by POSTing a single choice question to Jev's System
One decision API. Jev is a structured decision model (not a generative
LLM): it returns a typed choice + confidence over the candidates it's
given, which makes authoring-time element picking fast and deterministic.

## Wire it up

**Easiest — the workbench:** `agent-qa web` → **Extensions** →
**Element resolution → Jev (typesafe.ai)** → paste your API key →
**Save & enable**. This binary ships inside the `agent-qa` package, so
the server registers it and stores the key in `_config/jev.json` for
every workbench-spawned call (chat agent, editor, replay).

**From the CLI:**

```toml
# agent-qa.toml
[plugins]
resolve = "/path/to/examples/plugins/jev-resolve/agent-qa-plugin-jev-resolve"
```

```sh
export TYPESAFE_API_KEY="…"   # console.typesafe.ai → API keys
                              # shell env, CI secret (GitHub Actions), or vault —
                              # the key never lives in git
# optional overrides:
export RESOLVE_MODEL="jev-latest"           # default
export RESOLVE_MIN_CONFIDENCE=0.5           # default
export RESOLVE_ENDPOINT="…"                 # default api.typesafe.ai/v1/systemone
export RESOLVE_KEY_ENV="MY_KEY_VAR"         # if your key lives under another name
```

Then, during a recording session:

```sh
agent-qa resolve "the save button"            # probe: prints the pick
agent-qa smart-click "save it"                # last fallback rung resolves
agent-qa smart-fill "the username field" alice
```

## What the plugin sends

`POST https://api.typesafe.ai/v1/systemone` with
`Authorization: Bearer $TYPESAFE_API_KEY` and body:

```json
{
  "state": "Elements on the page:\n- button \"Save draft\" [e13]\n…",
  "model": "jev-latest",
  "questions": {
    "pick": {
      "type": "choice",
      "instructions": "Pick the element the author means: <description>",
      "criteria": {"e13": "button \"Save draft\" — <snapshot line>", "…": "…"}
    }
  }
}
```

Jev answers `{"answers": {"pick": {"choice": "<ref>", "confidence": 0.83}}}`.
Below `RESOLVE_MIN_CONFIDENCE` (or an unknown ref) the plugin answers
`{"ref": null}` and the caller falls back to a normal miss.

## Determinism

Resolution is **authoring-time only**: smart-click/smart-fill record the
picked element's concrete role+name into the scenario, so replays never
invoke the plugin — and never need a key. Remove the plugin (or unset the
key) and misses just fail the way they always did.
