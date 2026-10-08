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
const { EventEmitter } = require('node:events');

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
      steps: [
        { id: 'navHome', kind: 'do' },
        {
          id: 'headingVisible',
          kind: 'check',
          claim: { subject: { domshot: 'navHome' }, predicate: 'matches' },
        },
        { id: 'clickMissingLogin', kind: 'do' },
      ],
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
      autoHealed: ['navHome'],
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
  fs.writeFileSync(path.join(runDir, 'run.webm'), Buffer.from('WEBMDATA'));
  fs.writeFileSync(path.join(runDir, 'snapshots', 'navHome.txt'), 'heading "Example Domain"\n');

  // A secret OUTSIDE the run dir that a path-escape attempt would target.
  fs.writeFileSync(path.join(root, 'secret.txt'), 'TOP SECRET');

  // Heal trail: navHome's role+name locator drifted and auto-heal retried it;
  // clickMissingLogin was classified a value rejection (no patch, no retry).
  fs.mkdirSync(path.join(runDir, 'diffs'), { recursive: true });
  fs.writeFileSync(
    path.join(runDir, 'heal.jsonl'),
    [
      JSON.stringify({
        schema: 'heal-row/v1',
        ts: '2026-06-24T15:07:24.001Z',
        runId,
        stepId: 'navHome',
        mode: 'locator-correction',
        strategy: 'digits-tolerant',
        from: 'Users 2',
        to: 'Users 1',
      }),
      JSON.stringify({
        schema: 'heal-row/v1',
        ts: '2026-06-24T15:07:26.500Z',
        runId,
        stepId: 'clickMissingLogin',
        mode: 'value-rejection',
        rationale: 'field validationMessage: Value required',
      }),
    ].join('\n') + '\n',
  );
  fs.mkdirSync(path.join(runDir, 'shots-diff'), { recursive: true });
  fs.writeFileSync(path.join(runDir, 'shots-diff', 'navHome.diff.png'), 'DIFFDATA-navHome');

  // A unified text diff for a {"domshot"} miss on navHome.
  fs.mkdirSync(path.join(runDir, 'domshots-diff'), { recursive: true });
  fs.writeFileSync(
    path.join(runDir, 'domshots-diff', 'navHome.diff.txt'),
    '--- baselines/navHome.snap.txt\n+++ snapshots/navHome.txt\n- heading "Old" [ref=@e]\n+ heading "New" [ref=@e]\n',
  );

  fs.writeFileSync(
    path.join(runDir, 'diffs', 'navHome.patch.json'),
    JSON.stringify({
      schema: 'heal-patch/v1',
      stepId: 'navHome',
      scenarioContentHash: 'abc123',
      newLocator: { role: 'button', name: 'Users 1' },
      rationale: 'auto-heal via digits-tolerant: "Users 2" → "Users 1"',
    }),
  );

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

test('workbench run options default headless and replay args pin the CLI mode', () => {
  assert.deepEqual(srv._test.runOptsFromBody({}), { headed: false });
  assert.deepEqual(srv._test.runOptsFromBody({ headed: 'true' }), { headed: false });
  assert.deepEqual(srv._test.runOptsFromBody({ headed: true, profile: 'viewer' }), {
    headed: true,
    profile: 'viewer',
  });
  assert.deepEqual(srv._test.replayArgs('s-demo', 'replay-s-demo', {}), [
    'replay',
    's-demo',
    '--session',
    'replay-s-demo',
    '--headless',
  ]);
  assert.deepEqual(srv._test.replayArgs('s-demo', null, { headed: true, params: { baseUrl: 'https://example.com' } }), [
    'replay',
    's-demo',
    '--headed',
    '--param',
    'baseUrl=https://example.com',
  ]);
});

test('browser mode preparer preserves warm sessions and blocks in-flight mode flips', async () => {
  const closed = [];
  const prepare = srv._test.makeBrowserModePreparer({
    closeSession: async (session) => closed.push(session),
    activeSessions: () => ['warm-session'],
  });

  // Unknown warm daemon: recycle once because its launch mode cannot be known.
  const releaseHeadless = await prepare('warm-session', false);
  // An opposite-mode request cannot close a session still in use.
  await assert.rejects(() => prepare('warm-session', true), /busy in headless mode/);
  assert.deepEqual(closed, ['warm-session']);
  releaseHeadless();

  // Known same mode: preserve the warm session. A later flip recycles once.
  const releaseHeadlessAgain = await prepare('warm-session', false);
  releaseHeadlessAgain();
  const releaseHeaded = await prepare('warm-session', true);
  releaseHeaded();

  // A new/cold session needs no close.
  const releaseCold = await prepare('cold-session', false);
  releaseCold();

  assert.deepEqual(closed, ['warm-session', 'warm-session']);
});

test('detached replay holds its browser-mode lease until the child finishes', async () => {
  const closed = [];
  const prepareBrowserSession = srv._test.makeBrowserModePreparer({
    closeSession: async (session) => closed.push(session),
    activeSessions: () => [],
  });
  let finishReplay;
  const done = new Promise((resolve) => { finishReplay = resolve; });
  const calls = [];
  const deps = {
    prepareBrowserSession,
    replay: async (_sid, _session, opts) => {
      calls.push(opts.headed);
      return { ok: true, done };
    },
  };

  await srv._test.launchReplay(deps, 's-one', 'shared-session', { headed: false });
  await assert.rejects(
    () => srv._test.launchReplay(deps, 's-two', 'shared-session', { headed: true }),
    /busy in headless mode/,
  );
  assert.deepEqual(calls, [false]);
  assert.deepEqual(closed, []);

  finishReplay();
  await Promise.resolve();
  await srv._test.launchReplay(deps, 's-two', 'shared-session', { headed: true });
  assert.deepEqual(calls, [false, true]);
  assert.deepEqual(closed, ['shared-session']);
});

test('replay setup timeout defaults to three minutes and supports override/disable', () => {
  assert.equal(srv._test.replaySetupTimeoutMs({}), 180000);
  assert.equal(srv._test.replaySetupTimeoutMs({ AGENT_QA_REPLAY_SETUP_TIMEOUT_MS: '45000' }), 45000);
  assert.equal(srv._test.replaySetupTimeoutMs({ AGENT_QA_REPLAY_SETUP_TIMEOUT_MS: '0' }), 0);
  assert.equal(srv._test.replaySetupTimeoutMs({ AGENT_QA_REPLAY_SETUP_TIMEOUT_MS: 'bad' }), 180000);
});

test('a replay process killed before setup finishes is finalized as failed', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-replay-exit-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const sid = 's-crashed-replay';
  const runId = '2026-08-13T14-44-34-767Z__dcdc95af__admin-user';
  const runDir = path.join(root, sid, 'replays', runId);
  fs.mkdirSync(runDir, { recursive: true });
  fs.writeFileSync(
    path.join(runDir, 'audit.json'),
    JSON.stringify({
      schema: 'scenario-replay-audit/v1',
      runId,
      scenarioId: sid,
      startedAt: '2026-08-13T14:44:34.767Z',
      profile: 'admin-user',
    }),
  );

  const out = srv._test.finalizeIncompleteReplay(root, sid, new Set(), {
    code: null,
    signal: 'SIGTERM',
  });

  assert.equal(out.runId, runId);
  const audit = JSON.parse(fs.readFileSync(path.join(runDir, 'audit.json'), 'utf8'));
  assert.equal(audit.exitCode, 1);
  assert.match(audit.summary, /FAIL.*SIGTERM/);
  assert.ok(audit.finishedAt);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(runDir, 'status.json'), 'utf8')), {
    state: 'done',
    currentIdx: 0,
    total: 0,
    ok: false,
  });
  assert.equal(fs.readFileSync(path.join(root, sid, 'replays', 'latest.txt'), 'utf8'), runId + '\n');
});

