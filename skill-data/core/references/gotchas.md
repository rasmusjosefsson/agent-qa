## Gotchas

- **`AGENT_BROWSER_CDP` is BYO-only. Do not set it by default.** If every
  `agent-browser`/`agent-qa` command fails with `Connection refused` on the
  same port, the cause is almost always a stale or speculative
  `AGENT_BROWSER_CDP` export pointing at a port nothing is listening on — not
  a stale daemon, a version mismatch, or a browser that needs to be launched
  by hand. Unset the variable and retry with a plain `agent-qa browser
  --session <name> open <url>`; agent-qa/agent-browser launch and own the
  browser process themselves in the default (non-BYO) case. Only set
  `AGENT_BROWSER_CDP` when you deliberately started an external Chrome with
  `--remote-debugging-port` yourself.

- **Drive gestures with `agent-qa browser <args>`, not a bare `agent-browser`
  command.** Every agent-qa subcommand resolves and pins one exact
  `agent-browser` binary (`AGENT_BROWSER_BIN`, wired by the Node launcher
  from the installed npm sibling). A bare `agent-browser` shell command
  instead resolves whatever is on `$PATH`, which can be a different version
  from a separate `npm i -g agent-browser` install. Two versions driving the
  same named session daemon trip "Daemon version mismatch, restarting" on
  every call, and each restart wipes the tab back to blank mid-recording —
  this looks like flaky navigation or a broken click target, but the real
  cause is the version split. `agent-qa browser <args>` execs the exact same
  pinned binary every other verb uses, so this can't happen.

- **`agent-qa browser click <target>` takes a CSS selector or `@ref`, NOT an
  accessible name.** `agent-browser`'s own `click` verb resolves a selector or
  a snapshot `@ref` — `agent-qa browser click "Learn more"` fails with
  `Element not found`, even when that's the exact visible link text. For a
  name-based click, either use `agent-qa smart-click "<name>"` (does the
  fallback ladder AND auto-records — prefer this), or
  `agent-qa browser find role <role> click --name "<name>"` /
  `agent-qa browser find text "<text>" click` if you need the raw gesture
  without recording. Only fall back to an `@ref` from a fresh
  `agent-qa aria-snapshot` when name-based matching itself is ambiguous.

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

- **Recording is serial — never fire two commands against the same session at
  once.** Wait for each browser action and its recording call to finish before
  starting the next gesture, even the very first one. A session's daemon does
  not exist until the first command touches it; two calls racing to start it
  (e.g. a `browser open` and a `record-step` issued in the same batch) can
  land as "daemon started concurrently with different configuration" — just
  retry serially, one command at a time.

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
  `agent-qa browser fill 'Label' '<value>'` cannot resolve a visible input,
  take a fresh snapshot and use that input's current ref for the one browser
  action; record the durable role/name or raw locator separately.

- **Agent-browser daemon recovery is automatic once.** When the named daemon is
  alive but its child browser is gone, agent-qa closes that session and retries
  the original command once. Opt out with
  `AGENT_QA_AGENT_BROWSER_NO_AUTO_RECOVER=1` when debugging the daemon. If
  recovery still fails, use `agent-qa browser close --session <name>`, then
  `agent-qa browser close --all`, then `agent-qa browser doctor --fix`.

- **`verify` only inspects the active (unsealed) recording buffer — run it
  BEFORE `flush`, not after.** `flush` seals the buffer into `scenario.json`
  and clears the active recorder state, so `agent-qa verify` run afterward
  always fails with `no active recording` — that error means you flushed
  already, not that verify or the recording is broken. Sequence: record every
  step, `agent-qa verify`, then `agent-qa flush`, then `agent-qa scenario
  check <path>` to validate the sealed file.
