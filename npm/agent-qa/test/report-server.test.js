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

  await t.test('static index.html is served', async () => {
    const res = await fetch(`${base}/`);
    assert.equal(res.status, 200);
    assert.match(res.headers.get('content-type'), /text\/html/);
    assert.match(await res.text(), /report view/);
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
