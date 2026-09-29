# Gap map — what agent-qa covers today, and what's still open

A snapshot audit of the whole surface (verbs, claims, evals, CI, workbench)
with the gaps ranked by how much they'd hurt a real user. Updated each time
the picture shifts materially.

## Surface inventory

| Layer | What exists |
| --- | --- |
| Do verbs (~40) | navigation (`goto`/`reload`/`back`/`forward`), input (`click`/`dblclick`/`rightclick`/`type`/`clear`/`press`/`hover`/`select`/`check`/`uncheck`/`upload`/`focus`/`blur`/`drag`/`scrollTo`), touch gestures (`hold`/`swipe`/`pinch`/`rotate`), dialogs (`dialog`), files (`download`/`fileChooser`), system (`wait`/`read`/`state`/`mock`/`unmock`/`callGql`/`tab`/`viewport`), emulation (`frame`/`emulate` incl. device/geo/tz/locale/media/offline/headers/credentials), structure (`loop`/`group`/`useTemplate`) |
| Claim subjects (~16) | `element` (text/value/count/attribute incl. `style:<prop>` + `focused`), `url`, `network` (fired/status/responseJsonPath/postDataContains, ws/sse frames), `data`, `flag`, `dialog`, `file`, `shot` (visual diff, clip, mask, AA-tolerant), `domshot` (ARIA text snapshot, skip-regex), `storage`, `cookie`, `console`, `pageError`, `a11y` (axe-core), `timing` |
| Record vocabulary | ~50 methods covering every verb + claim kind; parity-gate tested; auto-claims for network + page errors on flush (default on) |
| Suite flags | `--all`/`--shard`/`--filter`/`--tags`/`--jobs`, `--retry`/`--until-fail`/`--watch`/`--keep-going`, `--har`/`--offline`/`--mock-from`/`--freeze`/`--base-url`, `--auto-promote`/`--update-baselines`, `--junit`/`--report`/`--record-video`, `--from`/`--until`, `--persona`/`--environment` |
| Audit | `flaky`, `slow`, `heal-chronic` (+`--all`), `verdict` (+`--all`), `cluster`, `trend` (+`--all`), `health`, run-vs-run compare (CLI + workbench) |
| Lint | `no-visual-check`, `shot-without-baseline`, `domshot-without-baseline`, `orphan-baseline`, `brittle-locator`, `fixed-sleep`, `check-all` in smoke |
| CI | `qa-gate` (fixture goldens + sticky verdict + run-report artifacts), `ui-goldens` (visual gate w/ embedded before/after/diff images), `qa-crawl` (draft coverage on UI PRs), `qa-adopt` + `/qa accept` commands, composite `action.yml` (+npm install mode, +app-under-test boot), `evals-nightly`, changelog-driven releases |
| Golden suites | ~30 QA Playground pages, ~34 the-internet edge cases (six sweeps), saucedemo suite (login/sort, full 21-step purchase, negative auth, logout, cookie+storage lifecycle), expandtesting (login round-trip, dynamic table, infinite scroll), todomvc (stateful SPA), demoqa widgets, httpbin hermetic-mock loop, workbench selftest goldens, quotes.toscrape.com (pagination, HttpOnly cookie claims, scroll offsets), parabank (registration with `{{vars._unique}}`, login/logout, profile update — volatile-URL claims normalized), demoblaze (category filters, add-to-cart alert claims, cart session persistence across reload, full purchase flow), automation-exercise (signup+cart lifecycle), wikipedia (search nav, TOC, history, REST API claims), formy (full form, bootstrap modal, jQuery datepicker, JS dropdown), testpages (ajax cascade, form POST echo, native dialogs, onblur validation), coffee-cart (cart badge, promo modal, checkout form, quantity steppers), globalsqa XYZ Bank (AngularJS login, deposit/withdraw, transactions ledger, manager console), hackernews (live HN API claims), selectorshub (shadow-DOM fills + snapshot attribute reads), practicesoftwaretesting (search, cart, login + QUERY-method API claims), lambdatest OpenCart (GET-form search with percent-encoded routes, hidden sticky-bar twins, delegated jQuery cart POST, cart page quantity rows) |

