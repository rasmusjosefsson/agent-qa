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
  `agent-qa browser --session <s> find role <role> click --name "<name>"` /
  `agent-qa browser find text "<text>" click` if you need the raw gesture
  without recording. Only fall back to an `@ref` from a fresh
  `agent-qa aria-snapshot` when name-based matching itself is ambiguous.

- **A click or double-click that cannot land fails.** The pinned
  agent-browser rejects `click` and `dblclick` when the target is hidden,
  disabled, covered, or outside the viewport. It does not report success
  for a miss. An overlay error names the covering element (`covered by
  <div#…>`). Dismiss or click that element, `scrollintoview` the original
  target when it was off-screen, then take a fresh snapshot before reusing
  a ref. `click --new-tab` skips these checks. Hover and fill do not; a
  successful hover is still not proof the pointer is over the target.

- **Snapshot refs survive same-document updates and die on navigation.**
  Replaced nodes and navigated pages or iframes invalidate their refs
  without recycling the id. When a ref's element was replaced, agent-browser
  uses the one element with the same role and name. When several match, the
  action fails and clicks nothing — snapshot again and use the new ref.

- **`outcome_unknown` means the command may already have run.** If the CLI
  loses the daemon's reply after the daemon received a click, fill, check,
  or navigation, agent-browser does not send it again. The error code is
  `outcome_unknown`. Snapshot or `get url` before repeating it, or you will
  double-apply the gesture. Read-only commands (`snapshot`, `get url`,
  `get text`) are retried by agent-browser itself. Keep
  `AGENT_BROWSER_DEFAULT_TIMEOUT` at or below `30000` (the default is
  `25000`). A higher value lets the CLI read timeout fire first and turns a
  slow action into `outcome_unknown` plus an EAGAIN stall.

- **`page_wedged` means the tab stopped responding.** A script that never
  returns fails with `page_wedged`. When agent-qa launched the browser, a
  second timeout replaces that tab with a blank one — open the page again
  before the next gesture. A page that recovers in between keeps its tab.
  With `AGENT_BROWSER_CDP` the tab belongs to that browser and is left
  as-is; close the stuck tab there.

- **`smart-click` takes an accessible name, not a snapshot ref.** Supported
  flags are `--role` (default `button`), repeatable `--scope <role> <name>`
  (outermost container first), `--session`, and `--no-record`. It reads the
  accessibility tree before clicking. When the requested role and name match
  more than one node, the command fails without clicking or recording — a
  scope is required. Example: `smart-click "Edit" --scope region Bea` records
  `on.scope` as role locators and activates the control inside that ancestor.
  If the requested role misses and exactly one interactive control has that
  accessible name under another role (`link`, `checkbox`, `menuitem`, `tab`,
  …), that role is clicked and recorded. Several interactive roles for the
  same name fail as ambiguous. Raw text is only the fallback when no
  interactive role+name matches. There is no public `--ref` flag and no `nth`
  disambiguator.

- **Resolve-plugin picks are gated by a confidence floor.** A configured
  `resolve` plugin's pick must report `confidence >= 0.85` (override via
  `AGENT_QA_RESOLVE_MIN_CONFIDENCE`) or it is rejected — a wrong-but-confident
  pick clicks the wrong element and records the wrong locator. Picks with no
  confidence field are trusted.

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

- **`verify` runs the checks that still describe the page — run it BEFORE
  `flush`, not after.** It checks dense step ids, paired snapshot and
  screenshot sidecars, and re-runs the enabled `kind:check` steps that
  follow the last `do` against the live browser session named in the
  recorder state (the same `dispatch_check` path replay uses). A later
  action changes the page, so an earlier claim — a dialog that was open,
  a row that was then deleted — stays in the scenario for replay and is
  not re-run at seal time. When the buffer has no `do`, every enabled
  page check is current. Shot, domshot, layout, and download-file claims
  compare replay artifacts and are not dispatched. `OK  recorder state is clean` means
  those ids, sidecars, and current live claims passed. A current claim
  that is already false fails verify with the reason replay would report.
  When no current page check remains, verify succeeds on ids and sidecars
  with no browser. When a current check's session is not running, verify
  fails with a clear error. `flush` seals the buffer into `scenario.json`
  and clears the active recorder state, so `agent-qa verify` run afterward
  always fails with `no active recording` — that error means you flushed
  already. Sequence: record every step, `agent-qa verify`, then
  `agent-qa flush`, then `agent-qa scenario check <path>` to validate the
  sealed file.

- **Session id ≠ scenario sid, and neither flag you'd guess exists.**
  `start` names the *browser session* with `--session <name>` (there is no
  `--sid`); it generates a scenario sid like `s-<ts>__<rand>` that `flush`
  writes and `scenario rename <sid> <new>` changes afterwards. `flush`
  itself takes flags only — no positional intent or sid. `record-step`
  and `record-setup` take no `--session` either: they append to the one
  active buffer, they never drive a browser.

- **There is no `fill` verb — the verb is `type`.** Drafts like
  `{"verb":"fill"}` fail schema validation (which now runs on the raw
  draft before serde, so a misplaced key inside a locator is caught at
  record time too). `fill-unique`/`smart-fill` are CLI helpers, not step
  verbs; a `type` step's `value` takes `{"from":"literal","literal":"…"}`
  or `{"from":"input","input":"<name>"}`.

- **`find role` accepts `--name`.** This clicks the named control and exits 0
  when that role and name are on the page:

  ```bash
  agent-qa browser --session <s> find role <role> click --name "<name>"
  ```

  `agent-qa browser --session qa-run find role button click --name "Save"`
  is that command against a Save button.

- **Role locators reach into open shadow DOM.** For pages whose inputs
  sit inside shadow roots (e.g. selectorshub's practice page), prefer
  `{role, name}` locators over css — the a11y tree flattens through
  shadow boundaries, and `element` attribute claims on role locators
  read the snapshot's own `value`/`text`/state fields.

- **`--auto-network` is opt-in.** Pass it when the API calls *are* the
  contract (signup hit the backend, the save POST fired). Without it
  flush appends only the landing-URL and pageError gates — a recorded
  nav shouldn't assert every request the page happened to make. When
  enabled it claims distinct XHR/fetch/non-GET calls from inside the
  recording window (OPTIONS/HEAD preflights and probes are never
  claimed — they're browser plumbing, and the telemetry host list drops
  collector beacons). Requests other tabs fire *during* the recording
  can still land on browsers where sessions share one Chrome — skip the
  flag if the claim list shows traffic the page never made.