test('setup watchdog terminates a live child that never reports progress', async (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-replay-watchdog-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const sid = 's-watchdog-replay';
  const runId = '2026-08-13T15-00-00-000Z__feedface__admin-user';
  const signals = [];

  const spawnImpl = () => {
    const runDir = path.join(root, sid, 'replays', runId);
    fs.mkdirSync(runDir, { recursive: true });
    fs.writeFileSync(
      path.join(runDir, 'audit.json'),
      JSON.stringify({
        schema: 'scenario-replay-audit/v1',
        runId,
        scenarioId: sid,
        startedAt: '2026-08-13T15:00:00.000Z',
      }),
    );
    const child = new EventEmitter();
    child.pid = 4242;
    child.stdout = new EventEmitter();
    child.stderr = new EventEmitter();
    child.unref = () => {};
    child.kill = (signal) => {
      signals.push(signal);
      setImmediate(() => child.emit('exit', null, signal));
      return true;
    };
    setImmediate(() => child.emit('spawn'));
    return child;
  };

  const replay = srv._test.makeReplaySpawner({
    bin: '/fake/agent-qa',
    env: {},
    cwd: root,
    root,
    spawnImpl,
    setupTimeoutMs: 10,
  });
  const started = await replay(sid, 'admin-user-session', { profile: 'admin-user' });
  assert.equal(started.ok, true);
  const exit = await started.done;

  assert.deepEqual(signals, ['SIGTERM']);
  assert.equal(exit.signal, 'SIGTERM');
  assert.equal(exit.finalizedRunId, runId);
  const audit = JSON.parse(
    fs.readFileSync(path.join(root, sid, 'replays', runId, 'audit.json'), 'utf8'),
  );
  assert.match(audit.summary, /FAIL.*SIGTERM/);
  assert.equal(audit.exitCode, 1);
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
    // do→check coverage: navHome covered by headingVisible; clickMissingLogin
    // bare. headingVisible's domshot claim counts toward golden (not shot).
    assert.deepEqual(sc.coverage, {
      doSteps: 2,
      checked: 1,
      bare: 1,
      ratio: 0.5,
      shotCovered: 0,
      shotRatio: 0,
      goldenCovered: 1,
      goldenRatio: 0.5,
    });
  });

  await t.test('GET /api/health passes through audit health --json', async () => {
    const seen = [];
    const { server: hSrv, base: hBase } = await boot(fx.root, {
      runCli: async (args) => {
        seen.push(args.join(' '));
        return {
          stdout:
            JSON.stringify([
              { scenarioId: fx.sid, flaky: ['s1'], slow: [], chronic: ['s2'] },
            ]) + '\n',
        };
      },
    });
    try {
      const res = await fetch(`${hBase}/api/health`);
      assert.equal(res.status, 200);
      const body = await res.json();
      assert.deepEqual(seen, ['audit health --json']);
      assert.equal(body.health.length, 1);
      assert.equal(body.health[0].scenarioId, fx.sid);
      assert.deepEqual(body.health[0].flaky, ['s1']);
      assert.deepEqual(body.health[0].chronic, ['s2']);
    } finally {
      hSrv.close();
    }
  });

  await t.test('GET /api/health degrades to [] when the CLI is absent or errors', async () => {
    const { server: nSrv, base: nBase } = await boot(fx.root);
    try {
      const res = await fetch(`${nBase}/api/health`);
      assert.equal(res.status, 200);
      assert.deepEqual((await res.json()).health, []);
    } finally {
      nSrv.close();
    }
    const { server: eSrv, base: eBase } = await boot(fx.root, {
      runCli: async () => {
        throw new Error('cli exploded');
      },
    });
    try {
      const res = await fetch(`${eBase}/api/health`);
      assert.equal(res.status, 200);
      assert.deepEqual((await res.json()).health, []);
    } finally {
      eSrv.close();
    }
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

  await t.test('GET /runs/:runId joins heal.jsonl rows with their patches', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}`);
    const body = await res.json();
    assert.equal(body.heals.length, 2);
    const [correction, rejection] = body.heals;
    assert.equal(correction.stepId, 'navHome');
    assert.equal(correction.mode, 'locator-correction');
    assert.equal(correction.strategy, 'digits-tolerant');
    assert.equal(correction.patch.schema, 'heal-patch/v1');
    assert.equal(correction.patch.newLocator.name, 'Users 1');
    assert.equal(rejection.stepId, 'clickMissingLogin');
    assert.equal(rejection.mode, 'value-rejection');
    assert.equal(rejection.patch, null);
  });

  await t.test('GET /runs surfaces the healed count from audit.autoHealed', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs`);
    const body = await res.json();
    assert.equal(body.replays[0].healed, 1); // audit.autoHealed lists navHome
  });

  await t.test('GET /runs/:runId lists stepIds with a shot diff map', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}`);
    const body = await res.json();
    assert.deepEqual(body.shotDiffs, ['navHome']);
  });

  await t.test('GET /runs/:runId reports video:true when run.webm exists', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}`);
    const body = await res.json();
    assert.equal(body.video, true);
  });

  await t.test('GET run file streams run.webm as video/webm', async () => {
    const res = await fetch(
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/file/run.webm`,
    );
    assert.equal(res.status, 200);
    assert.equal(res.headers.get('content-type'), 'video/webm');
    assert.equal(await res.text(), 'WEBMDATA');
  });

  await t.test('GET run file rejects names outside the allowlist + traversal', async () => {
    for (const u of [
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/file/events.jsonl`,
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/file/audit.json`,
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/file/..%2fscenario.json`,
    ]) {
      const res = await fetch(u);
      assert.ok(res.status === 400 || res.status === 404, `expected 4xx for ${u}, got ${res.status}`);
    }
  });

  await t.test('GET artifact streams a shot diff map as png', async () => {
    const res = await fetch(
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/shots-diff/navHome`,
    );
    assert.equal(res.status, 200);
    assert.equal(res.headers.get('content-type'), 'image/png');
    assert.equal(await res.text(), 'DIFFDATA-navHome');
  });

  await t.test('POST shot-accept promotes the run screenshot to a baseline', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/shot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ stepId: 'navHome' }),
    });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.ok, true);
    assert.deepEqual(body.minted, ['navHome']);
    assert.equal(
      fs.readFileSync(path.join(fx.root, fx.sid, 'baselines', 'navHome.png'), 'utf8'),
      'PNGDATA-navHome',
    );
  });

  await t.test('POST shot-accept {all:true} promotes every run screenshot', async () => {
    // Seed a second screenshot so all-mode has more than one mint candidate.
    const extra = path.join(fx.root, fx.sid, 'replays', fx.runId, 'screenshots', 'aSecond.png');
    fs.writeFileSync(extra, Buffer.from('PNGDATA-aSecond'));
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/shot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ all: true }),
    });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.ok, true);
    assert.deepEqual(body.minted, ['aSecond', 'navHome']);
    assert.equal(
      fs.readFileSync(path.join(fx.root, fx.sid, 'baselines', 'aSecond.png'), 'utf8'),
      'PNGDATA-aSecond',
    );
  });

  await t.test('POST shot-accept {all:true} 404s when the run has no screenshots', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/noShots/shot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ all: true }),
    });
    assert.equal(res.status, 404);
  });

  await t.test('POST shot-accept rejects a missing stepId / absent screenshot', async () => {
    const res1 = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/shot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({}),
    });
    assert.equal(res1.status, 400);
    const res2 = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/shot-accept`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ stepId: 'nope' }),
    });
    assert.equal(res2.status, 404);
  });

  await t.test('GET /runs/:runId lists stepIds with a domshot diff', async () => {
    const res = await fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}`);
    const body = await res.json();
    assert.deepEqual(body.domshotDiffs, ['navHome']);
  });

  await t.test('GET artifact streams a domshot diff as plain text', async () => {
    const res = await fetch(
      `${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/domshots-diff/navHome`,
    );
    assert.equal(res.status, 200);
    assert.match(res.headers.get('content-type') || '', /text\/plain/);
    assert.match(await res.text(), /heading "New"/);
  });

  await t.test('POST domshot-accept runs the CLI verb with --steps', async () => {
    const calls = [];
    const { server: dSrv, base: dBase } = await boot(fx.root, {
      runCli: async (args) => {
        calls.push(args);
        return { code: 0, stdout: '["navHome"]\n', stderr: '' };
      },
    });
    try {
      const res = await fetch(`${dBase}/api/scenarios/${fx.sid}/runs/${fx.runId}/domshot-accept`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ stepId: 'navHome' }),
      });
      assert.equal(res.status, 200);
      const body = await res.json();
      assert.equal(body.ok, true);
      assert.deepEqual(body.minted, ['navHome']);
      assert.deepEqual(calls, [
        ['domshot-accept', fx.sid, '--run', fx.runId, '--json', '--steps', 'navHome'],
      ]);

      // {all:true} mints every step a domshot claim references (no --steps).
      const res2 = await fetch(`${dBase}/api/scenarios/${fx.sid}/runs/${fx.runId}/domshot-accept`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ all: true }),
      });
      assert.equal(res2.status, 200);
      assert.deepEqual(calls[1], ['domshot-accept', fx.sid, '--run', fx.runId, '--json']);
    } finally {
      dSrv.close();
    }
  });

  await t.test('POST domshot-accept surfaces a non-zero CLI exit as 422', async () => {
    const { server: dSrv, base: dBase } = await boot(fx.root, {
      runCli: async () => ({ code: 1, stdout: '', stderr: 'no domshot claims' }),
    });
    try {
      const res = await fetch(`${dBase}/api/scenarios/${fx.sid}/runs/${fx.runId}/domshot-accept`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ all: true }),
      });
      assert.equal(res.status, 422);
      assert.match((await res.json()).error, /no domshot claims/);
    } finally {
      dSrv.close();
    }
  });

  await t.test('POST /api/scenarios/crawl spawns the crawl verb with flags', async () => {
    const calls = [];
    const { server: cSrv, base: cBase } = await boot(fx.root, {
      runCli: async (args) => {
        calls.push(args);
        return { code: 0, stdout: 'crawl: crawl-app — 3 route(s)\n', stderr: '' };
      },
    });
    try {
      const res = await fetch(`${cBase}/api/scenarios/crawl`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ url: 'https://app.example.com/', sid: 'crawl-app', max: 5 }),
      });
      assert.equal(res.status, 200);
      const j = await res.json();
      assert.equal(j.ok, true);
      assert.match(j.stdout, /3 route/);
      assert.deepEqual(calls, [['crawl', 'https://app.example.com/', '--sid', 'crawl-app', '--max', '5']]);
      const bad = await fetch(`${cBase}/api/scenarios/crawl`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ url: 'notaurl' }),
      });
      assert.equal(bad.status, 400);
    } finally {
      cSrv.close();
    }
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

  await t.test('artifact whose file becomes unreadable fails fast instead of hanging', async () => {
    // stat() succeeds on a mode-000 file (metadata needs only the parent
    // dir), then the open fails — before streamFile the socket stayed open
    // and the client waited forever on the promised content-length.
    const victim = path.join(fx.root, fx.sid, 'replays', fx.runId, 'screenshots', 'locked.png');
    fs.writeFileSync(victim, 'PNGDATA-locked');
    fs.chmodSync(victim, 0o000);
    try {
      await assert.rejects(
        fetch(`${base}/api/scenarios/${fx.sid}/runs/${fx.runId}/artifact/screenshots/locked`),
      );
    } finally {
      fs.chmodSync(victim, 0o644);
    }
  });

  await t.test('static index.html (React runs entry) is served', async () => {
    const res = await fetch(`${base}/`);
    assert.equal(res.status, 200);
    assert.match(res.headers.get('content-type'), /text\/html/);
    const html = await res.text();
    assert.match(html, /<title>agent-qa<\/title>/);
    assert.match(html, /\/assets\/[A-Za-z0-9._-]+\.js/);
  });

  await t.test('POST /compare shells the CLI and returns the parsed report', async () => {
    // The route runs `agent-qa compare <sid> <a> <b>` then reads back the
    // newest <sid>/compare/<ts>__<a>-vs-<b>/ dir. Stub the CLI, pre-write the
    // report dir it would have produced.
    const cdir = path.join(fx.root, fx.sid, 'compare', '2099-01-01T00-00-00-000Z__a1-vs-b2');
    fs.mkdirSync(path.join(cdir, 'snapshots'), { recursive: true });
    fs.mkdirSync(path.join(cdir, 'screenshots'), { recursive: true });
    fs.writeFileSync(
      path.join(cdir, 'compare.md'),
      [
        '# compare runA1 vs runB2',
        '',
        '## snapshots',
        '',
        '| step | outcome |',
        '|---|---|',
        '| navHome | SAME |',
        '| headingVisible | CHANGED |',
        '| extraStep | ONLY-B |',
        '',
        '## screenshots',
        '',
        '| step | outcome | differing pixels |',
        '|---|---|---|',
        '| navHome | SAME | - |',
        '| headingVisible | CHANGED | 0.0312 |',
        '',
      ].join('\n'),
    );
    fs.writeFileSync(
      path.join(cdir, 'snapshots', 'headingVisible.diff'),
      '--- a\n+++ b\n@@ -1 +1 @@\n- old\n+ new\n',
    );
    fs.writeFileSync(path.join(cdir, 'screenshots', 'headingVisible.diff.png'), 'PNGX');
    let failNext = false;
    const calls = [];
    const { server: cserver, base: cbase } = await boot(fx.root, {
      runCli: async (args) => {
        calls.push(args);
        return failNext ? { code: 1, stdout: '', stderr: 'boom' } : { code: 0, stdout: '', stderr: '' };
      },
    });
    try {
      const res = await fetch(`${cbase}/api/scenarios/${fx.sid}/compare`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ runA: 'runA1', runB: 'runB2' }),
      });
      assert.equal(res.status, 200);
      const j = await res.json();
      assert.deepEqual(calls, [['compare', fx.sid, 'runA1', 'runB2']]);
      assert.equal(j.runA, 'runA1');
      assert.equal(j.runB, 'runB2');
      assert.equal(j.snapshots.length, 3);
      assert.equal(j.snapshots[0].outcome, 'SAME');
      assert.equal(j.snapshots[0].diff, undefined);
      assert.equal(j.snapshots[1].outcome, 'CHANGED');
      assert.match(j.snapshots[1].diff, /\+ new/);
      assert.equal(j.snapshots[2].outcome, 'ONLY-B');
      assert.equal(j.screenshots.length, 2);
      assert.equal(j.screenshots[1].outcome, 'CHANGED');
      assert.equal(j.screenshots[1].differingPixels, 0.0312);
      assert.equal(j.screenshots[1].hasDiffPng, true);
      // The pixel-diff png is served back via the shots route.
      const shot = await fetch(`${cbase}/api/scenarios/${fx.sid}/compare/${j.folder}/shots/headingVisible`);
      assert.equal(shot.status, 200);
      assert.equal(shot.headers.get('content-type'), 'image/png');
      assert.equal(await shot.text(), 'PNGX');
      const miss = await fetch(`${cbase}/api/scenarios/${fx.sid}/compare/${j.folder}/shots/nope`);
      assert.equal(miss.status, 404);
      // A nonzero CLI exit surfaces as 422 with the stderr text.
      failNext = true;
      const bad = await fetch(`${cbase}/api/scenarios/${fx.sid}/compare`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: '{}',
      });
      assert.equal(bad.status, 422);
      assert.match((await bad.json()).error, /boom/);
    } finally {
      cserver.close();
    }
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

test('POST /replay threads headed mode through session preparation and deps.replay', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const prepared = [];
  const deps = {
    prepareBrowserSession: async (session, headed) => prepared.push({ session, headed }),
    replay: async (sid, session, opts) => {
      calls.push({ sid, session, opts });
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
  // screencast can attach to exactly that browser. Empty input is headless.
  assert.deepEqual(prepared, [{ session: `replay-${fx.sid}`, headed: false }]);
  assert.deepEqual(calls, [
    {
      sid: fx.sid,
      session: `replay-${fx.sid}`,
      opts: { headed: false, env: {} },
    },
  ]);

  const headed = await fetch(`${base}/api/scenarios/${fx.sid}/replay`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ headed: true }),
  });
  assert.equal(headed.status, 202);
  assert.deepEqual(prepared[1], { session: `replay-${fx.sid}`, headed: true });
  assert.equal(calls[1].opts.headed, true);
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

test('GET /replay-stream prefers audit.sessionName over the derived session', async (t) => {
  const fx = makeFixture();
  const seen = [];
  const fakeBridge = {
    subscribe: (res) => res.write('data: {"data":"AAAA"}\n\n'),
    unsubscribe: () => {},
  };
  const { server, base } = await boot(fx.root, {
    liveForSession: (session) => (seen.push(session), fakeBridge),
  });
  t.after(() => server.close());

  const auditPath = path.join(fx.root, fx.sid, 'replays', fx.runId, 'audit.json');
  // The runner recorded the session it actually drove — that wins even when a
  // profile-derived name would point at a different browser.
  fs.writeFileSync(
    auditPath,
    JSON.stringify({ schema: 'scenario-replay-audit/v1', profile: 'admin-user', sessionName: 'chat-c0ffee11' }),
  );
  const ac = new AbortController();
  assert.equal((await fetch(`${base}/api/scenarios/${fx.sid}/replay-stream`, { signal: ac.signal })).status, 200);
  ac.abort();

  // No recorded session → profile-derived fallback still applies.
  fs.writeFileSync(
    auditPath,
    JSON.stringify({ schema: 'scenario-replay-audit/v1', profile: 'admin-user' }),
  );
  const ac2 = new AbortController();
  assert.equal((await fetch(`${base}/api/scenarios/${fx.sid}/replay-stream`, { signal: ac2.signal })).status, 200);
  ac2.abort();

  assert.deepEqual(seen, ['chat-c0ffee11', 'admin-user-session']);
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

test('run dir with no status.json resolves stale vs running by freshness', async (t) => {
  // A replay that minted its dir then died before writing status.json used
  // to report state null → the UI showed "in flight" (and the "signing in"
  // banner) forever. Now: nothing written within the staleness window →
  // 'stale' (interrupted); a fresh dir may still be starting → 'running'.
  const fx = makeFixture();
  const deadRun = '2026-06-20T10-00-00-000Z__deadbeef';
  const deadDir = path.join(fx.root, fx.sid, 'replays', deadRun);
  fs.mkdirSync(deadDir, { recursive: true });
  fs.writeFileSync(
    path.join(deadDir, 'audit.json'),
    JSON.stringify({ schema: 'scenario-replay-audit/v1', runId: deadRun, startedAt: '2026-06-20T10:00:00.000Z' }),
  );
  // Backdate every artifact + the dir itself past the staleness window.
  const old = new Date(Date.now() - 60 * 60 * 1000);
  fs.utimesSync(path.join(deadDir, 'audit.json'), old, old);
  fs.utimesSync(deadDir, old, old);

  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  const runs = (await (await fetch(`${base}/api/scenarios/${fx.sid}/runs`)).json()).replays;
  const deadRow = runs.find((r) => r.runId === deadRun);
  assert.equal(deadRow.state, 'stale');

  const detail = await (await fetch(`${base}/api/scenarios/${fx.sid}/runs/${deadRun}`)).json();
  assert.equal(detail.status && detail.status.state, 'stale');

  // Same shape, fresh — a run that just started and hasn't written
  // status.json yet must still read 'running'.
  const freshRun = '2026-06-24T16-30-00-000Z__cafebabe';
  fs.mkdirSync(path.join(fx.root, fx.sid, 'replays', freshRun), { recursive: true });
  const runs2 = (await (await fetch(`${base}/api/scenarios/${fx.sid}/runs`)).json()).replays;
  assert.equal(runs2.find((r) => r.runId === freshRun).state, 'running');
});

test('the recorded/ capture sidecar never lists as a run', async (t) => {
  // flush writes replays/recorded/network.har — a HAR sidecar, not a run.
  // Unfiltered, it walked the same "fresh dir → running" path as a live run
  // and showed a phantom "Signing in…" row forever.
  const fx = makeFixture();
  const recDir = path.join(fx.root, fx.sid, 'replays', 'recorded');
  fs.mkdirSync(recDir, { recursive: true });
  fs.writeFileSync(path.join(recDir, 'network.har'), '{}');

  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  const runs = (await (await fetch(`${base}/api/scenarios/${fx.sid}/runs`)).json()).replays;
  assert.ok(!runs.some((r) => r.runId === 'recorded'), 'recorded/ must not list as a run');

  const sc = (await (await fetch(`${base}/api/scenarios`)).json()).scenarios[0];
  assert.notEqual(sc.activeRunId, 'recorded', 'recorded/ must not count as the active run');
});

test('GET /api/extension.zip downloads the bundled extension sources', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());

  const res = await fetch(`${base}/api/extension.zip`);
  // In a repo checkout the sources resolve from <repo>/extension/.
  assert.equal(res.status, 200);
  assert.equal(res.headers.get('content-type'), 'application/zip');
  assert.match(res.headers.get('content-disposition') || '', /agent-qa-extension\.zip/);
  const buf = Buffer.from(await res.arrayBuffer());
  assert.ok(buf.length > 500);
  assert.deepEqual(buf.subarray(0, 4), Buffer.from('PK\x03\x04'));
  // Central directory names carry the extension/ prefix; manifest is required for MV3.
  assert.ok(buf.includes('extension/manifest.json'), 'zip contains extension/manifest.json');
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
  const prepared = [];
  const deps = {
    prepareBrowserSession: async (session, headed) => prepared.push({ session, headed }),
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
    headed: true,
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
      opts: {
        headed: true,
        profile: 'qa-admin',
        params: { baseUrl: 'https://staging.example.com' },
        env: {},
      },
    },
  ]);
  assert.deepEqual(prepared, [{ session: 'qa-admin-session', headed: true }]);
});

test('POST /api/plans/:id/run serializes members and writes a last-run ledger', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const dones = [];
  const deps = {
    prepareBrowserSession: async () => {},
    replay: async (sid, session) => {
      calls.push({ sid, session });
      // Mint a run dir like a real child would (isRunDir just needs a safe
      // dir name under replays/) and hand back a controllable done promise.
      fs.mkdirSync(path.join(fx.root, sid, 'replays', `run-${calls.length}`), {
        recursive: true,
      });
      dones.push({});
      return {
        ok: true,
        done: new Promise((r) => {
          dones[dones.length - 1].resolve = r;
        }),
      };
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

  await j('POST', '/api/cases/m1', { title: 'Member one' });
  await j('POST', '/api/cases/m1/link', { scenarioSid: fx.sid });
  await j('POST', '/api/cases/m2', { title: 'Member two' });
  await j('POST', '/api/cases/m2/link', { scenarioSid: fx.sid });
  await j('POST', '/api/plans/p9', { name: 'P9', scope: { caseIds: ['m1', 'm2'] } });

  const res = await j('POST', '/api/plans/p9/run', { profile: 'qa-admin' });
  assert.equal(res.status, 202);
  const out = await res.json();
  assert.deepEqual(out.started, [
    { caseId: 'm1', sid: fx.sid },
    { caseId: 'm2', sid: fx.sid },
  ]);

  // Member 1 spawned in-request; member 2 must wait for its child to exit.
  assert.equal(calls.length, 1);
  await new Promise((r) => setTimeout(r, 30));
  assert.equal(calls.length, 1, 'member 2 spawned while member 1 was still running');
  dones[0].resolve({ code: 0 });
  await new Promise((r) => setTimeout(r, 60));
  assert.equal(calls.length, 2, 'member 2 did not start after member 1 exited');
  dones[1].resolve({ code: 0 });
  await new Promise((r) => setTimeout(r, 60));
  const ledger = await (await j('GET', '/api/plans/p9/last-run')).json();
  const statuses = ledger.rows.map((r) => `${r.caseId}:${r.status}`);
  assert.deepEqual(statuses, ['m1:running', 'm1:pass', 'm2:running', 'm2:pass']);
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
  const prepared = [];
  const deps = {
    prepareBrowserSession: async (session, headed) => prepared.push({ session, headed }),
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

  const res = await j('POST', '/api/personas/admin/connect', {
    environmentId: 'staging',
    headed: true,
  });
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
  assert.deepEqual(calls[1].args, ['profile-bootstrap', 'admin-user', '--headed']);
  assert.deepEqual(prepared, [{ session: 'admin-user-session', headed: true }]);
  assert.equal(calls[1].extraEnv.AGENT_QA_ENV_BASE_URL, 'https://s.example.com');
  assert.equal(calls[1].extraEnv.AGENT_QA_ENV_TENANT, '7');

  // no auth plugin (no registry entry + no env adapter) → 400
  await j('POST', '/api/environments/noplug', { name: 'NoPlug' });
  assert.equal((await j('POST', '/api/personas/admin/connect', { environmentId: 'noplug' })).status, 400);
});

test("POST /api/chat/c/:id/connect bootstraps auth into THAT chat's own session", async (t) => {
  const fx = makeFixture();
  const calls = [];
  const prepared = [];
  const deps = {
    chat: { hub: {} }, // chat available so a chat can be created
    prepareBrowserSession: async (session, headed) => prepared.push({ session, headed }),
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

  // Create the chat before any persona/environment exists so chat-create's
  // background auto-connect is a no-op — only the explicit connect bootstraps.
  const created = await (await j('POST', '/api/chat/create')).json();
  assert.match(created.session, /^chat-[0-9a-f]+$/);

  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  await j('POST', '/api/environments/staging', { name: 'Staging', auth: { plugin: 'agent-qa-plugin-acme' } });

  const res = await j('POST', `/api/chat/c/${created.id}/connect`, {
    personaId: 'admin',
    environmentId: 'staging',
    headed: true,
  });
  assert.equal(res.status, 200);
  const out = await res.json();
  assert.equal(out.authenticated, true);
  assert.equal(out.profile, 'admin-user');
  // The cookie must land in THIS chat's browser session, not the per-profile
  // default — so bootstrap + status carry --session <chat session>.
  assert.equal(out.session, created.session);
  const bootCall = calls.find((a) => a[0] === 'profile-bootstrap');
  const statCall = calls.find((a) => a[0] === 'profile-status');
  assert.deepEqual(bootCall, [
    'profile-bootstrap',
    'admin-user',
    '--session',
    created.session,
    '--headed',
  ]);
  assert.deepEqual(statCall, ['profile-status', 'admin-user', '--session', created.session]);
  assert.deepEqual(prepared, [{ session: created.session, headed: true }]);
  const connection = await (await j('GET', `/api/chat/c/${created.id}/connection`)).json();
  assert.deepEqual(connection, {
    state: 'connected',
    personaId: 'admin',
    environmentId: 'staging',
    profile: 'admin-user',
  });

  // personaId is required
  assert.equal((await j('POST', `/api/chat/c/${created.id}/connect`, {})).status, 400);
});

test('reportedAuthenticated reads the status line, not the JSON body', () => {
  // The printed JSON response can contain the word "authenticated" inside a
  // message — that must not count.
  assert.equal(
    srv._test.reportedAuthenticated(
      'profile admin-user → expired\n{\n  "status": "expired",\n  "detail": "token authenticated at 10:00 has expired"\n}',
    ),
    false,
  );
  assert.equal(
    srv._test.reportedAuthenticated('profile admin-user → authenticated\n{\n  "status": "authenticated"\n}'),
    true,
  );
  // Bare CLI line without a JSON body (e.g. --quiet style or older builds).
  assert.equal(srv._test.reportedAuthenticated('admin-user: authenticated'), true);
  assert.equal(srv._test.reportedAuthenticated('admin-user: signed out'), false);
});

test('connectFailureDetail distills the failing step for the UI', () => {
  const detail = srv._test.connectFailureDetail([
    { step: 'profile-add', code: 0, stdout: 'ok', stderr: '', spawnError: null },
    {
      step: 'profile-status',
      code: 1,
      stdout: 'profile admin-user → on-login\n{\n  "status": "on-login",\n  "message": "credentials rejected"\n}',
      stderr: '',
      spawnError: null,
    },
  ]);
  assert.match(detail, /on-login/);
  assert.match(detail, /credentials rejected/);

  // No parseable plugin output → fall back to stderr.
  assert.match(
    srv._test.connectFailureDetail([
      { step: 'profile-bootstrap', code: 1, stdout: '', stderr: 'auth-failed: EMAIL unset', spawnError: null },
    ]),
    /auth-failed: EMAIL unset/,
  );
  assert.equal(srv._test.connectFailureDetail([{ step: 'x', code: 0, stdout: '', stderr: '' }]), '');
});

test('connect failure returns a human-readable detail (and the no-credentials hint)', async (t) => {
  const fx = makeFixture();
  const deps = {
    chat: { hub: {} },
    runCli: async (args) => {
      if (args[0] === 'profile-status') {
        return {
          code: 1,
          stdout: 'profile admin-user → on-login\n{\n  "status": "on-login",\n  "message": "still on the login page"\n}',
          stderr: '',
        };
      }
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const booted = await boot(fx.root, deps);
  t.after(() => booted.server.close());
  const j = (m, p, b) =>
    fetch(`${booted.base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });
  const created = await (await j('POST', '/api/chat/create')).json();
  // Persona with NO credential entries → the hint names the likely cause.
  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  await j('POST', '/api/environments/staging', { name: 'Staging', auth: { plugin: 'agent-qa-plugin-acme' } });

  const out = await (
    await j('POST', `/api/chat/c/${created.id}/connect`, { personaId: 'admin', environmentId: 'staging' })
  ).json();
  assert.equal(out.authenticated, false);
  assert.match(out.detail, /status "on-login"/);
  assert.match(out.detail, /still on the login page/);
  assert.match(out.detail, /no credential entries configured/);
  // The failed auto-connect never ran (chat was created before the persona
  // existed), so the poll endpoint stays disconnected.
  const connection = await (await j('GET', `/api/chat/c/${created.id}/connection`)).json();
  assert.equal(connection.state, 'disconnected');
});

