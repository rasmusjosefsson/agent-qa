'use strict';
// Tests for the authoring-editor write surface (lib/report-server.js).
//
// The editor shells the Rust CLI for every mutation. These tests inject a
// STUB runCli so they exercise the HTTP routing, body validation, and
// result-relaying without needing the real binary or a live browser. The
// stub records the exact argv it was handed — that argv IS the contract
// between the Node server and the Rust CLI, so asserting on it locks the
// wiring down.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const srv = require('../lib/report-server.js');

// ---- unit helpers ----

test('lastJsonLine returns the last parseable JSON line', () => {
  assert.deepEqual(srv.lastJsonLine('noise\n{"ok":true}\n'), { ok: true });
  assert.equal(srv.lastJsonLine('not json at all'), null);
});

test('resolveRecordRoot honors env then default', () => {
  assert.equal(
    srv.resolveRecordRoot({ env: { AGENT_QA_RECORD_DIR: '/rec' }, cwd: '/x' }),
    '/rec',
  );
  assert.equal(
    srv.resolveRecordRoot({ env: {}, cwd: '/work' }),
    path.join('/work', 'tmp', 'agent-qa-record'),
  );
});

test('EDIT_KINDS mirrors the recorder trigger kinds', () => {
  assert.deepEqual(srv.EDIT_KINDS, ['do', 'check']);
});

// ---- integration harness ----

// A stub runCli driven by a per-test handler. It records every argv it is
// asked to run and returns whatever the handler decides.
function makeStub(handler) {
  const calls = [];
  const runCli = (args) => {
    calls.push(args);
    const out = handler ? handler(args) : null;
    return Promise.resolve(
      out || { code: 0, stdout: '', stderr: '', spawnError: null },
    );
  };
  return { runCli, calls };
}

function bootEdit(handler) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-edit-root-'));
  const recordRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-edit-rec-'));
  const stub = makeStub(handler);
  // Stub the live bridge so endpoint tests don't need a real CDP socket.
  const live = {
    subscribed: [],
    inputs: [],
    subscribe(res) {
      this.subscribed.push(res);
    },
    unsubscribe(res) {
      this.subscribed = this.subscribed.filter((r) => r !== res);
    },
    input(evt) {
      this.inputs.push(evt);
      return evt && evt.type === 'click';
    },
    async pick(nx, ny) {
      this.picked = { nx, ny };
      return { role: 'button', name: 'Login', x: 10, y: 20 };
    },
    stop() {},
  };
  const server = srv.createServer(root, { recordRoot, runCli: stub.runCli, live });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({ server, base: `http://127.0.0.1:${port}`, root, recordRoot, calls: stub.calls, live });
    });
  });
}

