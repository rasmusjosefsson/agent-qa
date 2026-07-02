'use strict';
// Smoke test for the read-only report viewer (lib/report-server.js).
// Builds a fixture scenario tree in a temp dir, boots the server against
// it, and asserts the JSON endpoints + path-safety rejection. No external
// deps — uses node:test + the global fetch.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const srv = require('../lib/report-server.js');

// Isolate package discovery from the developer's real ~/.agent-qa (which may
// have personas/environments registered via `agent-qa install`). Point
// AGENT_QA_HOME at an empty dir so persona/environment lists are deterministic;
// the discovery test overrides this with its own fixture and restores it.
process.env.AGENT_QA_HOME = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-home-empty-'));

// ---- fixture ----

function makeFixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-report-test-'));
  const sid = '2026-06-24T15-07-23-027Z__deadbeef';
  const runId = '2026-06-24T15-07-23-027Z__68f6bec5';
  const sdir = path.join(root, sid);
  const runDir = path.join(sdir, 'replays', runId);
  fs.mkdirSync(path.join(runDir, 'screenshots'), { recursive: true });
  fs.mkdirSync(path.join(runDir, 'snapshots'), { recursive: true });

  fs.writeFileSync(
    path.join(sdir, 'scenario.json'),
    JSON.stringify({
      schema: 'scenario/2',
      id: 'demo-scenario',
      intent: 'open example.com and click a missing button',
      steps: [{ id: 'navHome' }, { id: 'headingVisible' }, { id: 'clickMissingLogin' }],
    }),
  );

  fs.writeFileSync(path.join(sdir, 'replays', 'latest.txt'), runId + '\n');

  fs.writeFileSync(
    path.join(runDir, 'audit.json'),
    JSON.stringify({
      schema: 'scenario-replay-audit/v1',
      runId,
      scenarioId: 'demo-scenario',
      startedAt: '2026-06-24T15:07:23.027Z',
      finishedAt: '2026-06-24T15:07:27.143Z',
      exitCode: 1,
      summary: 'SUMMARY: 2/3 (FAIL)',
    }),
  );

  fs.writeFileSync(
    path.join(runDir, 'status.json'),
    JSON.stringify({ state: 'done', currentIdx: 3, total: 3, ok: false }),
  );

  const events = [
    { idx: 1, total: 3, id: 'navHome', intent: 'navigate', kind: 'do:goto', status: 'running' },
    {
      idx: 1,
      total: 3,
      id: 'navHome',
      intent: 'navigate',
      kind: 'do:goto',
      status: 'pass',
      ms: 1039,
      screenshot: 'screenshots/navHome.png',
      snapshot: 'snapshots/navHome.txt',
    },
    { idx: 3, total: 3, id: 'clickMissingLogin', intent: 'click Login', kind: 'do:click', status: 'running' },
    {
      idx: 3,
      total: 3,
      id: 'clickMissingLogin',
      intent: 'click Login',
      kind: 'do:click',
      status: 'fail',
      ms: 1052,
      error: 'Element not found',
      screenshot: 'screenshots/clickMissingLogin.png',
      snapshot: 'snapshots/clickMissingLogin.txt',
    },
  ];
  fs.writeFileSync(path.join(runDir, 'events.jsonl'), events.map((e) => JSON.stringify(e)).join('\n') + '\n');

  // A real PNG byte payload (1x1) and a snapshot text file.
  fs.writeFileSync(path.join(runDir, 'screenshots', 'navHome.png'), Buffer.from('PNGDATA-navHome'));
  fs.writeFileSync(path.join(runDir, 'snapshots', 'navHome.txt'), 'heading "Example Domain"\n');

  // A secret OUTSIDE the run dir that a path-escape attempt would target.
  fs.writeFileSync(path.join(root, 'secret.txt'), 'TOP SECRET');

  return { root, sid, runId };
}

function boot(root, deps) {
  const server = srv.createServer(root, deps);
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({ server, base: `http://127.0.0.1:${port}` });
    });
  });
}

// ---- unit-level helpers ----

test('isSafeSegment mirrors the Rust rule', () => {
  for (const ok of ['navHome', 'a.b-c_d', '2026-06-24T15-07-23-027Z__68f6bec5']) {
    assert.equal(srv.isSafeSegment(ok), true, ok);
  }
  for (const bad of ['', '.', '..', '../x', 'a/b', 'a b', 'a%2e', 'foo/../bar']) {
    assert.equal(srv.isSafeSegment(bad), false, bad);
  }
});

test('resolveScenariosRoot honors --root, env, then default', () => {
  assert.equal(srv.resolveScenariosRoot({ root: '/abs/here' }), '/abs/here');
  assert.equal(
    srv.resolveScenariosRoot({ env: { AGENT_QA_SCENARIOS_DIR: '/from/env' }, cwd: '/tmp' }),
    '/from/env',
  );
  assert.equal(
    srv.resolveScenariosRoot({ env: {}, cwd: '/work' }),
    path.join('/work', 'tmp', 'agent-qa-scenarios'),
  );
});

// ---- endpoint integration ----

