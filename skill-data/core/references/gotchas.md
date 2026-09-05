## Gotchas

- **`smart-click` takes an accessible name, not a snapshot ref.** Supported
  flags are `--role`, `--session`, and `--no-record`. It first tries native DOM
  activation by role/name, then agent-browser role/name, text/chunk fallbacks,
  and finally an internally resolved ref from a fresh ARIA snapshot. There is no
  public `--ref` flag, stale-ref cross-check, or ambiguity error contract.

- **A successful `smart-click` dispatch is not proof that app state changed.**
  The command records after the click path returns successfully, but it has no
  post-click state verifier. Snapshot or assert the resulting state when the
  outcome matters. If the command fails after the browser may have changed,
  snapshot before retrying so you do not double-click.

- **Do not double-record helper gestures.** `smart-click` and `fill-unique`
  append their recording rows automatically unless `--no-record` is passed.
  Do not follow a successful helper call with a duplicate `record-step`.

- **Replay has one popup-opener fallback.** For role `combobox` or `listbox`,
  replay counts open dialog/listbox/menu surfaces before native activation. If
  no new popup appears, it focuses the role/name target and presses
  `ArrowDown`. Recording-side `smart-click` does not perform this popup-growth
  probe. Other roles and attributes are not opener signals today.

- **Digit-only accessible-name drift may resolve, but it is not an audited heal
  guarantee.** Role/name DOM activation normalizes digit runs after exact and
  substring matching. There is no generated-suffix strategy, ambiguity
  rejection, per-locator opt-out, strict mode, or heal audit row. If the exact
  name identifies the entity under test, prefer a stable raw locator such as a
  test id.

- **Recording is serial.** Wait for each browser action and its recording call
  to finish before starting the next gesture. The keyframe captures the live
  tab; concurrent actions can make it describe the wrong state.

- **`truncate` is disk bookkeeping only.** `agent-qa truncate <N>` removes
  buffered rows with index `>= N` and archives their sidecars. Re-position the
  live tab yourself before re-recording. The active buffer lives at
  `<record_root>/recorder-state.json`.

- **Manual value correction is explicit.** `heal-respond` records a string
  correction. Feed it to `replay --heal-from-run` for a transient replay
  override, or to `heal-apply` for a recorder-native value-bearing action in
  the active buffer. Core replay does not generate heal requests or locator
  patches automatically. See [`heal.md`](heal.md) and
  [`recovery.md`](recovery.md).

- **Label-based fill can miss wrapper-based form controls.** If
  `agent-browser fill 'Label' '<value>'` cannot resolve a visible input, take a
  fresh snapshot and use that input's current ref for the one browser action;
  record the durable role/name or raw locator separately.

- **Agent-browser daemon recovery is automatic once.** When the named daemon is
  alive but its child browser is gone, agent-qa closes that session and retries
  the original command once. Opt out with
  `AGENT_QA_AGENT_BROWSER_NO_AUTO_RECOVER=1` when debugging the daemon. If
  recovery still fails, use `agent-browser close --session <name>`, then
  `agent-browser close --all`, then `agent-browser doctor --fix`.

- **Run `verify` before declaring a recording complete.** It checks the sealed
  scenario and its recording evidence. A row on disk or a successful browser
  gesture alone is not completion evidence.
