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
| Golden suites | ~30 QA Playground pages, ~34 the-internet edge cases (six sweeps), saucedemo suite (login/sort, full 21-step purchase, negative auth, logout, cookie+storage lifecycle), expandtesting (login round-trip, dynamic table, infinite scroll), todomvc (stateful SPA), demoqa widgets, httpbin hermetic-mock loop, workbench selftest goldens |

## Ranked gaps

### P1 — the golden loop is asymmetric for domshot

1. **`/qa accept` only re-mints `shot` baselines** — the workflow runs
   `agent-qa shot-accept`; a domshot diff on a UI PR can't be accepted by
   comment, it needs a local `domshot-accept`. `accept.ts` should call
   both (or a unified `goldens-accept`).
2. **Coverage metric counts shots, not domshots** — the `shot%` rollup
   ignores structural baselines, so a scenario covered only by domshots
   reports 0% visual coverage.
3. **WS/SSE capture (#278) has no golden** — none of the practice sites
   open a socket; needs a small fixture page (echo server + page) so the
   `network` claim's ws/sse frames are exercised end to end.

### P2 — record path still trails replay

4. **Touch/gesture capture** — `hold`/`swipe`/`pinch`/`rotate`/`rightclick`
   replay fine, but the recorder never emits them (rightclick capture
   landed in #268; the rest produce nothing). Real-device recording would
   need CDP touch-event bridging.
5. **Hermetic capture** — `--mock-from`/`--offline` are replay-only; there
   is no `record` path that stubs the backend while recording, so the
   httpbin-style hermetic scenario has to be authored by hand.
6. **Live-input divergence** (carried from #253): agent-browser's real
   input click doesn't trigger React state on saucedemo's add-to-cart;
   recorded `click` replays fine via native DOM click. If live clicks
   keep missing delegated handlers, the record path may silently drop
   user actions worth capturing — watch for it.

### P3 — polish

7. **`domshot` in the workbench selftest goldens** — the workbench's own
   UI is shot-covered only; ARIA goldens would catch structural drift
   that pixel diffs can't (e.g. tree reorder with identical pixels).
8. **heal-chronic → issue handoff** — the self-heal debt board is
   terminal-only; nothing auto-files or comments on chronic healers.

## Recently closed (for orientation)

- Touch verbs: `hold`+`swipe` (#259), `pinch` (#262), `rotate` (#267);
  `rightclick` (#266) + live-pane capture (#268)
- Geo/tz/locale emulation via own pooled CDP client — grant +
  `Emulation.setGeolocationOverride` on the active target (#277), +
  `setTimezoneOverride`/`setLocaleOverride` (#279)
- WS/SSE frames in `network` claims (#278)
- `wait locator` — poll DOM state before continuing (#282)
- `domshot` claim + `domshot-accept` (#283) + workbench diff
  card/re-mint/insert-check/accept-all (#284)
- Trusted-mouse `drag` + demoqa widgets goldens (#281)
- Crawl telemetry pruning + `crawl` golden (#272); `fixed-sleep` +
  `brittle-locator` lint (#273/#271)
- todomvc + expandtesting + httpbin sweeps (#269/#275/#274)
- `run-report.html` artifacts in qa-gate (#260); qaplayground skill
  edge-sweep docs (#261); `timing` golden (#276)
- Dialog tolerance on dialog-opening clicks (#246)
- Auto-claims on flush: `--auto-network`/`--auto-errors` (#252)
- Cookie/storage goldens + `do/state` seeding (#253)
- `pageError` + `console` + `a11y` claim subjects (#248/#167/#240)
- Network claims fired/status/json (#143), postDataContains (#205),
  `wait url` (#166)
- Visual loop: shot claims + clip + mask + diff maps + re-mint + `/qa
  accept` (#129–#186)
- Suite tooling: parallel `--jobs`, `--retry`, `--until-fail`, `--watch`,
  JUnit, HTML report