test('report viewer endpoints', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  await t.test('GET /api/scenarios lists the scenario + latest verdict', async () => {
    const res = await fetch(`${base}/api/scenarios`);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.scenariosRoot, fx.root);
    assert.equal(body.scenarios.length, 1);
    const sc = body.scenarios[0];
    assert.equal(sc.sid, fx.sid);
    assert.equal(sc.scenarioId, 'demo-scenario');
    assert.equal(sc.steps, 3);
    assert.equal(sc.latestRunId, fx.runId);
    assert.equal(sc.latestRun.summary, 'SUMMARY: 2/3 (FAIL)');
    assert.equal(sc.latestRun.state, 'done');
    assert.equal(sc.latestRun.ok, false);
  });

  await t.test('GET /runs returns replay history', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs`);
    const body = await res.json();
    assert.equal(body.replays.length, 1);
    assert.equal(body.replays[0].runId, fx.runId);
    assert.equal(body.replays[0].exitCode, 1);
  });

  await t.test('GET /runs/:runId parses events + status + audit', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}`);
    const body = await res.json();
    assert.equal(body.isLatest, true);
    assert.equal(body.status.state, 'done');
    assert.equal(body.audit.summary, 'SUMMARY: 2/3 (FAIL)');
    assert.equal(body.events.length, 4); // running + terminal for 2 steps
    const fail = body.events.find((e) => e.status === 'fail');
    assert.equal(fail.error, 'Element not found');
    assert.equal(fail.screenshot, 'screenshots/clickMissingLogin.png');
  });

  await t.test('GET artifact streams a captured screenshot', async () => {
    const res = await fetch(
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/screenshots/navHome`,
    );
    assert.equal(res.status, 200);
    assert.equal(res.headers.get('content-type'), 'image/png');
    assert.equal(await res.text(), 'PNGDATA-navHome');
  });

  await t.test('GET artifact streams a captured snapshot', async () => {
    const res = await fetch(
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/snapshots/navHome`,
    );
    assert.equal(res.status, 200);
    assert.match(await res.text(), /Example Domain/);
  });

  await t.test('missing artifact → 404 "not captured", not an error', async () => {
    const res = await fetch(
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/network/navHome`,
    );
    assert.equal(res.status, 404);
    const body = await res.json();
    assert.match(body.error, /not captured/);
  });

  await t.test('path-escape attempts are rejected', async () => {
    // Encoded traversal in the stepId segment.
    const escapes = [
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/screenshots/..%2f..%2f..%2fsecret`,
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/screenshots/%2e%2e`,
      `${base}/api/scenarios/..%2f${fx.sid}/runs/${fx.runId}/artifact/screenshots/navHome`,
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/evil/navHome`,
    ];
    for (const u of escapes) {
      const res = await fetch(u);
      assert.ok(res.status === 400 || res.status === 404, `expected 4xx for ${u}, got ${res.status}`);
      const text = await res.text();
      assert.doesNotMatch(text, /TOP SECRET/, `leak via ${u}`);
    }
  });

  await t.test('static index.html (React runs entry) is served', async () => {
    const res = await fetch(`${base}/`);
    assert.equal(res.status, 200);
    assert.match(res.headers.get('content-type'), /text\/html/);
    const html = await res.text();
    assert.match(html, /agent-qa · runs/);
    assert.match(html, /\/assets\/[A-Za-z0-9._-]+\.js/);
  });

  await t.test('GET /scenario returns the recorded definition', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/scenario`);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.sid, fx.sid);
    assert.equal(body.scenario.id, 'demo-scenario');
    assert.equal(body.scenario.steps.length, 3);
  });

  await t.test('GET /scenario for an unknown sid → 404', async () => {
    const res = await fetch(`${base}/api/scenarios/nope-no-such-sid/scenario`);
    assert.equal(res.status, 404);
  });
});

test('POST /replay spawns a replay via deps.replay', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    replay: async (sid, session) => {
      calls.push([sid, session]);
      return { ok: true, pid: 4242 };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());

  const res = await fetch(`${base}/api/scenarios/${fx.sid}/replay`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: '{}',
  });
  assert.equal(res.status, 202);
  const body = await res.json();
  assert.equal(body.ok, true);
  assert.equal(body.started, true);
  assert.equal(body.sid, fx.sid);
  // Replay is pinned to a deterministic per-sid session so the live
  // screencast can attach to exactly that browser.
  assert.deepEqual(calls, [[fx.sid, `replay-${fx.sid}`]]);
});

test('GET /replay-stream subscribes the per-session screencast bridge', async (t) => {
  const fx = makeFixture();
  const seen = [];
  const subs = [];
  const fakeBridge = {
    subscribe: (res) => {
      subs.push(res);
      res.write('data: {"data":"AAAA"}\n\n');
    },
    unsubscribe: (res) => {
      subs.splice(subs.indexOf(res), 1);
    },
  };
  const { server, base } = await boot(fx.root, {
    liveForSession: (session) => {
      seen.push(session);
      return fakeBridge;
    },
  });
  t.after(() => server.close());

  const ac = new AbortController();
  const res = await fetch(`${base}/api/scenarios/${fx.sid}/replay-stream`, { signal: ac.signal });
  assert.equal(res.status, 200);
  assert.match(res.headers.get('content-type'), /text\/event-stream/);
  const reader = res.body.getReader();
  const chunk = await reader.read();
  const text = Buffer.from(chunk.value).toString('utf8');
  assert.match(text, /AAAA/, 'a frame was streamed from the bridge');
  ac.abort();
  assert.deepEqual(seen, [`replay-${fx.sid}`], 'bridge keyed by the replay session');
});

test('GET /replay-stream without a CLI runner → 503', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root); // no deps.liveForSession
  t.after(() => server.close());
  const res = await fetch(`${base}/api/scenarios/${fx.sid}/replay-stream`);
  assert.equal(res.status, 503);
});

test('POST /replay for an unknown sid → 404, without spawning', async (t) => {
  const fx = makeFixture();
  let spawned = false;
  const { server, base } = await boot(fx.root, {
    replay: async () => {
      spawned = true;
      return { ok: true };
    },
  });
  t.after(() => server.close());
  const res = await fetch(`${base}/api/scenarios/ghost-sid/replay`, { method: 'POST', body: '{}' });
  assert.equal(res.status, 404);
  assert.equal(spawned, false);
});

test('POST /replay without a CLI runner → 503', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root); // no deps.replay
  t.after(() => server.close());
  const res = await fetch(`${base}/api/scenarios/${fx.sid}/replay`, { method: 'POST', body: '{}' });
  assert.equal(res.status, 503);
});

test('in-flight run is surfaced before latest.txt flips', async (t) => {
  // latest.txt only updates when a run finishes (cli/src/runner.rs), so an
  // active run must be discovered via status.json state === "running".
  const fx = makeFixture();
  const activeRun = '2026-06-24T16-00-00-000Z__feedface';
  const activeDir = path.join(fx.root, fx.sid, 'replays', activeRun);
  fs.mkdirSync(activeDir, { recursive: true });
  // Mid-flight: audit.json has startedAt only (no summary/finishedAt),
  // status.json says running. latest.txt still points at the OLD run.
  fs.writeFileSync(
    path.join(activeDir, 'audit.json'),
    JSON.stringify({ schema: 'scenario-replay-audit/v1', runId: activeRun, startedAt: '2026-06-24T16:00:00.000Z' }),
  );
  fs.writeFileSync(
    path.join(activeDir, 'status.json'),
    JSON.stringify({ state: 'running', currentIdx: 2, total: 3, ok: null }),
  );
  fs.writeFileSync(
    path.join(activeDir, 'events.jsonl'),
    JSON.stringify({ idx: 1, total: 3, id: 'navHome', kind: 'do:goto', status: 'pass', ms: 10 }) + '\n' +
      JSON.stringify({ idx: 2, total: 3, id: 'headingVisible', kind: 'check', status: 'running' }) + '\n',
  );

  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  const sc = (await (await fetch(`${base}/api/scenarios`)).json()).scenarios[0];
  assert.equal(sc.activeRunId, activeRun, 'active run discovered via status.json');
  assert.equal(sc.latestRunId, fx.runId, 'canonical latest still from latest.txt');
  assert.equal(sc.latestRun.runId, activeRun, 'current view prefers the active run');
  assert.equal(sc.latestRun.state, 'running');

  const runs = (await (await fetch(`${base}/api/scenarios/${fx.sid}/runs`)).json()).replays;
  const activeRow = runs.find((r) => r.runId === activeRun);
  assert.equal(activeRow.state, 'running');
  assert.equal(activeRow.summary, null);
});

test('serves the React app at /, /editor, /chat with hashed /assets', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  // /chat page route → the built React entry HTML referencing /assets.
  const page = await fetch(`${base}/chat`);
  assert.equal(page.status, 200);
  assert.match(page.headers.get('content-type') || '', /text\/html/);
  const html = await page.text();
  assert.match(html, /\/assets\//);

  // a hashed JS asset under /assets resolves with a JS content-type.
  const m = html.match(/\/assets\/[A-Za-z0-9._-]+\.js/);
  assert.ok(m, 'expected an /assets/*.js reference in the built HTML');
  const asset = await fetch(`${base}${m[0]}`);
  assert.equal(asset.status, 200);
  assert.match(asset.headers.get('content-type') || '', /javascript/);

  // /, /editor, /cases, /sets, /plans, /knowledge all serve the built entries.
  assert.equal((await fetch(`${base}/`)).status, 200);
  assert.equal((await fetch(`${base}/editor`)).status, 200);
  assert.equal((await fetch(`${base}/cases`)).status, 200);
  assert.equal((await fetch(`${base}/sets`)).status, 200);
  assert.equal((await fetch(`${base}/plans`)).status, 200);
  assert.equal((await fetch(`${base}/knowledge`)).status, 200);

  // traversal out of the assets subtree is rejected.
  const evil = await fetch(`${base}/assets/..%2F..%2Freport-server.js`);
  assert.equal(evil.status, 404);
});

test('test-case CRUD: upsert, list join, link, hidden from scenarios, delete', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root); // no deps → pure JSON surface
  t.after(() => server.close());

  const j = (m, p, body) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });

  // empty
  let r = await (await j('GET', '/api/cases')).json();
  assert.deepEqual(r.cases, []);

  // upsert → sealed case/1 with timestamps
  r = await (
    await j('POST', '/api/cases/login', {
      title: 'Log in',
      startUrl: 'https://x/',
      steps: ['Enter [EMAIL]', 'Submit'],
      expected: 'dashboard',
    })
  ).json();
  assert.equal(r.case.schema, 'case/1');
  assert.equal(r.case.id, 'login');
  assert.ok(r.case.createdAt && r.case.updatedAt);
  assert.ok(fs.existsSync(path.join(fx.root, '_cases', 'login', 'case.json')));

  // a case dir does NOT pollute the scenarios listing
  const scn = await (await fetch(`${base}/api/scenarios`)).json();
  assert.ok(!scn.scenarios.some((s) => s.sid === '_cases'));

  // link to the fixture scenario → list joins its last-run summary
  await j('POST', '/api/cases/login/link', { scenarioSid: fx.sid });
  r = await (await j('GET', '/api/cases')).json();
  assert.equal(r.cases.length, 1);
  assert.equal(r.cases[0].scenarioSid, fx.sid);
  assert.equal(r.cases[0].scenario.latestRun.ok, false);

  // partial save does not clobber the linked sid
  r = await (await j('POST', '/api/cases/login', { title: 'Log in v2' })).json();
  assert.equal(r.case.scenarioSid, fx.sid);
  assert.equal(r.case.title, 'Log in v2');

  // unsafe id rejected
  assert.equal((await j('POST', '/api/cases/..%2Fevil', {})).status, 400);

  // delete removes the case dir but leaves the scenario intact
  assert.equal((await j('POST', '/api/cases/login/delete')).status, 200);
  assert.ok(!fs.existsSync(path.join(fx.root, '_cases', 'login')));
  assert.ok(fs.existsSync(path.join(fx.root, fx.sid, 'scenario.json')));
});

test('case carries externalRefs (provider-agnostic links)', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());
  const j = (m, p, body) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });

  // unset → empty array; a provided ref is sealed (url defaults to null)
  let r = await (await j('POST', '/api/cases/login', { title: 'Log in' })).json();
  assert.deepEqual(r.case.externalRefs, []);
  r = await (
    await j('POST', '/api/cases/login', {
      externalRefs: [{ provider: 'demo', key: 'AB-1', url: 'https://x/AB-1' }, { key: 'AB-2' }],
    })
  ).json();
  assert.deepEqual(r.case.externalRefs, [
    { provider: 'demo', key: 'AB-1', url: 'https://x/AB-1' },
    { provider: '', key: 'AB-2', url: null },
  ]);
});

test('test-set CRUD: manual + tag membership, resolved cases, hidden from scenarios, delete', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  const j = (m, p, body) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });

  // two cases, one tagged "smoke"
  await j('POST', '/api/cases/login', { title: 'Log in', tags: ['smoke', 'auth'] });
  await j('POST', '/api/cases/checkout', { title: 'Checkout', tags: ['payments'] });

  // empty
  let r = await (await j('GET', '/api/sets')).json();
  assert.deepEqual(r.sets, []);

  // manual set with one member → sealed set/1, count reflects membership
  r = await (
    await j('POST', '/api/sets/smoke', { name: 'Smoke', mode: 'manual', caseIds: ['login'] })
  ).json();
  assert.equal(r.set.schema, 'set/1');
  assert.equal(r.set.mode, 'manual');
  assert.ok(fs.existsSync(path.join(fx.root, '_sets', 'smoke', 'set.json')));
  r = await (await j('GET', '/api/sets/smoke')).json();
  assert.equal(r.set.caseCount, 1);

  // a set dir does NOT pollute the scenarios listing
  const scn = await (await fetch(`${base}/api/scenarios`)).json();
  assert.ok(!scn.scenarios.some((s) => s.sid === '_sets'));

  // resolved member cases (joined to scenario summary, null until linked)
  r = await (await j('GET', '/api/sets/smoke/cases')).json();
  assert.equal(r.cases.length, 1);
  assert.equal(r.cases[0].id, 'login');

  // manual membership ignores ids that don't exist
  await j('POST', '/api/sets/smoke', { caseIds: ['login', 'ghost'] });
  r = await (await j('GET', '/api/sets/smoke/cases')).json();
  assert.equal(r.cases.length, 1);

  // tag set resolves by any-of label match, stays live as cases change
  await j('POST', '/api/sets/tagged', { name: 'Tagged', mode: 'tag', tagQuery: ['smoke'] });
  r = await (await j('GET', '/api/sets/tagged/cases')).json();
  assert.deepEqual(r.cases.map((c) => c.id), ['login']);
  await j('POST', '/api/cases/checkout', { tags: ['smoke'] });
  r = await (await j('GET', '/api/sets/tagged/cases')).json();
  assert.deepEqual(r.cases.map((c) => c.id).sort(), ['checkout', 'login']);

  // unsafe id rejected; delete removes the set, leaves cases intact
  assert.equal((await j('POST', '/api/sets/..%2Fevil', {})).status, 400);
  assert.equal((await j('POST', '/api/sets/smoke/delete')).status, 200);
  assert.ok(!fs.existsSync(path.join(fx.root, '_sets', 'smoke')));
  assert.ok(fs.existsSync(path.join(fx.root, '_cases', 'login', 'case.json')));
});

test('test-plan CRUD + scope resolution (union of sets and cases, deduped)', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  const j = (m, p, body) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });

  await j('POST', '/api/cases/login', { title: 'Log in', tags: ['smoke'] });
  await j('POST', '/api/cases/checkout', { title: 'Checkout', tags: ['smoke'] });
  await j('POST', '/api/cases/profile', { title: 'Profile' });
  await j('POST', '/api/sets/smoke', { name: 'Smoke', mode: 'tag', tagQuery: ['smoke'] });

  // empty
  let r = await (await j('GET', '/api/plans')).json();
  assert.deepEqual(r.plans, []);

  // plan from a set + a direct case → sealed plan/1, deduped union resolved
  r = await (
    await j('POST', '/api/plans/release', {
      name: 'Release',
      scope: { setIds: ['smoke'], caseIds: ['profile', 'login'] }, // login also via set
    })
  ).json();
  assert.equal(r.plan.schema, 'plan/1');
  assert.ok(fs.existsSync(path.join(fx.root, '_plans', 'release', 'plan.json')));
  r = await (await j('GET', '/api/plans/release')).json();
  assert.equal(r.plan.caseCount, 3); // login, checkout (set) + profile (direct), login not doubled

  // resolved order: set members first (in case-list / alphabetical order:
  // checkout, login), then the new direct case (profile); login isn't doubled
  r = await (await j('GET', '/api/plans/release/cases')).json();
  assert.deepEqual(
    r.cases.map((c) => c.id),
    ['checkout', 'login', 'profile']
  );

  // a plan dir does NOT pollute the scenarios listing
  const scn = await (await fetch(`${base}/api/scenarios`)).json();
  assert.ok(!scn.scenarios.some((s) => s.sid === '_plans'));

  // run without a resolved CLI → 503 (no deps in this harness)
  assert.equal((await j('POST', '/api/plans/release/run')).status, 503);

  // unsafe id rejected; delete leaves sets + cases intact
  assert.equal((await j('POST', '/api/plans/..%2Fevil', {})).status, 400);
  assert.equal((await j('POST', '/api/plans/release/delete')).status, 200);
  assert.ok(!fs.existsSync(path.join(fx.root, '_plans', 'release')));
  assert.ok(fs.existsSync(path.join(fx.root, '_sets', 'smoke', 'set.json')));
});

test('POST /api/plans/:id/run replays each member scenario via deps.replay', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    replay: async (sid, session, opts) => {
      calls.push({ sid, session, opts });
      return { ok: true };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());

  const j = (m, p, body) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });

  // one case linked to the fixture scenario, one without a recording
  await j('POST', '/api/cases/login', { title: 'Log in' });
  await j('POST', '/api/cases/login/link', { scenarioSid: fx.sid });
  await j('POST', '/api/cases/draft', { title: 'Draft (no recording)' });
  await j('POST', '/api/plans/p1', { name: 'P1', scope: { caseIds: ['login', 'draft'] } });

  // run with a persona (→ --profile) and environment values (→ --param)
  const res = await j('POST', '/api/plans/p1/run', {
    profile: 'qa-admin',
    params: { baseUrl: 'https://staging.example.com' },
  });
  assert.equal(res.status, 202);
  const out = await res.json();
  assert.deepEqual(out.started, [{ caseId: 'login', sid: fx.sid }]);
  assert.equal(out.skipped.length, 1);
  assert.equal(out.skipped[0].caseId, 'draft');
  assert.deepEqual(calls, [
    {
      sid: fx.sid,
      // A persona-scoped replay reuses the profile's persistent, authed session
      // (so re-runs skip the OAuth) rather than a throwaway replay-<sid>.
      session: 'qa-admin-session',
      // env is the injected plugin registry — empty here (none registered)
      opts: { profile: 'qa-admin', params: { baseUrl: 'https://staging.example.com' }, env: {} },
    },
  ]);
});

test('persona + environment CRUD (run-config records)', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());
  const j = (m, p, body) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });

  // personas: upsert seals persona/1 incl. a credentials block, list, delete
  let r = await (await j('GET', '/api/personas')).json();
  assert.deepEqual(r.personas, []);
  r = await (
    await j('POST', '/api/personas/admin', {
      name: 'Admin',
      profile: 'admin-user',
      credentials: { entries: { APP_EMAIL: 'a@b.com', APP_PASSWORD: 'pw' } },
    })
  ).json();
  assert.equal(r.persona.schema, 'persona/1');
  assert.equal(r.persona.profile, 'admin-user');
  assert.deepEqual(r.persona.credentials, { entries: { APP_EMAIL: 'a@b.com', APP_PASSWORD: 'pw' } });
  assert.ok(fs.existsSync(path.join(fx.root, '_personas', 'admin', 'persona.json')));

  // environments: params + auth.config coerced to strings, auth block sealed
  r = await (
    await j('POST', '/api/environments/staging', {
      name: 'Staging',
      baseUrl: 'https://staging.example.com',
      params: { region: 'eu', flag: true },
      auth: { plugin: 'agent-qa-plugin-acme', loginUrl: 'https://staging.example.com/sso', config: { tenant: 7 } },
    })
  ).json();
  assert.equal(r.environment.schema, 'environment/1');
  assert.deepEqual(r.environment.params, { region: 'eu', flag: 'true' });
  assert.equal(r.environment.auth.plugin, 'agent-qa-plugin-acme');
  assert.deepEqual(r.environment.auth.config, { tenant: '7' });

  // listed + unsafe id rejected + delete
  assert.equal((await (await j('GET', '/api/environments')).json()).environments.length, 1);
  assert.equal((await j('POST', '/api/personas/..%2Fevil', {})).status, 400);
  assert.equal((await j('POST', '/api/personas/admin/delete')).status, 200);
  assert.ok(!fs.existsSync(path.join(fx.root, '_personas', 'admin')));
});

test('GET /api/plugins reports discovered auth plugins (read-only)', async (t) => {
  const fx = makeFixture();

  // no CLI resolved → graceful "not available", not an error
  let booted = await boot(fx.root);
  let res = await (await fetch(`${booted.base}/api/plugins`)).json();
  assert.deepEqual(res, { available: false, plugins: [] });
  booted.server.close();

  // with a runCli, parses `plugins list --json`
  const deps = {
    runCli: async (args) => ({
      stdout: args.join(' ') === 'plugins list --json' ? '[{"kind":"auth","name":"acme"}]' : '[]',
      stderr: '',
      code: 0,
    }),
  };
  booted = await boot(fx.root, deps);
  t.after(() => booted.server.close());
  res = await (await fetch(`${booted.base}/api/plugins`)).json();
  assert.equal(res.available, true);
  assert.deepEqual(res.plugins, [{ kind: 'auth', name: 'acme' }]);
});

test('POST /api/personas/:id/connect bootstraps a profile via the auth plugin', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    runCli: async (args, extraEnv) => {
      calls.push({ args, extraEnv });
      // simulate no CLI-discoverable auth plugin (agent-qa.toml / $PATH empty)
      if (args[0] === 'plugins' && args[1] === 'path') return { code: 1, stdout: '', stderr: 'no plugin' };
      if (args[0] === 'profile-status') return { code: 0, stdout: 'admin-user: authenticated', stderr: '' };
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };

  // no CLI → 503
  let booted = await boot(fx.root);
  let j0 = (m, p, b) =>
    fetch(`${booted.base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });
  await j0('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  await j0('POST', '/api/environments/staging', {
    name: 'Staging',
    baseUrl: 'https://s.example.com',
    auth: { plugin: 'agent-qa-plugin-acme', config: { tenant: '7' } },
  });
  assert.equal((await j0('POST', '/api/personas/admin/connect', { environmentId: 'staging' })).status, 503);
  booted.server.close();

  // with a runCli → add → bootstrap → status, env config surfaced to plugin
  booted = await boot(fx.root, deps);
  t.after(() => booted.server.close());
  const j = (m, p, b) =>
    fetch(`${booted.base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  const res = await j('POST', '/api/personas/admin/connect', { environmentId: 'staging' });
  assert.equal(res.status, 200);
  const out = await res.json();
  assert.equal(out.authenticated, true);
  assert.equal(out.profile, 'admin-user');
  assert.deepEqual(
    calls.map((c) => c.args[0]),
    ['profile-add', 'profile-bootstrap', 'profile-status']
  );
  // profile-add registers the profile + adapter-name preference; the plugin
  // itself is discovered from the registry. Credentials reach the plugin via
  // the injected env, not profile-add flags.
  assert.deepEqual(calls[0].args, ['profile-add', 'admin-user', '--adapter', 'agent-qa-plugin-acme']);
  assert.equal(calls[1].extraEnv.AGENT_QA_ENV_BASE_URL, 'https://s.example.com');
  assert.equal(calls[1].extraEnv.AGENT_QA_ENV_TENANT, '7');

  // no auth plugin (no registry entry + no env adapter) → 400
  await j('POST', '/api/environments/noplug', { name: 'NoPlug' });
  assert.equal((await j('POST', '/api/personas/admin/connect', { environmentId: 'noplug' })).status, 400);
});

test("POST /api/chat/c/:id/connect bootstraps auth into THAT chat's own session", async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    chat: { hub: {} }, // chat available so a chat can be created
    runCli: async (args) => {
      calls.push(args);
      if (args[0] === 'profile-status') return { code: 0, stdout: 'admin-user: authenticated', stderr: '' };
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const booted = await boot(fx.root, deps);
  t.after(() => booted.server.close());
  const j = (m, p, b) =>
    fetch(`${booted.base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  await j('POST', '/api/environments/staging', { name: 'Staging', auth: { plugin: 'agent-qa-plugin-acme' } });

  const created = await (await j('POST', '/api/chat/create')).json();
  assert.match(created.session, /^chat-[0-9a-f]+$/);

  const res = await j('POST', `/api/chat/c/${created.id}/connect`, { personaId: 'admin', environmentId: 'staging' });
  assert.equal(res.status, 200);
  const out = await res.json();
  assert.equal(out.authenticated, true);
  assert.equal(out.profile, 'admin-user');
  // The cookie must land in THIS chat's browser session, not the per-profile
  // default — so bootstrap + status carry --session <chat session>.
  assert.equal(out.session, created.session);
  const bootCall = calls.find((a) => a[0] === 'profile-bootstrap');
  const statCall = calls.find((a) => a[0] === 'profile-status');
  assert.deepEqual(bootCall, ['profile-bootstrap', 'admin-user', '--session', created.session]);
  assert.deepEqual(statCall, ['profile-status', 'admin-user', '--session', created.session]);

  // personaId is required
  assert.equal((await j('POST', `/api/chat/c/${created.id}/connect`, {})).status, 400);
});