test("POST /api/chat/c/:id/prompt annotates the text with the pane's current page", async (t) => {
  const fx = makeFixture();
  const prompts = [];
  const bridge = { currentUrl: 'https://example.com/docs' };
  const { server, base } = await boot(fx.root, {
    chat: { hub: { prompt: async (text) => prompts.push(text) } },
    liveForSession: () => bridge,
  });
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: b ? JSON.stringify(b) : undefined,
    });
  const chat = await (await j('POST', '/api/chat/create')).json();

  const res = await j('POST', `/api/chat/c/${chat.id}/prompt`, { text: 'record this page' });
  assert.equal(res.status, 202);
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(prompts.length, 1);
  assert.match(prompts[0], /browser pane is currently on https:\/\/example\.com\/docs/);
  assert.match(prompts[0], /record this page$/);

  // No tracked page → only the sign-in note rides along; the text still ends it.
  bridge.currentUrl = null;
  await j('POST', `/api/chat/c/${chat.id}/prompt`, { text: 'record this page' });
  await new Promise((r) => setTimeout(r, 20));
  assert.match(prompts[1], /browsing anonymously/);
  assert.match(prompts[1], /record this page$/);
});

test('prompt annotation reports a connected persona instead of anonymous', async (t) => {
  const fx = makeFixture();
  const prompts = [];
  const { server, base } = await boot(fx.root, {
    chat: { hub: { prompt: async (text) => prompts.push(text) } },
    runCli: async (args) =>
      args[0] === 'profile-status'
        ? { code: 0, stdout: 'authenticated', stderr: '' }
        : { code: 0, stdout: 'ok', stderr: '' },
  });
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: b ? JSON.stringify(b) : undefined,
    });
  const chat = await (await j('POST', '/api/chat/create')).json();
  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  await j('POST', '/api/environments/staging', { name: 'Staging' });
  const conn = await j('POST', `/api/chat/c/${chat.id}/connect`, {
    personaId: 'admin',
    environmentId: 'staging',
  });
  assert.equal((await conn.json()).authenticated, true);

  await j('POST', `/api/chat/c/${chat.id}/prompt`, { text: 'hi' });
  await new Promise((r) => setTimeout(r, 20));
  const last = prompts[prompts.length - 1];
  assert.match(last, /signed in as the "admin-user" persona/);
});

