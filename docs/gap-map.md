# Gap map — what agent-qa covers today, and what's still open

A snapshot audit of the whole surface (verbs, claims, evals, CI, workbench)
with the gaps ranked by how much they'd hurt a real user. Updated each time
the picture shifts materially.

## Surface inventory

| Layer | What exists |
| --- | --- |
| Do verbs (31) | navigation (`goto`/`reload`/`back`/`forward`), input (`click`/`dblclick`/`type`/`clear`/`press`/`hover`/`select`/`check`/`uncheck`/`upload`/`focus`/`blur`/`drag`/`scrollTo`), dialogs (`dialog`), files (`download`/`fileChooser`), system (`wait`/`read`/`state`/`mock`/`unmock`/`callGql`/`tab`/`viewport`), structure (`loop`/`group`/`useTemplate`) |
| Claim subjects (15) | `element` (text/value/count/attribute), `url`, `network` (fired/status/responseJsonPath), `data`, `flag`, `dialog`, `file`, `shot` (visual diff, clip, mask), `storage`, `cookie`, `console`, `pageError`, `a11y` (axe-core) |
| Record vocabulary | ~50 methods covering every verb + claim kind; parity-gate tested |
| Suite flags | `--all`/`--shard`/`--filter`/`--tags`/`--jobs`, `--retry`/`--until-fail`/`--watch`/`--keep-going`, `--har`/`--offline`/`--mock-from`/`--freeze`/`--base-url`, `--auto-promote`/`--update-baselines`, `--junit`/`--report`/`--record-video`, `--from`/`--until` |
| Audit | `flaky`, `slow`, `heal-chronic` (+`--all`), `verdict` (+`--all`), `cluster`, `trend` (+`--all`), `health`, run-vs-run compare (CLI + workbench) |
| CI | `qa-gate` (fixture goldens + sticky verdict), `ui-goldens` (visual gate w/ embedded diffs), `qa-crawl` (draft coverage on UI PRs), `qa-adopt` + `/qa accept` commands, composite `action.yml`, `evals-nightly`, changelog-driven releases |
| Golden suites | ~30 QA Playground pages, ~34 the-internet edge cases (six sweeps), saucedemo suite (login/sort, full 21-step purchase, negative auth, logout), workbench selftest goldens |

## Ranked gaps

### P0 — the loop isn't closed for recorded users

1. ~~Record → network claims~~ **done in #252 + default-on in #256** —
   `flush --auto-network` appends deduped `networkFired` claims for
   XHR/fetch/non-GET traffic; on by default, `--no-auto-network` opts out.
2. ~~`pageError`/`console` claims in record~~ **done in #252 +
   default-on in #256** — `flush --auto-errors` appends
   `pageError notExists`; on by default, `--no-auto-errors` opts out.

### P1 — shipped verbs with no golden coverage

3. ~~`emulate` (#242) and `frame` (#245)~~ — goldens land in #257
   (frame: TinyMCE iframe read/claims/back-out) and #258 (emulate:
   device-UA + custom-header echo via httpbingo). **Found while probing:**
   `set geo` applies the CDP override but nothing grants the geolocation
   permission — `navigator.geolocation` hangs in headless, so no
   `/geolocation` golden until a grant path lands upstream or via a CDP
   passthrough verb.
4. ~~`storage`/`cookie` claims (#239)~~ **done in #253** —
   `cookiePresent`/`storagePresent` vocab + `assertCookie`/`assertStorage`
   helpers + sauce-tc05 (cookie lifecycle) / sauce-tc06 (`do/state`
   cookie seed lands logged-in).
5. ~~`loop`/`group`/`useTemplate`~~ **done in #255** —
   `runAuthoredGolden` writes authored scenario/2 docs straight into the
   golden scenarios root; struct-tc01 expands a login `useTemplate` plus
   a `group`→`loop` over the four saucedemo sort orders (17/17).

### P1 — claim subjects still missing

6. ~~`computedStyle` claim~~ **done in #254** — no new subject needed:
   `element` + `attribute: "style:<prop>"` reads `getComputedStyle`
   (edge-tc35 proves `display:none`→`block` on `/dynamic_loading`).
7. ~~`focus` claim~~ — already covered: `element` + `elementFocused`
   predicate (`{element: <locator>, attribute: "focused"}`); no new
   subject needed. (Listed in error.)
8. ~~`timing` claim~~ **done in #255** — `{"timing": "<stepId>"}` reads
   the run's own `events.jsonl` (latest terminal row's `ms`), numeric
   predicates compare; `stepTiming` vocab kind; struct-tc01 pins the
   login click <15s and each sort select <5s.

### P2 — platform coverage

9. **Mobile/touch**: `do/viewport` + `emulate` give layout, but no swipe/
   tap-hold/pinch verbs; edge pages have no touch cases yet.
10. **Geolocation/timezone**: #242 ships `geo`/`device` (device presets
    bundle timezone+locale), but geolocation is *blocked on a permission
    grant* — see P1 #3's finding. Timezone has no `set` subcommand in
    agent-browser at all (documented gap in #242).
11. **WebSocket/SSE**: the network layer is request/response only —
    `ws://` frames aren't captured; a `network` ofKind would need daemon
    support first.

### P3 — ecosystem polish

12. **`run-report` in CI** — the HTML report exists (#224) but no workflow
    uploads it as an artifact yet.
13. **Skill docs for edge sweeps** — the qaplayground skill covers the
    Playground flow but not the edge-pages-lib pattern a new-site sweep
    follows.

## Recently closed (for orientation)

- Dialog tolerance on dialog-opening clicks (#246)
- Auto-claims on flush: `--auto-network`/`--auto-errors` (#252)
- Cookie/storage goldens + `do/state` seeding (#253)
- Live-drive findings from the sauce sweep (#253): agent-browser's real
  input click doesn't trigger React state on saucedemo's add-to-cart
  (recorded `click` replays fine via native DOM click — live/recording
  divergence, not a replay bug); covered-element refusals on still-
  animating drawer links need a settle wait or `el.click()`. If live
  clicks keep missing delegated handlers, the record path may silently
  drop user actions worth capturing — watch for it.
- `pageError` + `console` + `a11y` claim subjects (#248/#167/#240)
- Network claims fired/status/json (#143), postDataContains (#205),
  `wait url` (#166)
- Visual loop: shot claims + clip + mask + diff maps + re-mint + `/qa
  accept` (#129–#186)
- Suite tooling: parallel `--jobs`, `--retry`, `--until-fail`, `--watch`,
  JUnit, HTML report