test('POST /api/chat/c/:id/replay re-auths via the connected persona, in its session', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    chat: { hub: {} },
    recordRoot: path.join(fx.root, 'rec'),
    runCli: async (args, extraEnv) => {
      calls.push({ args, env: extraEnv || {} });
      if (args[0] === 'profile-status') return { code: 0, stdout: 'admin-user: authenticated', stderr: '' };
      return { code: 0, stdout: 'SUMMARY: 1/1 (PASS)', stderr: '' };
    },
  };
  const booted = await boot(fx.root, deps);
  t.after(() => booted.server.close());
  const j = (m, p, b) =>
    fetch(`${booted.base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  await j('POST', '/api/personas/admin', {
    name: 'Admin',
    profile: 'admin-user',
    credentials: { entries: { APP_CLIENT_ID: 'cid-literal' } },
  });
  const created = await (await j('POST', '/api/chat/create')).json();

  // Replay before connecting a persona → 400 (nothing to re-auth as).
  assert.equal((await j('POST', `/api/chat/c/${created.id}/replay`, { sid: 's-x' })).status, 400);

  // Connect, then replay through the workbench (which injects creds).
  await j('POST', `/api/chat/c/${created.id}/connect`, { personaId: 'admin' });
  const res = await j('POST', `/api/chat/c/${created.id}/replay`, { sid: 's-2026' });
  assert.equal(res.status, 200);
  assert.equal((await res.json()).ok, true);

  const replay = calls.find((c) => c.args[0] === 'replay');
  // Runs in the chat's own (already-authed) session, under the connected profile.
  assert.deepEqual(replay.args, ['replay', 's-2026', '--session', created.session, '--profile', 'admin-user']);
  // Persona credentials are injected so the useProfile op can re-authenticate.
  assert.equal(replay.env.APP_CLIENT_ID, 'cid-literal');
  // ...and in the chat's record dir, where the connected profile is registered.
  assert.ok(String(replay.env.AGENT_QA_RECORD_DIR || '').includes(created.session));

  // sid is required.
  assert.equal((await j('POST', `/api/chat/c/${created.id}/replay`, {})).status, 400);
});

test('persona credentials inject into the plugin env; unresolved vault refs fail connect', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    runCli: async (args, extraEnv) => {
      calls.push({ args, extraEnv });
      if (args[0] === 'profile-status') return { code: 0, stdout: 'authenticated', stderr: '' };
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  await j('POST', '/api/environments/staging', { name: 'Staging', auth: { plugin: 'agent-qa-plugin-acme' } });

  // literal credential entries are injected into the bootstrap call's env
  await j('POST', '/api/personas/admin', {
    name: 'Admin',
    profile: 'admin-user',
    credentials: { entries: { APP_EMAIL: 'a@b.com', APP_PASSWORD: 'pw' } },
  });
  const res = await j('POST', '/api/personas/admin/connect', { environmentId: 'staging' });
  assert.equal(res.status, 200);
  const bootCall = calls.find((c) => c.args[0] === 'profile-bootstrap');
  assert.equal(bootCall.extraEnv.APP_EMAIL, 'a@b.com');
  assert.equal(bootCall.extraEnv.APP_PASSWORD, 'pw');

  // a vault: ref with no VAULT_ADDR can't resolve → connect fails, no bootstrap
  process.env.VAULT_ADDR = ''; // force "no endpoint" so the ref is unresolvable (no network)
  await j('POST', '/api/personas/vaulted', {
    name: 'Vaulted',
    profile: 'vaulted',
    credentials: { entries: { APP_EMAIL: 'vault:dev/data/x:EMAIL' } },
  });
  calls.length = 0;
  const r2 = await (await j('POST', '/api/personas/vaulted/connect', { environmentId: 'staging' })).json();
  assert.equal(r2.ok, false);
  assert.ok(r2.log.some((s) => s.step === 'vault'));
  assert.ok(!calls.some((c) => c.args[0] === 'profile-bootstrap'));
});

test('plan run + scenario replay inject the persona credentials and self-bootstrap the profile', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const replays = [];
  const deps = {
    runCli: async (args, extraEnv) => {
      calls.push({ args, extraEnv });
      return { code: 0, stdout: 'ok', stderr: '' };
    },
    replay: async (sid, session, opts) => {
      replays.push({ sid, session, opts });
      return { ok: true };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  await j('POST', '/api/environments/staging', {
    name: 'Staging',
    baseUrl: 'https://s.example.com',
    auth: { plugin: 'agent-qa-plugin-acme', config: { tenant: '7' } },
  });
  await j('POST', '/api/personas/admin', {
    name: 'Admin',
    profile: 'admin-user',
    credentials: { entries: { APP_EMAIL: 'a@b.com', APP_PASSWORD: 'pw' } },
  });

  // Runs-tab replay with a persona: the server registers the profile against
  // the env's plugin (idempotent profile-add) and injects the persona's creds
  // + the environment's connection config into the replay env, so replay's
  // own useProfile op re-authenticates on the fresh replay session.
  const rep = await j('POST', `/api/scenarios/${fx.sid}/replay`, {
    personaId: 'admin',
    environmentId: 'staging',
  });
  assert.equal(rep.status, 202);
  assert.deepEqual(calls[0].args, ['profile-add', 'admin-user', '--adapter', 'agent-qa-plugin-acme']);
  assert.equal(replays.length, 1);
  assert.equal(replays[0].opts.profile, 'admin-user');
  assert.equal(replays[0].opts.env.APP_EMAIL, 'a@b.com');
  assert.equal(replays[0].opts.env.APP_PASSWORD, 'pw');
  assert.equal(replays[0].opts.env.AGENT_QA_ENV_BASE_URL, 'https://s.example.com');
  assert.equal(replays[0].opts.env.AGENT_QA_ENV_TENANT, '7');

  // Plan run with a persona: same injection, per member scenario.
  calls.length = 0;
  replays.length = 0;
  await j('POST', '/api/cases/login', { title: 'Log in' });
  await j('POST', '/api/cases/login/link', { scenarioSid: fx.sid });
  await j('POST', '/api/plans/p1', { name: 'P1', scope: { caseIds: ['login'] } });
  const run = await j('POST', '/api/plans/p1/run', { personaId: 'admin', environmentId: 'staging' });
  assert.equal(run.status, 202);
  assert.ok(calls.some((c) => c.args[0] === 'profile-add' && c.args[1] === 'admin-user'));
  assert.equal(replays.length, 1);
  assert.equal(replays[0].opts.profile, 'admin-user');
  assert.equal(replays[0].opts.env.APP_EMAIL, 'a@b.com');

  // A persona whose vault ref can't resolve fails the run up front — no replay.
  process.env.VAULT_ADDR = '';
  await j('POST', '/api/personas/vaulted', {
    name: 'Vaulted',
    profile: 'vaulted',
    credentials: { entries: { APP_PASSWORD: 'vault:dev/data/x:PW' } },
  });
  replays.length = 0;
  const bad = await (await j('POST', '/api/plans/p1/run', { personaId: 'vaulted', environmentId: 'staging' })).json();
  assert.equal(bad.ok, false);
  assert.match(bad.error, /vault/i);
  assert.equal(replays.length, 0);
});

test('environment shared creds layer under persona creds (persona wins on conflict)', async (t) => {
  const fx = makeFixture();
  const replays = [];
  const deps = {
    runCli: async () => ({ code: 0, stdout: 'ok', stderr: '' }),
    replay: async (sid, session, opts) => {
      replays.push({ sid, session, opts });
      return { ok: true };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // Environment carries the app-level creds every identity shares, plus a key
  // that also appears on the persona (to prove precedence).
  await j('POST', '/api/environments/staging', {
    name: 'Staging',
    auth: {
      plugin: 'agent-qa-plugin-acme',
      creds: { APP_CLIENT_ID: 'cid-shared', OVERLAP: 'from-env' },
    },
  });
  // Persona carries only what varies + overrides OVERLAP.
  await j('POST', '/api/personas/admin', {
    name: 'Admin',
    profile: 'admin-user',
    credentials: { entries: { APP_EMAIL: 'a@b.com', OVERLAP: 'from-persona' } },
  });

  const rep = await j('POST', `/api/scenarios/${fx.sid}/replay`, {
    personaId: 'admin',
    environmentId: 'staging',
  });
  assert.equal(rep.status, 202);
  assert.equal(replays.length, 1);
  const env = replays[0].opts.env;
  assert.equal(env.APP_CLIENT_ID, 'cid-shared'); // shared from the environment
  assert.equal(env.APP_EMAIL, 'a@b.com'); // identity from the persona
  assert.equal(env.OVERLAP, 'from-persona'); // persona wins on a key collision

  // The stored environment persists the creds block.
  const got = await (await j('GET', '/api/environments/staging')).json();
  assert.deepEqual(got.environment.auth.creds, { APP_CLIENT_ID: 'cid-shared', OVERLAP: 'from-env' });
});

test('replay with no environmentId falls back to the default/sole environment (injects its shared creds)', async (t) => {
  const fx = makeFixture();
  const replays = [];
  const deps = {
    runCli: async () => ({ code: 0, stdout: 'ok', stderr: '' }),
    replay: async (sid, session, opts) => {
      replays.push({ sid, session, opts });
      return { ok: true };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // A single environment carrying the shared app cred, and a persona.
  await j('POST', '/api/environments/staging', {
    name: 'Staging',
    auth: { plugin: 'agent-qa-plugin-acme', creds: { APP_CLIENT_ID: 'cid-shared' } },
  });
  await j('POST', '/api/personas/admin', {
    name: 'Admin',
    profile: 'admin-user',
    credentials: { entries: { APP_EMAIL: 'a@b.com' } },
  });

  // Replay names the persona but NOT the environment — the sole env is used.
  const rep = await j('POST', `/api/scenarios/${fx.sid}/replay`, { personaId: 'admin' });
  assert.equal(rep.status, 202);
  assert.equal(replays.length, 1);
  assert.equal(replays[0].opts.env.APP_CLIENT_ID, 'cid-shared'); // shared cred injected from the default env
  assert.equal(replays[0].opts.env.APP_EMAIL, 'a@b.com');
});

test('package-provided personas are discovered read-only; local shadows; writes refused', async (t) => {
  const fx = makeFixture();
  // Isolated config home with a package persona dir registered in agent-qa.toml.
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-home-'));
  const pkgDir = path.join(home, 'packages', 'git', 'agent-qa-acme', 'personas');
  fs.mkdirSync(pkgDir, { recursive: true });
  fs.writeFileSync(
    path.join(pkgDir, 'acme-admin.json'),
    JSON.stringify({
      schema: 'persona/1',
      id: 'acme-admin',
      name: 'Acme Admin',
      profile: 'acme-admin',
      credentials: { entries: { APP_EMAIL: 'vault:dev/x:EMAIL' } },
    })
  );
  fs.writeFileSync(
    path.join(home, 'agent-qa.toml'),
    `[personas]\nextra-dirs = [\n  ${JSON.stringify(pkgDir)},\n]\n`
  );
  const prevHome = process.env.AGENT_QA_HOME;
  process.env.AGENT_QA_HOME = home;
  t.after(() => {
    if (prevHome === undefined) delete process.env.AGENT_QA_HOME;
    else process.env.AGENT_QA_HOME = prevHome;
  });

  const { server, base } = await boot(fx.root);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // Listed read-only, tagged with the package it came from.
  let list = (await (await j('GET', '/api/personas')).json()).personas;
  const pkg = list.find((p) => p.id === 'acme-admin');
  assert.ok(pkg, 'package persona is listed');
  assert.equal(pkg.readOnly, true);
  assert.equal(pkg.source, 'agent-qa-acme');

  // Editing / deleting a package record is refused.
  assert.equal((await j('POST', '/api/personas/acme-admin', { name: 'X', profile: 'x' })).status, 409);
  assert.equal((await j('POST', '/api/personas/acme-admin/delete')).status, 409);

  // Cloning under a new id creates a local, editable copy.
  const clone = await j('POST', '/api/personas/acme-copy', {
    name: 'Acme (mine)',
    profile: 'acme-copy',
    credentials: { entries: { APP_EMAIL: 'vault:dev/x:EMAIL' } },
  });
  assert.equal(clone.status, 200);
  list = (await (await j('GET', '/api/personas')).json()).personas;
  assert.equal(list.find((p) => p.id === 'acme-copy').readOnly, false);

  // A local record with the SAME id as a package one shadows it (local wins).
  fs.mkdirSync(path.join(fx.root, '_personas', 'acme-admin'), { recursive: true });
  fs.writeFileSync(
    path.join(fx.root, '_personas', 'acme-admin', 'persona.json'),
    JSON.stringify({ schema: 'persona/1', id: 'acme-admin', name: 'Local override', profile: 'acme-admin', credentials: { entries: {} } })
  );
  list = (await (await j('GET', '/api/personas')).json()).personas;
  const shadowed = list.filter((p) => p.id === 'acme-admin');
  assert.equal(shadowed.length, 1);
  assert.equal(shadowed[0].readOnly, false);
  assert.equal(shadowed[0].name, 'Local override');
});

test('plugin registry persists + injects AGENT_QA_PLUGINS into CLI calls', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    runCli: async (args, extraEnv) => {
      calls.push({ args, extraEnv });
      if (args[0] === 'plugins') return { code: 0, stdout: '[]', stderr: '' };
      if (args[0] === 'profile-status') return { code: 0, stdout: 'admin-user: authenticated', stderr: '' };
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // empty → set (trim + dedupe) → persisted on disk
  assert.deepEqual((await (await j('GET', '/api/config/plugins')).json()).paths, []);
  const r = await (await j('POST', '/api/config/plugins', { paths: [' /p/auth ', '/p/auth', '/p/policy'] })).json();
  assert.deepEqual(r.paths, ['/p/auth', '/p/policy']);
  assert.ok(fs.existsSync(path.join(fx.root, '_config', 'plugins.json')));

  // discovery injects the registry as AGENT_QA_PLUGINS
  await (await j('GET', '/api/plugins')).json();
  const disc = calls.find((c) => c.args[0] === 'plugins');
  assert.equal(disc.extraEnv.AGENT_QA_PLUGINS, '/p/auth:/p/policy');

  // connect (bootstrap) injects it too
  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  await j('POST', '/api/environments/staging', { name: 'Staging', auth: { plugin: 'agent-qa-plugin-acme' } });
  await j('POST', '/api/personas/admin/connect', { environmentId: 'staging' });
  const bootCall = calls.find((c) => c.args[0] === 'profile-bootstrap');
  assert.equal(bootCall.extraEnv.AGENT_QA_PLUGINS, '/p/auth:/p/policy');
});

test('POST /api/config/plugins/import saves, chmods, and registers a plugin file', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  const content = '#!/bin/sh\necho hi\n';
  const r = await (
    await j('POST', '/api/config/plugins/import', {
      filename: '../evil/agent-qa-plugin-x', // path stripped to basename
      contentBase64: Buffer.from(content).toString('base64'),
    })
  ).json();
  const dest = path.join(fx.root, '_config', 'plugins', 'agent-qa-plugin-x');
  assert.equal(r.path, dest);
  assert.deepEqual(r.paths, [dest]); // auto-registered
  assert.equal(fs.readFileSync(dest, 'utf8'), content);
  assert.ok(fs.statSync(dest).mode & 0o100); // executable bit set

  // bad input rejected
  assert.equal((await j('POST', '/api/config/plugins/import', { filename: 'x' })).status, 400);
});