test("GET /api/chat/c/:id/info reports the chat's agent/browser/plugin wiring", async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root, {
    chat: {
      hub: {
        getState: async () => ({
          backend: 'pi',
          model: { provider: 'anthropic', id: 'claude-x', label: 'Claude X' },
          sessionId: 'sess-1',
          thinkingLevel: 'off',
        }),
      },
    },
    runCli: async () => ({ code: 0, stdout: 'ok', stderr: '' }),
  });
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: b ? JSON.stringify(b) : undefined,
    });
  const chat = await (await j('POST', '/api/chat/create')).json();

  const res = await j('GET', `/api/chat/c/${chat.id}/info`);
  assert.equal(res.status, 200);
  const info = await res.json();
  assert.equal(info.chatId, chat.id);
  assert.equal(info.backend, 'pi');
  assert.equal(info.model.id, 'claude-x');
  assert.equal(info.sessionId, 'sess-1');
  assert.equal(info.browserSession, chat.session);
  assert.equal(info.scenariosRoot, fx.root);
  assert.equal(info.jev.enabled, false);
  assert.equal(info.jev.hasKey, false);
  assert.deepEqual(info.plugins, []);
  assert.equal(info.connected, null);
});

test('GET /api/chat/c/:id/info lists a bundled jev plugin when enabled', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root, {
    chat: { hub: {} }, // no getState — info still answers
    runCli: async () => ({ code: 0, stdout: 'ok', stderr: '' }),
  });
  t.after(() => server.close());
  await fs.promises.mkdir(`${fx.root}/_config`, { recursive: true });
  await fs.promises.writeFile(
    `${fx.root}/_config/jev.json`,
    JSON.stringify({ schema: 'jev/1', enabled: true, apiKey: 'k' })
  );
  const j = (m, p, b) =>
    fetch(`${base}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: b ? JSON.stringify(b) : undefined,
    });
  const chat = await (await j('POST', '/api/chat/create')).json();
  const info = await (await j('GET', `/api/chat/c/${chat.id}/info`)).json();
  assert.equal(info.backend, null);
  assert.equal(info.jev.enabled, true);
  assert.equal(info.jev.hasKey, true);
  const bundled = info.plugins.find((p) => p.source === 'bundled');
  assert.equal(bundled.name, 'jev-resolve');
});

test('chats persist under <root>/_chats and restore with the same session name', async (t) => {
  const fx = makeFixture();
  const deps = {
    chat: { hub: {} },
    runCli: async () => ({ code: 0, stdout: 'ok', stderr: '' }),
  };
  const mk = (b) => (m, p, body) =>
    fetch(`${b}${p}`, {
      method: m,
      headers: { 'content-type': 'application/json' },
      body: body ? JSON.stringify(body) : undefined,
    });

  const first = await boot(fx.root, deps);
  const chat = await (await mk(first.base)('POST', '/api/chat/create')).json();

  const recPath = path.join(fx.root, '_chats', `${chat.id}.json`);
  assert.ok(fs.existsSync(recPath));
  const rec = JSON.parse(fs.readFileSync(recPath, 'utf8'));
  assert.equal(rec.schema, 'chat/1');
  assert.equal(rec.id, chat.id);
  assert.equal(rec.browserSession, chat.session);

  // Restart against the same root — the chat list and its bound browser
  // session name come back, so recordDir/agent-session stay addressable.
  await new Promise((r) => first.server.close(r));
  const second = await boot(fx.root, deps);
  t.after(() => second.server.close());
  const j = mk(second.base);

  const list = await (await j('GET', '/api/chat/list')).json();
  const restored = list.chats.find((c) => c.id === chat.id);
  assert.ok(restored, 'restored chat in list');
  assert.equal(restored.session, chat.session);
  // Info reflects the restored binding.
  const info = await (await j('GET', `/api/chat/c/${chat.id}/info`)).json();
  assert.equal(info.browserSession, chat.session);

  // Disconnect persists the guest flag across a further restart.
  await j('POST', `/api/chat/c/${chat.id}/disconnect`);
  assert.equal(JSON.parse(fs.readFileSync(recPath, 'utf8')).guest, true);

  // Delete removes the record.
  await j('POST', `/api/chat/c/${chat.id}/delete`);
  assert.equal(fs.existsSync(recPath), false);
});

test('POST /api/chat/c/:id/disconnect drops the persona binding into guest mode', async (t) => {
  const fx = makeFixture();
  const deps = {
    chat: { hub: {} },
    prepareBrowserSession: async () => {},
    runCli: async (args) => {
      if (args[0] === 'profile-status') return { code: 0, stdout: 'admin-user: authenticated', stderr: '' };
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const booted = await boot(fx.root, deps);
  t.after(() => booted.server.close());
  const j = (m, p, b) =>
    fetch(`${booted.base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  const created = await (await j('POST', '/api/chat/create')).json();
  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  await j('POST', '/api/environments/staging', { name: 'Staging', auth: { plugin: 'agent-qa-plugin-acme' } });
  await j('POST', `/api/chat/c/${created.id}/connect`, { personaId: 'admin', environmentId: 'staging' });
  assert.equal((await (await j('GET', `/api/chat/c/${created.id}/connection`)).json()).state, 'connected');

  const out = await (await j('POST', `/api/chat/c/${created.id}/disconnect`)).json();
  assert.deepEqual(out, { state: 'disconnected', guest: true });
  const connection = await (await j('GET', `/api/chat/c/${created.id}/connection`)).json();
  assert.deepEqual(connection, {
    state: 'disconnected',
    personaId: null,
    environmentId: null,
    profile: null,
    guest: true,
  });

  // A later connect still works and clears guest.
  await j('POST', `/api/chat/c/${created.id}/connect`, { personaId: 'admin', environmentId: 'staging' });
  const again = await (await j('GET', `/api/chat/c/${created.id}/connection`)).json();
  assert.equal(again.state, 'connected');
  assert.equal(again.guest, undefined);
});