## Ranked gaps

### P1 — the golden loop is asymmetric for domshot

1. ~~**`/qa accept` only re-mints `shot` baselines**~~ — #286 re-mints
   domshots too; `accept.ts` covers both golden kinds.
2. ~~**Coverage metric counts shots, not domshots**~~ — #287 adds
   `golden%` counting domshot baselines alongside shots (scenario +
   coverage-all + Cases badge).
3. ~~**WS/SSE capture (#278) has no golden**~~ — #278 itself shipped
   `evals/fixtures/ws.html` + `evals/golden/ws-tc01.ts`: a Bun.serve echo
   socket + SSE endpoint asserting `wsPayloadContains` and SSE-as-200.

### P2 — record path still trails replay

4. ~~**Touch/gesture capture**~~ — #291 adds a Gesture click-mode to the
   live pane: press+drag classifies into hold/swipe/pinch/rotate around
   the hovered element's box (heading change + orbit radius separate
   rotate from through-center swipes), dispatches via `run-step`, and
   records a validated do-draft. Remaining gap: real-device multi-touch
   recording would need CDP touch-event bridging — desktop-classified
   gestures cover the synthetic case.
5. ~~**Hermetic capture**~~ — #292 adds `start --mock-from <har>` and
   `start --offline`: the session's mock registry is seeded and the
   stub+reject init script is registered via `AGENT_BROWSER_INIT_SCRIPTS`
   before the browser opens, so every navigation of the recording is
   hermetic, not just the first page.
6. ~~**Live-input divergence**~~ — could not reproduce on
   agent-browser 0.37.1: a real `click` on saucedemo's add-to-cart
   updates React state (cart badge 1→2). Likely fixed upstream; drop the
   watch item unless a counter-example resurfaces.
