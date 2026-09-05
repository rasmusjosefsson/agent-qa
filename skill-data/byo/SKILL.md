---
name: byo
description: Status of agent-qa browser attachment support.
---

# Browser attachment status

`agent-qa` does not implement `--byo`, `--launch`, `--port`, `--tab`, or
`--clone-profile`. Do not suggest or run those flags.

Use `agent-qa byo-doctor` to inspect local browser availability. It does not
attach or drive a browser.

For an external CDP browser, configure agent-browser before starting a
recording. Set both values for every direct `agent-browser` command and every
`agent-qa` command.

```bash
export AGENT_BROWSER_CDP=9223
export AGENT_BROWSER_PIN_TAB=1
```

You can put the same local settings in `agent-qa.toml`.

```toml
[browser]
cdp = "9223"
pin_tab = true
```

`agent-qa start` saves the resolved connection in local recorder state. It
passes that connection to every browser child during recording. The endpoint
and pin policy never enter `scenario.json`.