test('POST /api/chat/c/:id/replay re-auths via the connected persona, in its session', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const prepared = [];
  const deps = {
    chat: { hub: {} },
    recordRoot: path.join(fx.root, 'rec'),
    prepareBrowserSession: async (session, headed) => prepared.push({ session, headed }),
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
  const res = await j('POST', `/api/chat/c/${created.id}/replay`, {
    sid: 's-2026',
    headed: true,
  });
  assert.equal(res.status, 200);
  assert.equal((await res.json()).ok, true);

  const replay = calls.find((c) => c.args[0] === 'replay');
  // Runs in the chat's own (already-authed) session, under the connected profile.
  assert.deepEqual(replay.args, [
    'replay',
    's-2026',
    '--session',
    created.session,
    '--headed',
    '--profile',
    'admin-user',
  ]);
  assert.deepEqual(prepared, [
    { session: created.session, headed: false },
    { session: created.session, headed: true },
  ]);
  // Persona credentials are injected so the useProfile op can re-authenticate.
  assert.equal(replay.env.APP_CLIENT_ID, 'cid-literal');
  // ...and in the chat's record dir, where the connected profile is registered.
  assert.ok(String(replay.env.AGENT_QA_RECORD_DIR || '').includes(created.session));

  // sid is required.
  assert.equal((await j('POST', `/api/chat/c/${created.id}/replay`, {})).status, 400);
});