7. **Untouched-recordable surfaces** — `dialog` now records during
   live capture (#293: the bridge answers the opening dialog — accept,
   with the page's own `defaultPrompt` for prompts — then emits the
   check `{"dialog": true}` + `do/dialog` pair in the order replay
   expects). All four remaining surfaces have capture PRs open: file
   uploads (#263 merged; `download` = #307), iframes (`frame` move
   emission = #295), window resizes → `do/viewport` (#297), and a
   `state`-seed action (`capture page state` = #294). Structural verbs
   (`loop`/`group`/`useTemplate`) stay authored by design.

### P3 — polish

8. ~~**`domshot` in the workbench selftest goldens**~~ — #288 adds a
   domshot claim to selftest-settings, the first structural golden.
9. ~~**heal-chronic → issue handoff**~~ — #290 adds `--issue`: a
   paste-ready markdown handoff (table + promote block). Auto-filing
   stays out on purpose — the debt list is small enough to paste.

### P4 — upstream capture limits

10. ~~**Navigation-redirect statuses are uncapturable**~~ — #317 adds an
    own-CDP `Network.*` capture client (`cdp_net.rs`): per-request
    `NetEvent` records keep redirect hops, `redirect_entries()` mints
    `cdp-<id>` synthetic CapturedRequests for each 3xx hop, and
    `find_completed` is OR'd into `wait_for_resource` polls.
11. ~~**`wait url` can't match in-flight requests**~~ — same PR: the
    own-CDP poll sees requests while still pending, so a slow endpoint
    (>15s cold, e.g. demoblaze `/signup`) matches as soon as it lands.
12. **Silent clicks on unhittable elements** — `agent-browser click`
    reports success when the element's centre fails `elementFromPoint`
    (below fold / zero-size / covered): the click dispatches to nothing
    and the recorded scenario replays the miss. Seen on three sites
    (formy submit anchor, testpages dialog buttons, buggy register).
    Golden layer: `ensureHittable` (#321). Replay layer: #324 hit-tests
    every raw-locator coordinate action (click/hover/focus/fill/dblclick/
    xpath) — scrolls + warns `[v2-replay] ... is unhittable` so the miss
    is visible in the step log. Still open: role-locator hover/fill go
    through `find_role_act` unguarded (resolving role+name in-page needs
    the role engine), and a daemon-side fix upstream would be the real
    close.
13. **Record-side resolution is still first-match** — replay prefers the
    first *visible* css match (#348), but the live `agent-browser`
    `click`/`wait`/`fill` that golden runners drive keep first-match
    semantics: a hidden twin refuses the trusted click (`covered by
    #main-header`) or hangs `wait` forever. Lambdatest exposed this on
    the record path — worked around with `clickSelectorForce` (eval
    `.click()` on the first match, safe because the twins share the
    delegated handler). Real closes: an upstream prefer-visible in
    agent-browser, or a record-side helper that resolves visibility
    first and drives the chosen node by ref.

### P5 — uncovered surfaces (smaller, real)

14. **IndexedDB is not assertable** — `storage` claims cover localStorage/
    sessionStorage and `cookie` covers the CDP jar; IndexedDB (the store
    real apps actually use for offline data) has no claim or seeding
    path.
15. **Clipboard** — no claim or step can read/set clipboard contents, so
    copy-to-clipboard UX can't be covered.
16. **Service-worker-served responses** — cache-first PWAs answer from
    the SW without a `Network.*` hit on the page target; `network`
    claims on those endpoints would time out. Untested whether the
    own-CDP client sees SW-target traffic — needs a probe.
17. **Closed shadow roots are unreachable** — role locators pierce *open*
    roots via the a11y tree; closed roots hide everything by design.
    Worth documenting as a hard limit rather than a gap to fix.
18. **`set` toggles without a clear state** — #349 resets `offline` +
    `headers` at run start, but `viewport`/`device`/`geo`/`credentials`/
    `media` still leak across replays on a reused session with no
    `set`-level off. Needs either upstream clear verbs or a tracked
    reset baseline in the runner.

## Entry-point dogfood pass (`init → start → record-step → buffer → flush → replay → audit`)

Driving the CLI as a fresh user surfaced a class of papercuts the golden
suites couldn't see — golden runners assemble scenarios programmatically,
so the file-based verbs and their defaults were never exercised:

- **`record --open <url>` sealed nothing** — the URL only navigated the
  warm browser, so `env.open` was `[fresh]` and a cold replay (or
  `--offline`, which skips the cached session) started on about:blank.
  #329 seals `--open` as an `EnvOp::Nav` in the recorder state.
- **Every `scenario <verb>` wanted a file path, not a sid** — `scenario
  check hello` read `hello` as a literal path. #328 adds
  `scenario_file_arg`: `-`, existing paths, then `<scenarios>/<arg>/
  scenario.json` sid lookup.
- **Authoring a `do` step by hand needed the full `Locator` object** —
  three failure modes hit in sequence (`missing intent` → `requires on`
  → untagged-enum decode errors on every sane guess at the shape).
  #330's shorthand `"on": "css:.x" | "xpath:…" | "testId:…" | "text:…"`
  lowers to a named raw locator at deserialize time.
- **`audit <verb> <sid>` required an explicit runId** — #331 defaults
  it to `latest` (the already-stamped replays/latest.txt or lex-max dir).
- **`record pause`/`resume`, `buffer check`/`load`/`move`/`edit`/
  `discard`, `scenario copy`/`rename`/`delete`/`prune-replays`,
  `doctor --deep`, `init --ci`, `replay --mock-from recorded --offline`,
  and the full `audit` family** — all verified working end to end on a
  recorded session in `/tmp/e2e-dogfood`.

### What the dogfood pass says about the remaining surface

- The **verb-level** surface is mature; the remaining sharp edges are all
  in the **file/argument affordances** around it — defaults that assume
  workbench context (`latest.txt`), paths where a sid is the natural
  handle, schema shapes that are fine for JSON tooling but hostile to a
  hand-edited draft.
- **Cold-vs-warm divergence is the trap to keep testing**: anything that
  "works" while the recording session is still alive must be replayed
  with `record` stopped (or `--offline`) to prove it doesn't rely on
  daemon state.
- **Where authoring friction remains**: `record-step` still requires the
  caller to know step-shape (`intent`, claim subject spelling); a
  `record-step --from-stdin` batch mode or `scenario new --from-har`
  would shorten the LLM authoring loop further — both are deliberately
  unbuilt until a second dogfood wave proves which one earns it.

## Dogfood pass II — flag consistency + claim authoring

- **`-h`/`--help` is now positional-safe** (#332): `init` had no help arm;
  `record`/`buffer` only checked `args[0]`; scenario's file-taking
  subverbs swallowed `--help` as a path (`read --help: No such file`).
  All verbs now treat help-anywhere as usage+exit 0.
- **`record-setup` storage ops on a never-navigated session produce an
  unreplayable scenario** — `[fresh, localStorage]` with no `nav` op runs
  `localStorage` on about:blank and dies `SecurityError` at env.open.
  #329's nav-seal fixes the common case (a `--open` session); still open:
  a `record-setup` cookie/localStorage on a no-nav recording. Options:
  warn at setup time ("no nav op precedes this storage op"), or have the
  replay env-runner skip storage ops on opaque origins with a visible
  warning. Not fixed — needs a decision on which layer owns it.
- **Claim predicate sugar** (#333): `{"predicate":{"contains":"x"}}`
  lowers to predicate+value; unary-in-object and doubled-value are
  explicit errors. Second most-common hand-author miss after locators.
- **`record-step` takes no `--session`** (appends to the one active
  buffer, never drives a browser) and **`flush` takes no intent arg**
  (intent is `start`'s job) — both error with usage, consistent.
- **`run-step`/`smart-click` verified live** — role click dispatches
  (`named control click`), check drafts evaluate (`url contains`),
  smart-click's miss lists the real accessible names
  (`none match name "More information...". Names seen: "Learn more"`).
- **`aria-snapshot`, `perf-snapshot`, `record continue --skip-replay`,
  `truncate`, `buffer discard`, `verify`, `list`, `info`, `config`,
  `plugins`, `skills` — all clean.

## Dogfood pass III — role locators + shadow DOM (#342–#344)

- **a11y snapshot `@eN` refs ARE driveable** — `agent-browser click @e65`
  / `fill @e65` act on the ref directly, and `find role <r> --name` does
  not exist (`--name` is rejected): the name-filtered path is
  `--json snapshot -i` → `data.refs` (a `ref → {role, name}` map), then
  drive the ref. Earlier notes that refs couldn't be driven were wrong.
- **Role locators pierce open shadow roots** — the a11y tree flattens
  through them, so `type`/`click` on `{role, name}` reaches inputs no css
  selector can see (selectorshub xpath-practice-page, shub-tc01). The
  `@eN` drive makes the record path possible too.
- **Element `attribute` claims on role locators read the snapshot tail**
  (#342): `value`/`text`/`focused`/`FLAG_STATES`/`NUMERIC_STATES` resolve
  from the live a11y tree — the only claim path that reaches inside
  shadow DOM without a piercing selector.
- **Draft typos fail at record, not replay** (#343): `record-step`/
  `run-step`/stdin JSONL validate the *raw* draft against the schema
  before serde — a misplaced key inside a locator (e.g.
  `{"role":...,"attribute":"x"}`) now dies at record with the offending
  instance printed instead of a misleading predicate error later.
- **Ad/telemetry-heavy pages flood `--auto-network`** — selectorshub
  emits google-docs/play.google.com beacon claims on flush; the beacon
  filter (#315) covers same-origin `/cdn-cgi/rum` but not third-party
  hosts. For noisy pages flush `--no-auto-network` (shub-tc01 does via
  `flushArgs`) and hand-author the claims worth keeping.
- **`start` has no `--sid`; `flush` has no positional arg** — sids are
  generated (`s<ts>`); rename post-flush with `scenario rename`. Both
  flag guesses error cleanly, but agents reach for them every time —
  worth a skill-doc line.

## Lessons from the fresh-site sweeps

Real sites taught durable patterns:

- **Volatile URL pieces hide in three places** — query strings (already
  stripped), `;k=v` matrix params, and id-bearing path segments. Flush's
  auto-network claims now normalize all three (digits → `\d+`, hex/uuid
  → class regex). If a site invents a fourth carrier, extend
  `normalize_volatile_path_segments`.
- **Dialogs that follow an XHR are async** — `assertDialogText` reads
  the pending dialog *at record time*, so a click whose handler alerts
  after an ajax call needs a settle wait (`waitMs` ~2s, or
  `waitRequest` on the endpoint) between click and assert.
- **Timed waits must not touch the page** — `do:wait {ms}` slept via
  `agent-browser wait` until #310; a pending dialog wedged it. Now a
  pure `thread::sleep`.
- **Frameworks bind late and submit eagerly** (testpages/xyz, #320/#323)
  — ajax handlers may bind after `load` (bounded `waitMs` is the honest
  gate), AngularJS un-hides the submit only once the model is set (wait
  on `:not(.ng-hide)`), and a synthetic click can land before `ng-submit`
  attaches — `clickSelectorForce` (eval `el.click()`) is the reliable
  submit for that generation.
- **Page state is in-memory on some SPAs** (coffee-cart, #322) — a
  second `open` reloads the app and empties the cart; in-app link clicks
  preserve it. And SPAs often render hidden duplicates (a
  `ul.cart-preview` shadowing the real list's `+/-` buttons) — scope
  row-action selectors to the visible list or the click misses.
- **`{{vars._unique}}` templates survive into the scenario** —
  fillSelector's recorded `fillBySelector` keeps the template so replay
  mints fresh uniqueness per run (parabank registration, xyz last name).

## Dogfood pass IV — OpenCart (lambdatest): hidden twins + daemon state

Record→replay on a server-rendered OpenCart store surfaced a systemic
class the sweep list hadn't hit yet:

- **Hidden duplicate controls are everywhere on real storefronts** —
  `button.btn-cart` exists twice per product page; the first DOM match
  is a 0×0 sticky-bar twin covered by `#main-header`. Replay-side fix
  (#348): every css resolution for an action target picks
  `els.find(visible) || els[0]`. Record-side is still first-match (gap
  13). The same page puts a visible *category dropdown-toggle* before
  the submit button inside the search form — a comma selector's "first
  visible" pick hits the wrong control; tighten to the actual submit
  (`form button.type-text`).
- **`agent-browser set` toggles are daemon-side and persist across
  replays** — a leftover `offline on` silently drops every request
  while the warm-goto skip means no navigation ever fires, so
  `network.json` is empty and `{"network"}` claims time out with a
  misleading `no request matched`. #349 resets `offline`+`headers` at
  run start; viewport/device/geo/credentials/media still lack clears
  (gap 18).
- **The deferred click dispatch can outlive its node** — the setTimeout
  that dodges dialog-blocking evals captured `el` before the theme's
  hydration re-render could detach it; it now re-resolves inside the
  timeout (also #348).
- **GET forms percent-encode route params** — OpenCart search submits
  to `route=product%2Fsearch` (encoded slash). URL and network regexes
  need `product(%2F|/)search`; the same applies to any server that
  encodes slashes in params.
- **`press Enter` doesn't submit every form** — the search input's
  Enter handler is a no-op on this theme; a real submit-button click
  is required. When a type+Enter pattern stalls on a new site, reach
  for the submit control first.

## Recently closed (for orientation)

- OpenCart/lambdatest findings: prefer-visible css resolution + deferred
  click re-resolve (#348), persistent-emulation reset at run start +
  `network requests --clear` dedupe (#349), lambdatest goldens (#350)
- selectorshub shadow-DOM sweep (#344), practicesoftwaretesting sweep
  incl. QUERY-method API claims (#345), agent-driveable `@eN` refs +
  gotchas skill notes (#346), record-translate dead-arm cleanup (#347)
- Authoring ergonomics dogfood (#325–#341): `init` literal fix +
  claim-value lint, `run-report`/`compare`/`audit` `latest` resolution,
  scenario readers take sids, locator shorthand (`css:…`), `-h` anywhere,
  predicate sugar, role/xpath `exists` probes, `record-step` draft hints
  + `smart-fill`, buffer `s<N>` ids, `--root` on `*-all` verbs,
  `record-setup -` JSONL, `record-setup` nav warning
- Role-locator attribute claims via the a11y snapshot (#342);
  record-time schema validation of raw drafts + `deny_unknown_fields`
  on locator variants (#343); selectorshub shadow-DOM sweep (#344)
- Touch verbs: `hold`+`swipe` (#259), `pinch` (#262), `rotate` (#267);
  `rightclick` (#266) + live-pane capture (#268); record-side gesture
  classification (#291)
- Geo/tz/locale emulation via own pooled CDP client — grant +
  `Emulation.setGeolocationOverride` on the active target (#277), +
  `setTimezoneOverride`/`setLocaleOverride` (#279)
- WS/SSE frames in `network` claims (#278)
- `wait locator` — poll DOM state before continuing (#282)
- `domshot` claim + `domshot-accept` (#283) + workbench diff
  card/re-mint/insert-check/accept-all (#284); `/qa accept` covers
  domshots (#286); `golden%` counts them (#287); selftest domshot
  golden (#288); `heal-chronic --issue` handoff (#290);
  `AGENT_BROWSER_BIN` bare-name PATH resolution (#289)
- Trusted-mouse `drag` + demoqa widgets goldens (#281)
- Crawl telemetry pruning + `crawl` golden (#272); `fixed-sleep` +
  `brittle-locator` lint (#273/#271)
- todomvc + expandtesting + httpbin sweeps (#269/#275/#274)
- `run-report.html` artifacts in qa-gate (#260); qaplayground skill
  edge-sweep docs (#261); `timing` golden (#276)
- Dialog tolerance on dialog-opening clicks (#246); record-side dialog
  capture — answer + check/do pair (#293); `start --mock-from`/
  `--offline` hermetic recording (#292)
- Auto-claims on flush: `--auto-network`/`--auto-errors` (#252); flush
  skips telemetry beacons incl. same-origin `/cdn-cgi/rum` (#315)
- Fresh-site sweeps III–VII: orangehrm SPA auth (#311), books.toscrape
  (#312), practicetestautomation (#313), react-admin MUI SPA with
  cross-origin API claims + hidden-popover trap (#314), automationcamp —
  record-driven `frame` verb, delayed dialogs, visibility triggers
  (#316)
- Cookie/storage goldens + `do/state` seeding (#253)
- Own-CDP `Network.*` capture — redirect-hop statuses + in-flight
  `wait url` (#317)
- Sweeps VIII–XI: wikipedia (#318), formy (#319), testpages (#320),
  coffee-cart (#322), XYZ Bank AngularJS (#323); `ensureHittable`
  click-guard in the golden lib (#321); site skip list in the
  eval-loop skill (#323)
- `pageError` + `console` + `a11y` claim subjects (#248/#167/#240)
- Network claims fired/status/json (#143), postDataContains (#205),
  `wait url` (#166)
- Visual loop: shot claims + clip + mask + diff maps + re-mint + `/qa
  accept` (#129–#186)
- Suite tooling: parallel `--jobs`, `--retry`, `--until-fail`, `--watch`,
  JUnit, HTML report
