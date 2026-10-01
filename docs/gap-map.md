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
| Golden suites | ~30 QA Playground pages, ~34 the-internet edge cases (six sweeps), saucedemo suite (login/sort, full 21-step purchase, negative auth, logout, cookie+storage lifecycle), expandtesting (login round-trip, dynamic table, infinite scroll), todomvc (stateful SPA), demoqa widgets, httpbin hermetic-mock loop, workbench selftest goldens, quotes.toscrape.com (pagination, HttpOnly cookie claims, scroll offsets), parabank (registration with `{{vars._unique}}`, login/logout, profile update — volatile-URL claims normalized), demoblaze (category filters, add-to-cart alert claims, cart session persistence across reload, full purchase flow), automation-exercise (signup+cart lifecycle), wikipedia (search nav, TOC, history, REST API claims), formy (full form, bootstrap modal, jQuery datepicker, JS dropdown), testpages (ajax cascade, form POST echo, native dialogs, onblur validation), coffee-cart (cart badge, promo modal, checkout form, quantity steppers), globalsqa XYZ Bank (AngularJS login, deposit/withdraw, transactions ledger, manager console), hackernews (live HN API claims), selectorshub (shadow-DOM fills + snapshot attribute reads), practicesoftwaretesting (search, cart, login + QUERY-method API claims), lambdatest OpenCart (GET-form search with percent-encoded routes, hidden sticky-bar twins, delegated jQuery cart POST, cart page quantity rows), bonigarcia selenium-webdriver-java (GET form submit, open shadow DOM text, jQuery UI mouse drag, native dialogs + Bootstrap modal, web storage seeding), petstore.octoperf.com JPetStore (Struts catalog browse, add-to-cart POST, sign-in round-trip, full order placement with `;jsessionid` matrix-param URLs) |

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

