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
| Golden suites | ~30 QA Playground pages, ~34 the-internet edge cases (six sweeps), saucedemo suite (login/sort, full 21-step purchase, negative auth, logout, cookie+storage lifecycle), expandtesting (login round-trip, dynamic table, infinite scroll), todomvc (stateful SPA), demoqa widgets, httpbin hermetic-mock loop, workbench selftest goldens, quotes.toscrape.com (pagination, HttpOnly cookie claims, scroll offsets), parabank (registration with `{{vars._unique}}`, login/logout, profile update — volatile-URL claims normalized), demoblaze (category filters, add-to-cart alert claims, cart session persistence across reload, full purchase flow), automation-exercise (signup+cart lifecycle), wikipedia (search nav, TOC, history, REST API claims), formy (full form, bootstrap modal, jQuery datepicker, JS dropdown), testpages (ajax cascade, form POST echo, native dialogs, onblur validation), coffee-cart (cart badge, promo modal, checkout form, quantity steppers), globalsqa XYZ Bank (AngularJS login, deposit/withdraw, transactions ledger, manager console) |

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
   expects). Still open: `download` (browser download events may never
   reach the page session while the daemon owns download policy),
   `frame` (pick is top-frame only), `viewport` (window resize), and a
   `state`-seed affordance (replay seeds storage/cookies; recording has
   no "capture current state" action). Structural verbs
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

## Recently closed (for orientation)

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