const postJson = (base, route, body) =>
  fetch(`${base}${route}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body || {}),
  });

test('editor endpoints shell the right CLI argv', async (t) => {
  const handler = (args) => {
    const v = args[0];
    if (v === 'buffer' && args[1] === 'list') {
      return {
        code: 0,
        stdout: JSON.stringify({ sid: 's-xyz', intent: 'do a thing', session: 'default', rows: [
            { stepId: 's0', stepIndex: 0, step: { id: 's0', kind: 'do', intent: 'open page', verb: 'goto', value: { from: 'literal', literal: 'https://example.com/' } } },
          ],
        }),
        stderr: '',
        spawnError: null,
      };
    }
    if (v === 'start') return { code: 0, stdout: 'started sid=s-xyz\n', stderr: '', spawnError: null };
    if (v === 'run-step') {
      return { code: 0, stdout: JSON.stringify({ ok: true, stepId: 's0', kind: 'do', verb: 'click' }), stderr: '', spawnError: null };
    }
    if (v === 'aria-snapshot') {
      return {
        code: 0,
        stdout: JSON.stringify({ session: 'default', nodes: [{ depth: 0, role: 'button', name: 'Login', ref: 'e5', pickable: true }] }),
        stderr: '',
        spawnError: null,
      };
    }
    if (v === 'flush') {
      return { code: 0, stdout: 'flushed sid=s-xyz steps=2\nwrote /tmp/x/scenario.json\n', stderr: '', spawnError: null };
    }
    return { code: 0, stdout: '', stderr: '', spawnError: null };
  };
  const { server, base, recordRoot, calls } = await bootEdit(handler);
  t.after(() => server.close());

  await t.test('GET /api/edit/buffer reads CLI state', async () => {
    const res = await fetch(`${base}/api/edit/buffer`);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.sid, 's-xyz');
    assert.equal(body.intent, 'do a thing');
    assert.equal(body.rows.length, 1);
    assert.equal(body.rows[0].step.kind, 'do');
    assert.deepEqual(calls.at(-1), ['buffer', 'list', '--json']);
  });

  await t.test('POST /api/edit/start passes intent + open url and returns sid', async () => {
    const res = await postJson(base, '/api/edit/start', { intent: 'open it', url: 'https://example.com/' });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.ok, true);
    assert.equal(body.sid, 's-xyz');
    assert.deepEqual(calls.at(-1), ['start', 'open it', '--open', 'https://example.com/']);
  });

  await t.test('POST /api/edit/record passes kind + JSON payload', async () => {
    const payload = { intent: 'open page', verb: 'goto', value: { from: 'literal', literal: 'https://example.com/' } };
    const res = await postJson(base, '/api/edit/record', { kind: 'do', payload });
    assert.equal(res.status, 200);
    assert.deepEqual(calls.at(-1), ['record-step', 'do', JSON.stringify(payload)]);
  });

  await t.test('POST /api/edit/run-step relays the structured report', async () => {
    const res = await postJson(base, '/api/edit/run-step', {
      kind: 'do',
      payload: { intent: 'click Login', verb: 'click', on: { role: 'button', name: 'Login' } },
    });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.result.ok, true);
    assert.equal(body.result.verb, 'click');
    assert.deepEqual(calls.at(-1), [
      'run-step',
      'do',
      JSON.stringify({ intent: 'click Login', verb: 'click', on: { role: 'button', name: 'Login' } }),
    ]);
  });

  await t.test('GET /api/edit/snapshot returns parsed ARIA nodes', async () => {
    const res = await fetch(`${base}/api/edit/snapshot?interactive=1`);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.result.nodes[0].role, 'button');
    assert.deepEqual(calls.at(-1), ['aria-snapshot', '--interactive']);
  });

  await t.test('POST /api/edit/delete + move pass indices', async () => {
    await postJson(base, '/api/edit/delete', { index: 1 });
    assert.deepEqual(calls.at(-1), ['buffer', 'delete', '1']);
    await postJson(base, '/api/edit/move', { from: 2, to: 0 });
    assert.deepEqual(calls.at(-1), ['buffer', 'move', '2', '0']);
  });

  await t.test('POST /api/edit/flush surfaces sid + scenario file', async () => {
    const res = await postJson(base, '/api/edit/flush', {});
    const body = await res.json();
    assert.equal(body.sid, 's-xyz');
    assert.equal(body.scenarioFile, '/tmp/x/scenario.json');
    assert.deepEqual(calls.at(-1), ['flush']);
  });
});

test('live-browser endpoints relay to the bridge', async (t) => {
  const { server, base, live } = await bootEdit();
  t.after(() => server.close());

  await t.test('GET /api/edit/stream opens an SSE stream + subscribes', async () => {
    const ctrl = new AbortController();
    const res = await fetch(`${base}/api/edit/stream`, { signal: ctrl.signal });
    assert.equal(res.status, 200);
    assert.match(res.headers.get('content-type'), /text\/event-stream/);
    assert.equal(live.subscribed.length, 1, 'subscribed to the bridge');
    ctrl.abort();
    await res.body.cancel().catch(() => {});
  });

  await t.test('POST /api/edit/input forwards the event (200 when handled)', async () => {
    const res = await postJson(base, '/api/edit/input', { type: 'click', nx: 0.5, ny: 0.5 });
    assert.equal(res.status, 200);
    assert.equal((await res.json()).ok, true);
    assert.deepEqual(live.inputs.at(-1), { type: 'click', nx: 0.5, ny: 0.5 });
  });

  await t.test('POST /api/edit/input returns 409 when the bridge cannot handle it', async () => {
    const res = await postJson(base, '/api/edit/input', { type: 'key', key: 'F13' });
    assert.equal(res.status, 409);
    assert.equal((await res.json()).ok, false);
  });

  await t.test('POST /api/edit/pick returns the picked element', async () => {
    const res = await postJson(base, '/api/edit/pick', { nx: 0.5, ny: 0.5 });
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.ok, true);
    assert.deepEqual(body.element, { role: 'button', name: 'Login', x: 10, y: 20 });
    assert.deepEqual(live.picked, { nx: 0.5, ny: 0.5 });
  });

  await t.test('POST /api/edit/pick rejects out-of-range coords', async () => {
    const res = await postJson(base, '/api/edit/pick', { nx: 2, ny: 0.5 });
    assert.equal(res.status, 400);
  });
});

test('editor write-path validation rejects bad input before shelling', async (t) => {
  const { server, base, calls } = await bootEdit(() => ({ code: 0, stdout: '', stderr: '', spawnError: null }));
  t.after(() => server.close());

  await t.test('unknown kind → 400, CLI not invoked', async () => {
    const before = calls.length;
    const res = await postJson(base, '/api/edit/record', { kind: 'evil', payload: {} });
    assert.equal(res.status, 400);
    assert.equal(calls.length, before, 'CLI must not be shelled on a bad kind');
  });

  await t.test('start without intent → 400', async () => {
    const res = await postJson(base, '/api/edit/start', {});
    assert.equal(res.status, 400);
  });

  await t.test('delete with non-integer index → 400', async () => {
    const res = await postJson(base, '/api/edit/delete', { index: 'x' });
    assert.equal(res.status, 400);
  });

  await t.test('GET on a POST-only route → 405', async () => {
    const res = await fetch(`${base}/api/edit/flush`);
    assert.equal(res.status, 405);
  });
});

test('CLI failures relay as 422 / 500', async (t) => {
  await t.test('non-zero exit → 422 with stderr', async () => {
    const { server, base } = await bootEdit((args) =>
      args[0] === 'record-step'
        ? { code: 1, stdout: '', stderr: 'invalid payload: route required', spawnError: null }
        : { code: 0, stdout: '', stderr: '', spawnError: null },
    );
    t.after(() => server.close());
    const res = await postJson(base, '/api/edit/record', { kind: 'do', payload: { intent: 'bad' } });
    // The direct draft passes the Node object-shape gate; the CLI rejects it.
    assert.equal(res.status, 422);
    const body = await res.json();
    assert.equal(body.ok, false);
    assert.match(body.error, /route required/);
  });

  await t.test('spawn failure → 500', async () => {
    const { server, base } = await bootEdit(() => ({
      code: null,
      stdout: '',
      stderr: '',
      spawnError: new Error('spawn agent-qa ENOENT'),
    }));
    t.after(() => server.close());
    const res = await postJson(base, '/api/edit/flush', {});
    assert.equal(res.status, 500);
    const body = await res.json();
    assert.match(body.error, /ENOENT/);
  });
});

test('editor is gated off when no CLI runner is configured', async (t) => {
  // createServer without deps → the read-only viewer, editor disabled.
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-edit-noop-'));
  const server = srv.createServer(root);
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  t.after(() => server.close());
  const { port } = server.address();
  const base = `http://127.0.0.1:${port}`;

  const res = await fetch(`${base}/api/edit/buffer`);
  assert.equal(res.status, 503);

  // /api/root advertises editor availability for the UI to branch on.
  const root2 = await (await fetch(`${base}/api/root`)).json();
  assert.equal(root2.editor, false);
});

test('static editor.html (React editor entry) is served', async (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-edit-static-'));
  const stub = makeStub();
  const server = srv.createServer(root, { recordRoot: root, runCli: stub.runCli });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  t.after(() => server.close());
  const { port } = server.address();
  const base = `http://127.0.0.1:${port}`;

  const html = await fetch(`${base}/editor`);
  assert.equal(html.status, 200);
  assert.match(html.headers.get('content-type'), /text\/html/);
  const body = await html.text();
  assert.match(body, /editor/i);

  // the entry references a hashed JS asset under /assets which resolves as JS.
  const m = body.match(/\/assets\/[A-Za-z0-9._-]+\.js/);
  assert.ok(m, 'expected an /assets/*.js reference in editor.html');
  const js = await fetch(`${base}${m[0]}`);
  assert.equal(js.status, 200);
  assert.match(js.headers.get('content-type'), /javascript/);

  // /api/root advertises the editor as available.
  const r = await (await fetch(`${base}/api/root`)).json();
  assert.equal(r.editor, true);
});