test('persona credentials inject into the plugin env; unresolved credential refs fail connect', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    runCli: async (args, extraEnv) => {
      calls.push({ args, extraEnv });
      // creds-resolve: resolve everything except `vault:` refs (no plugin
      // claims them in this fixture) — mirrors the verb's contract.
      if (args[0] === 'creds-resolve') {
        const map = JSON.parse(args[1]);
        const unresolved = Object.keys(map).filter((k) => String(map[k]).startsWith('vault:'));
        if (unresolved.length) {
          return {
            code: 1,
            stdout: '',
            stderr: `could not resolve credential refs: ${unresolved.join(', ')} — no credentials plugin serves scheme(s) vault`,
          };
        }
        return { code: 0, stdout: JSON.stringify(map), stderr: '' };
      }
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

  // a scheme ref no installed plugin claims can't resolve → connect fails,
  // no bootstrap. (The CLI's creds-resolve reports the cause; here the stub
  // answers it like a missing provider would.)
  await j('POST', '/api/personas/vaulted', {
    name: 'Vaulted',
    profile: 'vaulted',
    credentials: { entries: { APP_EMAIL: 'vault:dev/data/x:EMAIL' } },
  });
  calls.length = 0;
  const r2 = await (await j('POST', '/api/personas/vaulted/connect', { environmentId: 'staging' })).json();
  assert.equal(r2.ok, false);
  assert.ok(r2.log.some((s) => s.step === 'credentials'));
  assert.match(r2.detail, /no credentials plugin/);
  // No remediation declared on the environment → none offered; provider
  // login commands stay downstream (auth.remediation).
  assert.equal(r2.remediation, undefined);
  assert.ok(!calls.some((c) => c.args[0] === 'profile-bootstrap'));
});