9. ~~**Mobile/touch**~~ — gesture verbs shipped: `hold`/`swipe` (#259),
   `pinch` (#262), `rotate` (#267); `contextmenu` recording catches real
   right-clicks (#268). Remaining: no multi-touch *record* path (verbs
   are authored, not captured).
10. ~~**Geolocation/timezone**~~ — geo landed in #277 (origin-scoped
    permission grant + flat-session override); `timezone`/`locale` ride
    the same path (setTimezoneOverride + setLocaleOverride +
    UA acceptLanguage — navigator.language, Intl, and the wire header
    all covered; emulate-tc03 proves all three).
11. ~~**WebSocket/SSE**~~ **done in #278** — sockets and event-streams
    fold into the network claims surface as `cdpws-*` entries (method
    `WS`, status 101, `wsFrames[]` per frame, `wsPayloadContains`
    matcher; SSE arrives as `resourceType: "EventSource"` GETs). The
    daemon is untouched — a pooled CDP flat session on the active page
    target does the listening.
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
    is visible in the step log. Role-locator hover/fill now hit-test
    the resolved element too (#372) — a covered/offscreen target warns
    instead of silently missing. A nameless role locator
    (`{"role":"button"}`) still dispatches first-match, but the
    ambiguity is now visible: when the snapshot shows >1 candidate the
    step logs `role='X' has no name and matches N nodes` (#388).
    Remaining: a daemon-side fix upstream would be the real close.
13. ~~**Record-side resolution is still first-match**~~ — closed for the
    golden runners by `evals/golden/visible.ts` (#356): `pickVisible`
    makes the same visibility pick replay does, `clickVisibleEval`
    dispatches mousedown/mouseup/click on it, and `clickTrustedOrVisible`
    keeps the trusted `agent-browser click` with an unhittable-triggered
    eval fallback — wired through every lib's `clickSelector`/
    `domClickSelector`/`clickSelectorForce`. Lambdatest exposed this on
    the record path — previously worked around with a first-match eval
    click. Still open upstream: `agent-browser`'s own `click`/`wait`/
    `fill` keep first-match semantics for hand-driven flows, and live-
    pane recording resolves whatever DOM node the user physically
    clicked (correct by definition).

### P5 — uncovered surfaces (smaller, real)

12. ~~**`run-report` in CI**~~ — qa-gate renders + uploads per-run
    `run-report.html` artifacts (#260).
13. ~~**Skill docs for edge sweeps**~~ — the qaplayground skill documents
    the edge-pages-lib + fixture-server pattern (#261).
14. ~~IndexedDB is not assertable~~ — closed by the `{"indexeddb": {"db",
    "store","key"?}}` claim + `do/state` `params.indexeddb` seeding
    (#353). The probe lists `indexedDB.databases()` before opening so
    `notExists` never creates the db; seeding creates missing stores via
    a version bump.
15. ~~Clipboard~~ — closed by the `{"clipboard": true}` claim +
    `do/state` `params.clipboard` seeding (#354). Reads go through
    origin-scoped `Browser.setPermission` (`clipboard-read`, `+write`)
    and focus emulation over the pooled CDP connection — both required
    headless; `file://` pages have no grantable origin. Empty clipboard
    maps to `null` so `exists` means "holds text".
16. ~~Service-worker-originated fetches~~ — closed. `cdp_net` now arms
    `Target.setAutoAttach` on its capture socket; every worker-class
    target (service/dedicated/shared worker, plus pages) gets
    `Network.enable` on its own session, and worker-sourced requests
    merge into `network_requests` tagged `resource_type: "worker"` —
    `{"network"}` claims and `wait url` cover cache-fill and
    background-sync traffic the page never touched. requestIds are
    keyed `session:id` since they're only unique per target.
17. **Closed shadow roots are unreachable** — role locators pierce *open*
    roots via the a11y tree; closed roots hide everything by design.
    **Hard limit, not a gap**: no CDP/JS surface can see inside a closed
    root (its `shadowRoot` is null for everyone). Documented as
    permanent — closed.
18. **`set` toggles without a clear state** — narrowed, not closed.
    `do/emulate` now accepts `"<key>": "off"` for every runner-side key
    (`device`/`geo`/`timezone`/`locale`/`headers`/`colorScheme`/
    `reducedMotion`/`permissions`/`offline`) — the step issues the CDP
    clear on the runner's own pooled connection, which is what owns the
    overrides. Two honest limits: `credentials: "off"` bails (its auth
    handler isn't a clearable header — use `--fresh-browser`), and an
    `agent-browser set`-applied toggle can still not be cleared from a
    second CDP session — `--fresh-browser` remains the escape hatch.

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
  caller to know step-shape (`intent`, claim subject spelling). The
  batch half closed — `record-step -`/`record-setup -` read JSONL
  drafts from stdin (#338/#341), and `scenario new --from-har` (#395)
  mints a nav skeleton from a captured HAR for the send-us-your-bug
  flow.

## Dogfood pass II — flag consistency + claim authoring

- **`-h`/`--help` is now positional-safe** (#332): `init` had no help arm;
  `record`/`buffer` only checked `args[0]`; scenario's file-taking
  subverbs swallowed `--help` as a path (`read --help: No such file`).
  All verbs now treat help-anywhere as usage+exit 0.
- **`record-setup` storage ops on a never-navigated session produce an
  unreplayable scenario** — `[fresh, localStorage]` with no `nav` op runs
  `localStorage` on about:blank and dies `SecurityError` at env.open.
  #329's nav-seal fixes the common case (a `--open` session), and #337
  added the setup-time warning ("no nav op precedes this storage op").
  Still open: the replay env-runner could skip storage ops on opaque
  origins with a visible warning — a residual decision, low priority
  since the record path now warns up front.
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
  `els.find(visible) || els[0]`. Record-side matched in #356 (gap 13
  closed). The same page puts a visible *category dropdown-toggle* before
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

## Dogfood pass V — bonigarcia.dev: a second tool class of pages

The selenium-webdriver-java practice site adds shapes QA Playground and
the storefronts don't have. All five cases passed record→replay on the
first run once the right helpers existed:

- **Text waits pierce open shadow roots** — the only way to claim
  shadow-DOM content from the record path; new `waitText` lib helper
  records `wait condition {kind:"text"}` which lands as an element
  visibility claim on a text locator. css `wait`/`present` still can't
  reach inside the root — document that pairing.
- **Two drag classes exist and need different live drives** — HTML5
  draggables respond to `DragEvent` dispatch; jQuery UI `draggable()`
  (bonigarcia, and demoqa's widgets) listens to real mouse sequences.
  New `dragSelectorMouse` dispatches mousedown→move→up live while
  recording the same `do/drag` step — replay's trusted-mouse path
  covers both. A lib-level `dragSelector` still synthesizes DragEvents
  for HTML5 pages.
- **do/state storage seeding is recordable** — `seedStorage` drives the
  live browser with `localStorage.setItem` and records the equivalent
  `do/state` params; the web-storage page rendering the seed back is
  the claim.
- **Page-visible dialog outcomes assert cleanly** — bonigarcia writes
  `You chose: false` / `You typed: X` into the DOM, which chains
  alert/confirm/prompt claims to page state on real pages for the
  first time.

## Dogfood pass VII — testautomationpractice.blogspot (TAP): blogspot widget soup

One page packs form fields, radios, checkboxes, two jQuery UI widgets
(datepicker, price slider), native alert/confirm/prompt, a dblclick
copy button, file upload, a live Wikipedia-search widget, and a
two-window `window.open` popup. Four goldens (#358) + one drag case
(#359, stacked on #281) all pass record→replay:

- **Popup `window.open` maps to `tab t2` deterministically** — a fresh
  session numbers tabs in open order, so recorded `tab t2` + `tab close`
  + `tab t1` round-trips cleanly through replay. First real coverage of
  the `tab` verb on a popup (not `tab new`).
- **jQuery UI datepicker closes by `display:none`, not removal** — the
  calendar node stays in the DOM; absence claims misfire on this widget
  class. The value-regex claim (`mm/dd/yyyy`) is the honest check.
- **`agent-browser drag` unifies live drive with replay** — #281's
  trusted-mouse `do/drag` made the synth-`DragEvent` live drive a
  parity lie (record showed nothing move, replay moved it). TAP's
  jQuery droppable works end-to-end only when the live drive uses the
  daemon's trusted `drag` command too (#359 flips `dragSelector`).
  `dragSelectorMouse` (bonigarcia) becomes redundant — a future cleanup.
- **nopCommerce is Cloudflare-gated** — `demo.nopcommerce.com` serves
  "Just a moment" to headless Chrome forever; swapped sites. Rule of
  thumb for sweep picks: probe with a real `agent-browser open`, not
  curl. **Verified deeper (headed+profile pass):** the wall isn't
  headless detection — Cloudflare Turnstile fingerprints the
  Chrome-for-Testing build itself, so `--headed --browser-profile`
  hits the interactive managed challenge and even *trusted* mouse
  clicks on the checkbox get bounced to a second round forever. The
  one path that bypasses it is the **extension** — it rides the
  user's own real Chrome (no CDP fingerprint), records steps+HAR on
  their side, and our side replays hermetically with `--mock-from`.
  A real-Chrome channel override is upstream `agent-browser` work.

## Dogfood pass VI — JPetStore (petstore.octoperf.com)

Struts app; every URL carries a `;jsessionid=<id>` matrix param and the
signin POST 302-redirects into the account page. Two real findings:

- **Volatile matrix params broke every auto-network claim** — flush's
  `insert_auto_network_claims` and crawl's `network_claim_steps`
  normalized URLs by splitting on `?`/`#` only, so `;jsessionid=` stayed
  inside `urlMatches` and could never match a fresh session. Recorded
  claims were unmatchable by construction; both emission sites now split
  on `;` too (#352). Same class as the parabank volatile-URL fix —
  worth grepping for other URL-normalization sites if it recurs.
- **agent-browser's input channel dies after a POST→302** — after the
  signin submit, `click`/`@ref`/`press` dispatch nothing (no events
  reach any page target) until an `open` to a *different origin*
  re-binds it; same-origin opens and `reload` do not. eval `.click()`
  still works. Record-side live drive worked around it with
  `clickSelectorForce`; replay's own trusted-input path is unaffected.
  Upstream quirk — flag if it shows up elsewhere.

## Recently closed (for orientation)

- IndexedDB claim + `do/state` seeding (#353) — P5 #14 closed
- JPetStore sweep incl. `;jsessionid` matcher normalization (#352)
- bonigarcia sweep incl. recordable storage seeds + mouse-driven drag helper (#351)
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

## Dogfood pass VIII — demoblaze.com: storefront SPA with modal auth

- **Async dialogs are the norm, not the exception.** TAP's alerts fired
  synchronously inside the click handler, so a single-shot
  `dialog status` read after the click worked. demoblaze's add-to-cart
  and signup alerts fire from XHR `.then` handlers ~1–1.5s after the
  click returns — the lib's `assertDialogText` now polls (8s window)
  instead of racing. Any site where a dialog is triggered by a network
  response needs this shape. Replay was never affected: the recorded
  `{dialog: true}` check claim already polls.
- **`fillSelectorReplayValue`** landed in `edge-pages-lib` (mirroring
  the automation-exercise lib): type a live-minted username, record
  `{{vars._unique}}` — each replay signs up a fresh account and the
  auto-network claims verify `POST /signup` + `POST /login` actually
  hit the API. Template fills are the record-side answer to
  unique-value flows.
- demoblaze's category rail reuses `id="itemc"` on all three links
  (Phones/Laptops/Monitors) — the click selector has to key off the
  `onclick="byCat('...')"` argument, not the id. Duplicate-id sites
  defeat `#id` locators; prefer attribute hooks.
- demoblaze keeps `.modal` in the DOM with `display:none` after close —
  same "close-by-style" trap as jQuery UI; the nav link stays covered
  until the fade finishes, so the sweep waits ~800ms (bootstrap's
  transition) before the next click.

## Dogfood pass IX — thinking-tester-contact-list: React SPA + mongo CRUD

Three cases shipped (`cl-tc01..03`, 103 checks total) covering signup →
add/edit/delete contact → logout→re-login. First suite exercising a real
CRUD REST API with per-entity ids and a native `confirm()` delete.

Surfaced **a real flush bug, fixed in the sweep PR**: auto-network
claims recorded REST entity ids literally (`GET /contacts/<mongoId>`) —
a replay mints a different contact via signup, so the claim could never
match. `insert_auto_network_claims` now rewrites volatile segments
(≥16 hex/dash, ≥6 digits) into regex classes and dedupes on the pattern.
Verified live: `GET/PUT/DELETE /contacts/<freshId>` all PASS on replay.

New race class, **post-commit-effect nav binding**: the app's router
binds nav-link/button handlers in a `useEffect` that runs after the view
commits — a synthetic click ~0ms after a render check (`#error` text,
details `#edit-contact`, navbar `#logout`) is a *silent no-op* (element
exists, click dispatches, nothing happens). Rows rendering is NOT proof
handlers are bound. Mitigation in the sweep: short recorded `wait`
settles (~700ms) before each nav click. Replay side now self-warns:
`click_locator` arms a MutationObserver + url/resource probe and
eprintlns "click produced no observable effect" when nothing changed
(landed in #362); **record-side detection shipped in #364** — `start`
arms a per-document probe (capture-phase click listener + MutationObserver
snapshotting mutations/url/resources/activeElement), `record-step` diffs
the latest click's snapshot on click-family verbs and eprintln-advises on
an all-zero delta, re-arming after every do-step to survive navigation.
Focus moves count as an effect; a pending dialog suppresses the check.

Site vetoes catalogued while scouting: `uitestingplayground.com` has a
broken cert (ERR_CERT_COMMON_NAME_INVALID, apex + www; http blocked),
`computer-database.gatling.io` is DNS-dead, `demo.nopcommerce.com` is
Cloudflare-gated, `automationintesting.com` is now a landing page.

## Dogfood pass X — letcode.in: Angular workspace pages (#363)

Four cases (`let-tc01..04`): input fills + element-state claims, the
alert/confirm/prompt triad (prompt value echoes back into the DOM — a
full request/response round-trip through the dialog pipeline), the
`frame` verb driving fills inside a same-origin `/frameui` iframe, and
radio-group checked-state assertions.

Surfaced **a telemetry-list gap**: Google's consent CMP
(`fundingchoicesmessages.google.com`) and ad-quality beacon
(`ep1.adtrafficquality.google`) are NOT analytics hosts — they fire
nonce'd POSTs that became unmatchable auto-network claims (5s timeout,
run fails). Added them + `googlesyndication.com`/`consent.google.com`
to `telemetry.rs` on the sweep branch (stacked on #315). Lesson: any
ad-consent CMP host will produce this failure mode — the host list
wants a maintenance note or a pattern heuristic later.

Deliberately-broken-by-design pages exist: letcode's `/radio` checkbox
section demos a bug where `.click()` on the input does not check it.
Sweep wrote around it; worth remembering when a "click had no effect"
looks like a tool bug — sometimes it IS the app (the #362 warning now
surfaces exactly this signal on replay).

`edge-pages-lib` gained `frameInto`/`frameMain`/`fillInFrame` +
`record-translate`'s `frame` method — iframe scenarios are now
recordable from the runner DSL, not just authored JSON.

## Dogfood pass XI — letcode.in sweep II: the dismissal problem (#365)

Four more letcode pages (`/dropdowns` multi-selects, `/button`
style+disabled claims, `/window` `window.open` + tab round-trip,
`/file` upload+download claims). The wall surfaced here is a general
flake class: Google **fundingchoices** mounts a reward-ad wall
(`.fc-monetization-dialog` + `.fc-dialog-overlay`) asynchronously —
seconds after load, stochastic — and it's NOT dismissible (only a
"View a short ad" button). It intercepts every click-family hit-test;
a recorded scenario that passed at record time can fail at replay when
the wall mounts between steps.

**Shipped as a verb, not a workaround**: `do/dismiss` takes a raw
css/testId/xpath locator, removes matching nodes at dispatch, and keeps
the selector on a per-run dismissal list re-applied before every later
interactive step — a late-mounting overlay can never intercept anything
downstream. Role/text locators bail (can't lower to a re-runnable
selector); the list persists across navigations (CMP banners re-mount
per page until accepted) and absent matches are a no-op. This doubles
as the GDPR/cookie-banner tool for any real site.

Recorded side is identical: `dismissBySelector` in `edge-pages-lib`
+ a `record-translate` arm. tc04's download claim — the one the wall
broke — now passes 10/10 behind the dismiss step.

## Dogfood pass XII — never-stuck hardening (#372–#390)

A sweep over the "agent silently does the wrong thing or stalls" class
— every fix makes a previously invisible failure mode loud:

- **Role-locator hover/fill hit-tests** (#372): covered/offscreen
  targets warn instead of dispatching to nothing (the same guarantee
  raw-locator clicks got in #324).
- **heal-chronic --apply** (#373): one command promotes every chronic
  self-heal row's latest patch — the heal-debt handoff is now an
  action, not a paste-ready issue.
- **`--browser-profile`** (#374): a persistent Chrome profile
  (login state, warm cookies) — but NOT a Cloudflare bypass:
  Turnstile fingerprints the Chrome-for-Testing build itself, so
  managed challenges stay blocked even headed (see the nopCommerce
  note). It still helps sites whose gating is cookie/session-based.
- **`emulate <key>:"off"`** (#375): every runner-side emulate key can
  be cleared mid-run; `credentials:"off"` bails honestly.
- **Subprocess cap** (#376): every `agent-browser` spawn has a default
  300s timeout (`AGENT_QA_AGENT_BROWSER_TIMEOUT_MS`, `0` disables) —
  a hung daemon call can't stall a run forever.
- **`start` collision guard** (#377): refuses while a recording is
  active; `--force` abandons it explicitly.
- **Worker-originated fetch capture** (#378): `Target.setAutoAttach` +
  per-target `Network.enable` — service-worker cache fills and
  background sync land in `network_requests` tagged `worker`.
- **Extension shadow locators** (#379): clicks inside open shadow roots
  record `{role, name}` instead of an unresolvable host-relative css.
- **flush packages file literals** (#380): absolute upload/fileChooser
  paths copy under `<scenario>/files/` with `by_src` dedup — a recorded
  upload replays on any machine.
- **`--auto-shots` skips nav/non-visual verbs** (#381): shot claims
  only attach where a screenshot is meaningful.
- **`emulate touch`** (#382): `Emulation.setTouchEmulationEnabled` for
  mobile-gated UX.
- **Extension keyboard coverage** (#383): nav-keys on widget roles,
  Escape anywhere, Enter on widgets+inputs record as `press` drafts.
- **`copy`/`extract` carry `files/` + `inputs.local.json`** (#384):
  cloned scenarios keep packaged uploads and local inputs.
- **`flush` refuses an empty buffer** (#385): a false start can't seal
  a schema-valid `steps:[]` scenario that replays trivially green.
- **Extension records mid-flow navigations** (#386): link nav, typed
  URLs, SPA pushState and back/forward emit `goto` drafts (deduped via
  `lastUrl`) — recordings no longer teleport between pages.
- **Upload-file lint** (#387): `upload-file-missing` +
  `upload-file-absolute` catch refs that would only break at replay.
- **Nameless-role ambiguity warning** (#388): `{"role":"button"}` with
  >1 candidates logs a warning instead of silently first-matching.
- **`--all` empty-selection message** (#389): filter/tags/shard-emptied
  selections name the flags, not the root.
- **`record status` liveness probe** (#390): reports a dead recording
  session (lost HAR capture) before flush discovers it.

## Dogfood pass XIII — cross-process races + capture honesty (#393–#404)

The second never-stuck wave, aimed at races between processes and
silent capture gaps:

- **Per-session run lock** (#393): `runner::run` holds a create_new
  lockfile for the run's lifetime — a second replay/crawl on the same
  session name bails instead of fighting for one browser; dead
  holders' locks are stolen via `kill(pid,0)`.
- **Paused-flush warning** (#394): `flush` while `record pause` is
  armed warns instead of silently sealing a partial scenario.
- **`scenario new --from-har`** (#395): mints an env.open nav skeleton
  from a captured HAR — the "user sent us a bug" on-ramp, named
  errors for malformed/empty HARs.
- **Origin-bound env ops warning** (#396): cookie/localStorage/flag/gql
  ops before any `nav` in env.open warn — they silently no-op'd on
  about:blank.
- **Atomic recording claim** (#397): `record start` claims the state
  file with create_new — two racing starts can no longer overwrite
  each other's sid+session binding.
- **CAS saves** (#398): `RecorderState::save` compare-and-swaps on
  seq/loaded_seq — concurrent `record-step` appends bail loudly
  ("changed on disk — retry") instead of last-writer-wins drops.
- **Mid-recording run guards** (#399): `replay`/`crawl` refuse on a
  session whose state file says it's mid-recording; `record continue`
  bypasses via env internally (and now clears it after the replay).
- **Unbound `{{vars.*}}` warning** (#400): an unresolved token warns
  once per name instead of passing the literal into the field.
- **Workbench paused-drop surfacing** (#401): a step dropped because
  the recording is paused throws through `record-skip` instead of
  looking captured.
- **Extension dead-capture visibility** (#403): navigating to a
  non-http page flips the badge to `!`, the popup says capture is
  paused, and the export bundle carries `warnings[]` that `ingest`
  echoes.
- **`buffer load` refuses ANY active recording** (#404): the
  empty-buffer loophole — a fresh recording's state file could be
  silently replaced, re-binding sid/session under a live tab.

## Dogfood pass XIV — the silent-green + vanishing-artifact sweep (#405–#418)

Third wave of the never-stuck audit — the classes where a command exits 0
having verified nothing, or an artifact disappears without a trace:

- **TTY-blocked stdin reads** (#406): `record-step -`/`record-setup -`
  bail with "expects piped JSONL" instead of hanging on Ctrl+D.
- **Destructive verbs vs in-flight runs** (#407/#410): `scenario delete`
  refuses the active recording's source; delete/rename refuse while a
  run holds the scenario's audit open.
- **Dead report-server UI stalls** (#411): the vite proxy answers JSON
  502s (SSE-safe — no proxy timeout) and ChatPage renders an honest
  empty state instead of an infinite spinner.
- **Unreadable state file** (#412): `record start` propagates the parse
  error naming the escape hatch; `start --force` discards it.
- **Silent-green plans** (#413): `plan run` bails "0 of N members ran —
  all skipped" instead of exiting 0 on an unrecorded suite.
- **Extension dblclick split** (#414): click drafts debounce 350ms —
  a dblclick collapses the two click drafts into one `do/dblclick`
  (previously replayed as two clicks, diverging on double-click-only
  handlers).
- **Resume-on-dead-session** (#415): `record resume` flips the paused
  flag (a newer recording may capture later) but warns that capture
  can't restart — the honest middle ground.
- **Replay-button no-show** (#416): `useRuns.replay` awaited the spawn
  but let the run-registration poll die silently — a replay dying
  before its first audit write now returns an error naming the cause.
- **Template cycles** (#417): `flatten_steps` had no depth cap — a
  schema-valid self-referencing `useTemplate` stack-overflowed the
  replay with no error at all. Expansion bails past depth 64 with a
  cycle-hinting message.
- **Corrupt scenario.json vanishes from the sidebar** (#418): the
  listing collapsed missing and unparseable into the same null and
  filtered the row — a corrupted recording showed nothing. The server
  now distinguishes: missing → hidden; unparseable → `scenarioError`
  on the row + a red `unreadable` badge; plan-run skip reasons name
  "missing or unreadable".

### Verified-not-broken in this sweep (no change needed)

`--shard` bounds reject `0/2`/`5/2`; `check-all`/`validate-all`/`lint-all`
tolerate an empty root deliberately (`init --ci` CI runs before scenarios
exist); vault-ref resolution names unresolvable keys; the npm launcher's
missing-binary paths name the platform package to install; `record-step`
binds the recording's own session (no redirect flag to mis-set);
`scenario rename` guards same-sid/unsafe/missing-source/in-flight/
existing-dest/bound-recording in order; `scenario new --from-har` bails
on empty navs; audit.json corruption degrades to null fields in
listings, not crashes; `record status`/`continue`/`record-step` paused
paths all report honestly; `crawl` propagates open failures and prunes
dead links with a warn.

## Dogfood pass XV — dead-end honesty + durability sweep (#420–#426)

Fourth wave — the classes where a tool call hangs forever, a dead
server looks like a dead button, a corrupt file pretends to be absent,
or a fresh write stomps someone else's artifact:

- **Stream-read errors hang the client** (#421): `createReadStream`
  errors don't propagate through `pipe()` — a file pruned between
  stat() and open() (or a permission flip) left the HTTP socket open
  forever after `Content-Length` headers went out. All five stream
  sites share `streamFile`, which destroys the response on read error
  so the fetch fails fast; the catch-all handler now guards
  `headersSent` before attempting a 500.
- **Chat calls die silently on a dead server** (#422): `postPrompt`
  dispatched `add_user` then threw unhandled — the message appeared
  sent but never left the browser. Write calls now return a 503-shaped
  `{ok:false}`; `abort` clears the streaming flag in `finally` so the
  spinner can't stick.
- **Editor calls die silently too** (#425): same class on the other
  api module — `getJson`/`postJson` now degrade rejections to the same
  503 shape, so `flush`/`cancel`/move/edit flash errors instead of
  looking dead; reads degrade to their empty shapes.
- **Corrupt plan/set/case vanishes** (#423): `load_plan` collapsed
  missing+unparseable into "no such plan"; it now names the corrupt
  path. `load_json_list` warn-and-skips unreadable entries, and
  `resolve_plan_case_ids` warns per dangling scope ref instead of
  silently dropping them.
- **Flush clobbers an unrelated scenario** (#424): a fresh
  `record start <sid>` + flush overwrote an existing scenario for the
  same sid (the buffer's `source_ref` wasn't checked). Flush now
  refuses unless the recording actually continues that scenario.
- **Zero-step replay + validate ordering** (#420): `runner::run` did
  browser setup before loading the scenario — a zero-step or invalid
  scenario burned a browser session and a lock before erroring. Load
  + validate now run first; zero steps bails with "nothing to replay",
  and the `empty-steps` lint escalated to an error (a 0/0 PASS
  verifies nothing).
- **Non-atomic scenario writes** (#426): the eight `scenario.json`
  writes (new/from-har/insert/redact/copy/extract/rename/tag) used
  bare `fs::write` — a mid-write kill left a truncated file that
  corrupt-loads forever. All now go through `atomic_write_file`
  (.tmp + rename), matching `flush`/`heal-promote`.

### Verified-not-broken in this sweep (no change needed)

Stale session locks steal cleanly when the holder's pid is dead;
`--from`/`--until` name the missing step id plus every valid id;
`--mock-from` names the missing HAR and how to mint one;
`agent-qa web` explains EADDRINUSE with the port flag; corrupt
`audit.json`/record-state files bail with the path in the message;
buffer delete/move/edit/insert bounds-check before mutating; the
page-level api modules throw on `!res.ok` and every page catches;
`scenario extract` refuses to overwrite an existing destination;
`record status` bounds its liveness probe at 3s; all `events.jsonl`
readers tolerate missing/truncated files; `run_plan`/`member_rows`
bail on zero resolved members.

## Dogfood pass XVI — capture-surface honesty (#428)

Fifth wave — the silent-capture class in the browser extension plus a
re-audit of the surfaces the last wave didn't touch:

- **CSP-blocked injection exports `network: []` silently** (#428): the
  extension's network patch (`inject.js`) rides a
  `<script src=chrome-extension://>` element a strict page CSP can
  block — steps kept recording while the bundle exported an empty
  network array, so `--mock-from`/`--offline` replay failed for a
  reason nobody could see. `inject.js` now posts a `__aqNetReady`
  beacon; `content.js` forwards `netReady` on `record:start`, on
  mid-recording document re-arms, and live when the beacon arrives
  while active; any delivered entry also marks capture armed. On stop,
  a bundle with steps but no beacon gains a `warnings[]` entry naming
  the gap (echoed by `ingest`). Sticky-conservative: only warns when
  capture *never* armed anywhere.

### Verified-not-broken in this sweep (no change needed)

- **Role-locator select/check/uncheck do not need hit-tests** — the
  residual item from pass XIII closes as a non-issue: `select` drives
  `select_via_combobox` (opener activation → popup-count probe →
  ArrowDown keyboard fallback → named option errors), and check/uncheck
  dispatch through the click path that already has the ClickProbe
  post-check + named-control/text/snapshot fallbacks. No coordinate
  click exists on these paths to silently miss.
- **live-bridge reconnect is bounded** — retries only while a
  subscriber is present, broadcasts `bridge-error` to subscribers,
  flushes a held click before dropping the socket, and unsubref'd
  timers can't keep the worker alive.
- **Launcher binary honesty** — `agent-qa web` without the platform
  package prints `editor: (unavailable)` at startup AND every
  runCli-backed API call returns 503 `agent-qa CLI not resolved`
  (connect, compare, recording verbs).
- **record-step on a dead session is honest** — drafts are authored
  steps, not browser state, so appending without a live browser is
  correct; dead-session sidecar failures warn per-step.
- **truncate ref safety** — `{{steps.sN.*}}` refs only point backward
  at executed steps, so dropping the tail can't dangle a survivor.
- **Extension on non-http pages** — `popup:start` refuses `file:`/
  `chrome:`/store URLs with "can't record on this page".
- **`record-step` validation depth** — raw-draft schema pass catches
  misplaced keys before serde drops them; stdin JSONL names the bad
  line; TTY `-` bails instead of blocking on read.

## Dogfood pass XVII — extension capture-surface + silent-trap sweep (#432–#437)

Audited the whole extension (content.js, inject.js, background.js,
popup.js, manifest) plus the remaining workbench/CLI spawn and
param-validation surface for never-stuck / silent-loss classes.

- **A failed export destroyed the bundle** (#435): `chrome.downloads.
  download` is callback-async — the popup's Stop handler resolved before
  the download's fate was known, and a failed download still deleted the
  persisted session. The handler now awaits a `download()` that resolves
  to an error string via `chrome.runtime.lastError`; on failure it keeps
  the session, returns `recording: true` so the armed stop-button
  re-enters `popup:stop` and retries the same bundle, and the popup
  paints the error instead of silently restarting.
- **File uploads were silently skipped** (#436): `input[type=file]`
  changes hit a bare `return`. They now emit a real `upload` draft
  carrying `files/<name>` literals (contents can't cross worlds, names
  can) — the flow replays end to end and the bundle warning names every
  file to drop under the scenario dir.
- **Multi-selects recorded one option** (#436): `el.value` is only the
  first selected option and "" for valueless options; the draft now
  carries every selected value (or text) joined on ',' — exactly the
  shape the replay `select` verb splits and matches.
- **Popup/new-tab flows were silently lost** (#436): `target=_blank` /
  `window.open` spawns a tab capture can't follow. `webNavigation.
  onCreatedNavigationTarget` counts them, flags the badge, and the
  export bundle names the gap.
- **Form posts were invisible to body matching** (#437): `URLSearchParams`
  and `FormData` request bodies (the standard login/CRUD shape) recorded
  `postData: undefined`, and `responseType:"json"` XHRs had no response
  body. Both serialize into the entry now — `postDataContains` and mock
  matching can see them.
- **`mock`/`unmock` params silently wrong** (#434): `params.status`
  outside 100–999 truncated into a nonsense status (`u16`), a non-string
  `unmock url` cleared ALL rules, and an unmatched unmock glob did
  nothing. All three now bail or warn.
- **Malformed `agent-qa.toml` was silently dropped** (#433): a parse
  error meant the whole config file was ignored; it now warns.
- **Hung CLI calls killed workbench requests silently** (#432): every
  `execFile`/`exec` path in report-server has a timeout now (180s CLI
  runner, 30s browser-session closer), and a timeout-kill resolves
  `code: 124` instead of the confusing `code: 0`-equivalent.

### Verified-not-broken in this sweep (no change needed)

- **`record`/`buffer` arg surface** — unknown flags bail across
  replay/scenario/buffer/heal-chronic; stray positionals name the
  expected usage.
- **`heal-chronic`/`audit` tails** — bounded iteration, corrupt lines
  skipped by design, `--issue`/`--apply`/`--json` combos guarded.
- **`buffer discard`** — clears the state file and reports; the
  browser session persists intentionally for reuse.
- **Web frontend fetch surface** — all calls funnel through api libs
  with `.catch(() => offline(...))` + `res.ok` checks; the three direct
  fetches (version badge, blob convert, session poller) each handle
  failure appropriately for their role.
- **cli/src/crawl.rs** — `wait_for_load_capped(5s)` + bounded BFS.
- **`inject.js` duplicate guard** (`__aqNetInstalled`), tail-loss on
  post-stop requests is benign, `clip()` bounds body size.
- **`parse_draft` multi-shape** — schema `"literal": {}` is
  unconstrained, so array `files/<n>` literals validate end to end.
