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
| Golden suites | ~30 QA Playground pages, ~34 the-internet edge cases (six sweeps), saucedemo suite (login/sort, full 21-step purchase, negative auth, logout, cookie+storage lifecycle), expandtesting (login round-trip, dynamic table, infinite scroll), todomvc (stateful SPA), demoqa widgets, httpbin hermetic-mock loop, workbench selftest goldens, quotes.toscrape.com (pagination, HttpOnly cookie claims, scroll offsets), parabank (registration with `{{vars._unique}}`, login/logout, profile update — volatile-URL claims normalized), demoblaze (category filters, add-to-cart alert claims, cart session persistence across reload, full purchase flow) |

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
6. **Live-input divergence** (carried from #253): agent-browser's real
   input click doesn't trigger React state on saucedemo's add-to-cart;
   recorded `click` replays fine via native DOM click. If live clicks
   keep missing delegated handlers, the record path may silently drop
   user actions worth capturing — watch for it.
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

10. **Navigation-redirect statuses are uncapturable** — a POST → 302 →
    GET chain records the POST's status as null because agent-browser's
    netlog never sees the redirect hop's response. The `#277` pooled
    CDP client is the substrate for owning `Network.*` capture directly.
11. **`wait url` can't match in-flight requests** — it polls resource
    timing, which only lists *completed* entries; a request still
    pending at claim time waits out the full timeout (fine) but a slow
    endpoint (>15s cold, e.g. demoblaze `/signup`) needs an explicit
    `params.timeoutMs`.

## Lessons from the fresh-site sweeps (quotes/parabank/demoblaze, #309/#310)

Real sites taught three durable patterns:

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
- Auto-claims on flush: `--auto-network`/`--auto-errors` (#252)
- Cookie/storage goldens + `do/state` seeding (#253)
- `pageError` + `console` + `a11y` claim subjects (#248/#167/#240)
- Network claims fired/status/json (#143), postDataContains (#205),
  `wait url` (#166)
- Visual loop: shot claims + clip + mask + diff maps + re-mint + `/qa
  accept` (#129–#186)
- Suite tooling: parallel `--jobs`, `--retry`, `--until-fail`, `--watch`,
  JUnit, HTML report