test('plan run + scenario replay inject the persona credentials and self-bootstrap the profile', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const replays = [];
  const deps = {
    runCli: async (args, extraEnv) => {
      calls.push({ args, extraEnv });
      if (args[0] === 'creds-resolve') {
        const map = JSON.parse(args[1]);
        const unresolved = Object.keys(map).filter((k) => String(map[k]).startsWith('vault:'));
        if (unresolved.length) {
          return {
            code: 1,
            stdout: '',
            stderr: `could not resolve credential refs: ${unresolved.join(', ')} — no credentials plugin serves scheme(s) vault`,
          };
        }
        return { code: 0, stdout: JSON.stringify(map), stderr: '' };
      }
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

  // A persona whose scheme ref can't resolve fails the run up front — no replay.
  await j('POST', '/api/personas/vaulted', {
    name: 'Vaulted',
    profile: 'vaulted',
    credentials: { entries: { APP_PASSWORD: 'vault:dev/data/x:PW' } },
  });
  replays.length = 0;
  const bad = await (await j('POST', '/api/plans/p1/run', { personaId: 'vaulted', environmentId: 'staging' })).json();
  assert.equal(bad.ok, false);
  assert.match(bad.error, /credential refs/i);
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

test('autoConnectDefault signs in the default persona for a new chat (background pre-connect)', async (t) => {
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

  // Both records must be flagged `default` — a sole record stays guest.
  await j('POST', '/api/environments/staging', {
    name: 'Staging',
    default: true,
    auth: { plugin: 'agent-qa-plugin-acme', creds: { APP_CLIENT_ID: 'cid' } },
  });
  await j('POST', '/api/personas/admin', {
    name: 'Admin',
    profile: 'admin-user',
    default: true,
    credentials: { entries: { APP_EMAIL: 'a@b.com' } },
  });

  const { autoConnectDefault } = require('../lib/report-server.js');
  const entry = { browser: { name: 'chat-test' }, recordDir: () => null };
  await autoConnectDefault(fx.root, entry, deps);

  assert.equal(entry.autoConnect.state, 'connected');
  assert.equal(entry.connectedProfile, 'admin-user'); // bound so `start` records a useProfile baseline
  const boot2 = calls.find((c) => c.args[0] === 'profile-bootstrap');
  assert.ok(boot2, 'auto-connect ran profile-bootstrap');
  assert.equal(boot2.extraEnv.APP_EMAIL, 'a@b.com'); // persona identity cred
  assert.equal(boot2.extraEnv.APP_CLIENT_ID, 'cid'); // environment shared cred
  assert.equal(boot2.args[boot2.args.indexOf('--session') + 1], 'chat-test'); // into the chat's own session
});

test('autoConnectDefault runs an extension-declared remediation before retrying sign-in', async (t) => {
  const fx = makeFixture();
  const commands = [];
  let statusCalls = 0;
  const deps = {
    runAuthRemediation: async (argv) => {
      commands.push(argv);
      return { ok: true };
    },
    runCli: async (args) => {
      if (args[0] === 'profile-status') {
        statusCalls += 1;
        return { code: 0, stdout: statusCalls === 1 ? 'signed out' : 'authenticated', stderr: '' };
      }
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // Remediation argv only counts from trusted records — write the env file
  // directly (as a shipped package record / hand edit would), not via POST.
  fs.mkdirSync(path.join(fx.root, '_environments', 'staging'), { recursive: true });
  fs.writeFileSync(
    path.join(fx.root, '_environments', 'staging', 'environment.json'),
    JSON.stringify({
      id: 'staging',
      name: 'Staging',
      auth: {
        plugin: 'agent-qa-plugin-acme',
        remediation: { label: 'Sign in to credentials provider', argv: ['credential-login', '--browser'], automatic: true },
      },
      default: true,
    }),
  );
  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user', default: true });

  const entry = { browser: { name: 'chat-test' }, recordDir: () => null };
  await srv.autoConnectDefault(fx.root, entry, deps);

  assert.deepEqual(commands, [['credential-login', '--browser']]);
  assert.equal(entry.autoConnect.state, 'connected');
  assert.equal(statusCalls, 2);
});

test('chat remediation runs the extension command and retries that chat connection', async (t) => {
  const fx = makeFixture();
  const commands = [];
  const deps = {
    chat: { hub: {} },
    runAuthRemediation: async (argv) => {
      commands.push(argv);
      return { ok: true };
    },
    runCli: async (args) => {
      if (args[0] === 'profile-status') return { code: 0, stdout: commands.length ? 'authenticated' : 'signed out', stderr: '' };
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // Remediation argv only counts from trusted records — write the env file
  // directly rather than POSTing it through the API.
  fs.mkdirSync(path.join(fx.root, '_environments', 'staging'), { recursive: true });
  fs.writeFileSync(
    path.join(fx.root, '_environments', 'staging', 'environment.json'),
    JSON.stringify({
      id: 'staging',
      name: 'Staging',
      auth: { remediation: { label: 'Prepare credentials', argv: ['credential-login'], automatic: false } },
    }),
  );
  await j('POST', '/api/personas/admin', { name: 'Admin', profile: 'admin-user' });
  const chat = await (await j('POST', '/api/chat/create')).json();

  const result = await (await j('POST', `/api/chat/c/${chat.id}/remediate`, { personaId: 'admin', environmentId: 'staging' })).json();
  assert.deepEqual(commands, [['credential-login']]);
  assert.equal(result.authenticated, true);
  assert.equal(result.session, chat.session);
});

test('environment POST cannot plant remediation argv (trusted records only)', async (t) => {
  const fx = makeFixture();
  const commands = [];
  const deps = {
    chat: { hub: {} },
    runAuthRemediation: async (argv) => {
      commands.push(argv);
      return { ok: true };
    },
    runCli: async () => ({ code: 0, stdout: 'ok', stderr: '' }),
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // A browser-supplied remediation argv must be stripped at write time —
  // otherwise any local page could plant a command /remediate would exec.
  const res = await j('POST', '/api/environments/evil', {
    name: 'Evil',
    auth: { remediation: { label: 'x', argv: ['planted'], automatic: true } },
  });
  assert.equal(res.status, 200);
  const saved = JSON.parse(
    fs.readFileSync(path.join(fx.root, '_environments', 'evil', 'environment.json'), 'utf8'),
  );
  assert.equal(saved.auth.remediation, null);
  assert.equal(commands.length, 0);

  // …and editing the same record preserves a remediation that a trusted
  // (hand-written/package) record already had on disk.
  saved.auth.remediation = { label: 'Keep', argv: ['keep'], automatic: false };
  fs.writeFileSync(path.join(fx.root, '_environments', 'evil', 'environment.json'), JSON.stringify(saved));
  await j('POST', '/api/environments/evil', { name: 'Renamed' });
  const after = JSON.parse(
    fs.readFileSync(path.join(fx.root, '_environments', 'evil', 'environment.json'), 'utf8'),
  );
  assert.deepEqual(after.auth.remediation, { label: 'Keep', argv: ['keep'], automatic: false });
});

test('chat recording controls run buffer verbs in the chat record dir', async (t) => {
  const fx = makeFixture();
  const recordRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-rec-'));
  const calls = [];
  const deps = {
    chat: { hub: {} },
    recordRoot,
    runCli: async (args, extraEnv) => {
      calls.push({ args, extraEnv });
      return { code: 0, stdout: 'ok', stderr: '' };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  const chat = await (await j('POST', '/api/chat/create')).json();
  const chatDir = path.join(recordRoot, chat.session);

  for (const [sub, want] of [
    ['pause', ['record', 'pause']],
    ['resume', ['record', 'resume']],
    ['step-delete', ['buffer', 'delete', '1']],
    ['step-edit', ['buffer', 'edit', '0', JSON.stringify({ verb: 'click', on: '#a' })]],
    [
      'step-insert',
      [
        'buffer',
        'insert',
        '0',
        'check',
        JSON.stringify({ intent: 'page looks right', claim: { subject: { shot: 's0' }, predicate: 'matches' } }),
      ],
    ],
    ['check', ['buffer', 'check']],
  ]) {
    const body =
      sub === 'step-delete'
        ? { index: 1 }
        : sub === 'step-edit'
          ? { index: 0, payload: { verb: 'click', on: '#a' } }
          : sub === 'step-insert'
            ? { index: 0, kind: 'check', payload: { intent: 'page looks right', claim: { subject: { shot: 's0' }, predicate: 'matches' } } }
            : {};
    const r = await j('POST', `/api/chat/c/${chat.id}/recording/${sub}`, body);
    assert.equal(r.status, 200, sub);
    const last = calls.at(-1);
    assert.deepEqual(last.args, want, sub);
    assert.equal(last.extraEnv.AGENT_QA_RECORD_DIR, chatDir, `${sub} targets the chat's record dir`);
  }

  // Validation: bad index rejected without a CLI call.
  const before = calls.length;
  const bad = await j('POST', `/api/chat/c/${chat.id}/recording/step-delete`, { index: -1 });
  assert.equal(bad.status, 400);
  assert.equal(calls.length, before);
});

test('POST /api/edit/load delegates to buffer load with a safe sid', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = { runCli: async (args) => (calls.push(args), { code: 0, stdout: 'ok', stderr: '' }) };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  let res = await j('POST', '/api/edit/load', { sid: 'flow' });
  assert.equal(res.status, 200);
  assert.deepEqual(calls.at(-1), ['buffer', 'load', 'flow']);

  res = await j('POST', '/api/edit/load', { sid: 'flow', force: true });
  assert.equal(res.status, 200);
  assert.deepEqual(calls.at(-1), ['buffer', 'load', 'flow', '--force']);

  res = await j('POST', '/api/edit/load', { sid: '../escape' });
  assert.equal(res.status, 400);
  res = await j('POST', '/api/edit/load', {});
  assert.equal(res.status, 400);
});

test('autoConnectDefault is a no-op when no persona/environment is configured', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = { runCli: async (args) => (calls.push(args), { code: 0, stdout: 'ok', stderr: '' }) };
  const { server } = await boot(fx.root, deps);
  t.after(() => server.close());
  const { autoConnectDefault } = require('../lib/report-server.js');
  const entry = { browser: { name: 'chat-test' }, recordDir: () => null };
  await autoConnectDefault(fx.root, entry, deps);
  assert.equal(entry.autoConnect, undefined); // nothing to connect → never started
  assert.ok(!calls.some((a) => a[0] === 'profile-bootstrap'));
});

test('autoConnectDefault is a no-op for a sole unflagged persona/environment', async (t) => {
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

  // One persona + one environment, neither flagged default — new chats must
  // not auto-connect (no vault lookup against public-page work).
  await j('POST', '/api/environments/staging', {
    name: 'Staging',
    auth: { plugin: 'agent-qa-plugin-acme', creds: { APP_CLIENT_ID: 'cid' } },
  });
  await j('POST', '/api/personas/admin', {
    name: 'Admin',
    profile: 'admin-user',
    credentials: { entries: { APP_EMAIL: 'a@b.com' } },
  });

  const { autoConnectDefault } = require('../lib/report-server.js');
  const entry = { browser: { name: 'chat-test' }, recordDir: () => null };
  await autoConnectDefault(fx.root, entry, deps);

  assert.equal(entry.autoConnect, undefined);
  assert.equal(entry.connectedProfile, undefined);
  assert.ok(!calls.some((c) => c.args[0] === 'profile-bootstrap'));
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

test('POST /api/scenarios/:sid/insert-step delegates to scenario insert', async (t) => {
  const fx = makeFixture();
  const calls = [];
  const deps = {
    runCli: async (args) => {
      calls.push(args);
      return { code: 0, stdout: 'inserted check step at 2 (id=s3); 4 step(s)\n', stderr: '' };
    },
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (p, b) =>
    fetch(`${base}${p}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(b),
    });

  const draft = { intent: 'still ok', claim: { subject: { url: true }, predicate: 'exists' } };
  const res = await j(`/api/scenarios/${fx.sid}/insert-step`, { kind: 'check', draft, after: 's1' });
  assert.equal(res.status, 200);
  const body = await res.json();
  assert.equal(body.ok, true);
  assert.deepEqual(calls[0], [
    'scenario',
    'insert',
    path.join(fx.root, fx.sid, 'scenario.json'),
    'check',
    JSON.stringify(draft),
    '--after',
    's1',
  ]);

  // --at index fallback
  await j(`/api/scenarios/${fx.sid}/insert-step`, { kind: 'do', draft, at: 0 });
  assert.equal(calls[1][5], '--at');
  assert.equal(calls[1][6], '0');

  // bad input rejected: unsafe sid, missing draft, bad kind
  assert.equal((await j(`/api/scenarios/..%2fescape/insert-step`, { kind: 'check', draft })).status, 400);
  assert.equal((await j(`/api/scenarios/${fx.sid}/insert-step`, { kind: 'check' })).status, 400);
  assert.equal((await j(`/api/scenarios/${fx.sid}/insert-step`, { kind: 'wait', draft })).status, 400);
});

test('POST /api/edit/check runs buffer check with strict passthrough', async (t) => {
  const fx = makeFixture();
  const calls = [];
  let fail = false;
  const deps = {
    runCli: async (args) => (
      calls.push(args),
      fail ? { code: 1, stdout: 'lint FAIL (1 error)', stderr: '' } : { code: 0, stdout: 'lint OK', stderr: '' }
    ),
  };
  const { server, base } = await boot(fx.root, deps);
  t.after(() => server.close());
  const j = (p, b) =>
    fetch(`${base}${p}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(b || {}) });

  const r = await j('/api/edit/check', {});
  assert.equal(r.status, 200);
  assert.deepEqual(calls.at(-1), ['buffer', 'check']);
  const body = await r.json();
  assert.match(body.stdout, /lint OK/);

  await j('/api/edit/check', { strict: true });
  assert.deepEqual(calls.at(-1), ['buffer', 'check', '--strict']);

  // A failing check surfaces the report text as `error` with 422.
  fail = true;
  const bad = await j('/api/edit/check', {});
  assert.equal(bad.status, 422);
  assert.match((await bad.json()).error, /lint FAIL/);
});

test('GET/POST /api/config/settings round-trips and validates', async (t) => {
  const fx = makeFixture();
  const { server, base } = await boot(fx.root);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  // defaults: no stored values, effective auto/headless
  const d0 = await (await j('GET', '/api/config/settings')).json();
  assert.equal(d0.effective.chatBackend, 'auto');
  assert.equal(d0.effective.headedDefault, false);
  assert.equal(d0.root, fx.root);

  // write both fields → persisted file + effective flip
  const d1 = await (await j('POST', '/api/config/settings', { chatBackend: 'opencode', headedDefault: true })).json();
  assert.equal(d1.effective.chatBackend, 'opencode');
  assert.equal(d1.effective.headedDefault, true);
  const onDisk = JSON.parse(fs.readFileSync(path.join(fx.root, '_config', 'settings.json'), 'utf8'));
  assert.equal(onDisk.schema, 'settings/1');
  assert.equal(onDisk.chatBackend, 'opencode');

  // 'auto' clears the stored backend
  const d2 = await (await j('POST', '/api/config/settings', { chatBackend: 'auto' })).json();
  assert.equal(d2.effective.chatBackend, 'auto');
  assert.equal(d2.settings.chatBackend, undefined);

  // validation: unknown key / bad enum / bad type → 400
  assert.equal((await j('POST', '/api/config/settings', { nope: 1 })).status, 400);
  assert.equal((await j('POST', '/api/config/settings', { chatBackend: 'wat' })).status, 400);
  assert.equal((await j('POST', '/api/config/settings', { headedDefault: 'yes' })).status, 400);
});

test('GET/POST /api/jev enables the bundled plugin and stores the key write-only', async (t) => {
  const fx = makeFixture();
  // Isolate the global config home so the stored key lands in a temp dir —
  // JEV writes to $AGENT_QA_HOME/_config/jev.json (shared across roots).
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-home-jev-'));
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

  // defaults: bundled plugin ships in the package; off, no key
  const d0 = await (await j('GET', '/api/jev')).json();
  assert.equal(d0.available, true);
  assert.equal(d0.enabled, false);
  assert.equal(d0.hasKey, false);

  // enable + store key → persisted 0600 config in the GLOBAL home, key never
  // echoed back; nothing written under the scenarios root.
  const d1 = await (await j('POST', '/api/jev', { enabled: true, apiKey: 'sk-test-1' })).json();
  assert.equal(d1.ok, true);
  assert.equal(d1.enabled, true);
  assert.equal(d1.hasKey, true);
  assert.equal('apiKey' in d1, false);
  const onDisk = JSON.parse(fs.readFileSync(path.join(home, '_config', 'jev.json'), 'utf8'));
  assert.equal(onDisk.schema, 'jev/1');
  assert.equal(onDisk.apiKey, 'sk-test-1');
  assert.equal(fs.existsSync(path.join(fx.root, '_config', 'jev.json')), false);
  const d2 = await (await j('GET', '/api/jev')).json();
  assert.equal('apiKey' in d2, false);
  assert.equal(d2.hasKey, true);

  // disable without touching the stored key
  const d3 = await (await j('POST', '/api/jev', { enabled: false })).json();
  assert.equal(d3.enabled, false);
  assert.equal(d3.hasKey, true);
});

test('a per-root jev.json shadows the global record on read', async (t) => {
  const fx = makeFixture();
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-home-jev-'));
  const prevHome = process.env.AGENT_QA_HOME;
  process.env.AGENT_QA_HOME = home;
  t.after(() => {
    if (prevHome === undefined) delete process.env.AGENT_QA_HOME;
    else process.env.AGENT_QA_HOME = prevHome;
  });
  // Global says enabled; the root file overrides with disabled.
  fs.mkdirSync(path.join(home, '_config'), { recursive: true });
  fs.writeFileSync(
    path.join(home, '_config', 'jev.json'),
    JSON.stringify({ schema: 'jev/1', enabled: true, apiKey: 'sk-global' }),
  );
  fs.mkdirSync(path.join(fx.root, '_config'), { recursive: true });
  fs.writeFileSync(
    path.join(fx.root, '_config', 'jev.json'),
    JSON.stringify({ schema: 'jev/1', enabled: false, apiKey: 'sk-root' }),
  );

  const { server, base } = await boot(fx.root);
  t.after(() => server.close());
  const j = (m, p, b) =>
    fetch(`${base}${p}`, { method: m, headers: { 'content-type': 'application/json' }, body: b ? JSON.stringify(b) : undefined });

  const d = await (await j('GET', '/api/jev')).json();
  assert.equal(d.enabled, false); // root shadowed global
  assert.equal(d.hasKey, true);   // root's key, not global's
});

test('GET /api/scenarios surfaces a corrupt scenario.json instead of hiding the sid', async (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-report-corrupt-'));
  const badSid = 'broken-sid';
  fs.mkdirSync(path.join(root, badSid), { recursive: true });
  fs.writeFileSync(path.join(root, badSid, 'scenario.json'), '{"schema":"scenario/2","steps":[');

  const { server, base } = await boot(root);
  t.after(() => server.close());
  const res = await fetch(`${base}/api/scenarios`);
  assert.equal(res.status, 200);
  const body = await res.json();
  assert.equal(body.scenarios.length, 1);
  const sc = body.scenarios[0];
  assert.equal(sc.sid, badSid);
  assert.equal(sc.hasScenario, false);
  assert.match(sc.scenarioError, /unparseable scenario\.json/);
});

// A wedged CLI child used to hang the request forever (no execFile
// timeout), and if it did die to a signal the null exit code surfaced
// as code:0 — a silent success. makeCliRunner now bounds every call.
test('makeCliRunner kills a hung CLI at the host timeout', async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-hung-cli-'));
  const bin = path.join(tmp, 'hung-cli.js');
  fs.writeFileSync(bin, '#!/usr/bin/env node\nsetTimeout(() => {}, 60000);\n');
  fs.chmodSync(bin, 0o755);
  const runCli = srv.makeCliRunner({ bin, env: { ...process.env }, cwd: tmp, timeoutMs: 250 });
  const r = await runCli(['anything']);
  assert.equal(r.code, 124);
  assert.equal(r.spawnError, null);
  assert.match(r.stderr, /host timeout/);
});

test('makeCliRunner never maps a signal death to exit 0', async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-sig-cli-'));
  const bin = path.join(tmp, 'sig-cli.js');
  fs.writeFileSync(bin, '#!/usr/bin/env node\nprocess.kill(process.pid, "SIGKILL");\n');
  fs.chmodSync(bin, 0o755);
  const runCli = srv.makeCliRunner({ bin, env: { ...process.env }, cwd: tmp });
  const r = await runCli(['anything']);
  assert.notEqual(r.code, 0);
  assert.notEqual(r.code, null);
});

test('makeCliRunner still reports a clean exit code', async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-ok-cli-'));
  const bin = path.join(tmp, 'ok-cli.js');
  fs.writeFileSync(bin, '#!/usr/bin/env node\nconsole.log("out");\nprocess.exit(3);\n');
  fs.chmodSync(bin, 0o755);
  const runCli = srv.makeCliRunner({ bin, env: { ...process.env }, cwd: tmp });
  const r = await runCli(['anything']);
  assert.equal(r.code, 3);
  assert.match(r.stdout, /out/);
});
