# Recording and replay recovery

Recovery has two explicit paths. Neither path autonomously drives the browser
or invents a corrected value.

## 1. Remove invalid recording rows

Use this when a gesture was wrong or later rows no longer describe the live
flow.

1. Snapshot the live page and determine whether the failed gesture already
   changed state.
2. Drive the tab back to step `N`'s pre-state with `agent-browser` primitives
   such as `open`, `click`, `keypress Escape`, `fill`, or `history.back()`.
3. Run:

   ```bash
   agent-qa truncate <N> [--archive-tag <slug>]
   ```

4. Re-record from row `N`, one gesture at a time.

`truncate` drops rows with index `>= N` from
`<record_root>/scenario.steps.jsonl` (normally
`tmp/agent-qa-record/scenario.steps.jsonl`) and archives their sidecars under
`<sid>/failed/truncate-<timestamp>/`. It never
touches the live tab.

## 2. Correct one string value

Use this only when later recording rows remain valid and one buffered do-step
needs a corrected string value.

```bash
# Use an existing run id, or choose a safe manual id and pass it to both calls.
agent-qa heal-respond <sid> --run <runId> --step <stepId> \
  --value '<corrected-string>' --rationale '<why>'
agent-qa heal-apply <sid> --run <runId> --step <stepId> \
  [--target-step <index-or-id>]
```

Then re-position the live tab and re-issue the corrected gesture. `heal-apply`
changes `<record_root>/scenario.steps.jsonl`; it does not change browser
state.

For a replay-only correction, skip `heal-apply` and run:

```bash
agent-qa replay <sid> --heal-from-run <runId> [the original replay flags]
```

That override is transient and does not modify `scenario.json`.

## Choosing a path

| Situation | Action |
| --- | --- |
| Wrong action, wrong target, or invalid later rows | Rewind the tab, then `truncate <N>` and re-record |
| One do-step string value is wrong; later rows remain valid | `heal-respond` + `heal-apply`, then re-drive the corrected gesture |
| One replay do-step string value needs a temporary override | `heal-respond` + `replay --heal-from-run` |
| Locator drift | Re-record or hand-edit through a reviewed external patch; core replay has no autonomous locator-heal loop yet |

See [`heal.md`](./heal.md) for current capabilities and limitations.
