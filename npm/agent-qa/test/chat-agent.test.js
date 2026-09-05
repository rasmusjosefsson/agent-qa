'use strict';
// Tests for the in-app chat backend.
//
// Dependency-light, no real LLM: the pure wrapper (createChatHub) is driven
// with an injected fake AgentSession, and the /api/chat/* routes are exercised
// against a stubbed hub. Also covers pi-SDK path resolution helpers and the
// "unavailable when the SDK can't be resolved" fallback.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const srv = require('../lib/report-server.js');

const chatAgentUrl = pathToFileURL(path.join(__dirname, '..', 'lib', 'chat-agent.mjs')).href;
let chatAgentMod = null;
async function chatAgent() {
  if (!chatAgentMod) chatAgentMod = await import(chatAgentUrl);
  return chatAgentMod;
}

// ---- fakes ----

function makeFakeSession(overrides = {}) {
  let listener = null;
  const calls = { prompt: [], abort: 0, dispose: 0, subscribe: 0 };
  const session = {
    isStreaming: false,
    messages: [],
    model: { provider: 'anthropic', id: 'claude-x', label: 'Claude X' },
    thinkingLevel: 'off',
    sessionId: 'sess-1',
    agent: { state: { streamingMessage: null } },
    subscribe(l) {
      listener = l;
      calls.subscribe++;
      return () => {
        listener = null;
      };
    },
    async prompt(text, opts) {
      calls.prompt.push({ text, opts });
    },
    async abort() {
      calls.abort++;
    },
    dispose() {
      calls.dispose++;
    },
    emit(ev) {
      if (listener) listener(ev);
    },
    ...overrides,
  };
  return { session, calls };
}

function makeFakeRes() {
  return {
    frames: '',
    ended: false,
    write(s) {
      this.frames += s;
      return true;
    },
    end() {
      this.ended = true;
    },
  };
}

function makeManualTimers() {
  let lastFn = null;
  return {
    timers: {
      set(fn) {
        lastFn = fn;
        return { unref() {} };
      },
      clear() {},
    },
    trigger() {
      if (lastFn) lastFn();
    },
  };
}

// ---- createChatHub wrapper ----

test('createChatHub lazily creates the session on first prompt', async () => {
  const { createChatHub } = await chatAgent();
  const fake = makeFakeSession();
  let made = 0;
  const hub = createChatHub({
    createSession: () => {
      made++;
      return fake.session;
    },
  });
  assert.equal(hub.hasSession, false);
  await hub.prompt('hello');
  assert.equal(made, 1);
  assert.equal(hub.hasSession, true);
  assert.equal(fake.calls.prompt.length, 1);
  assert.equal(fake.calls.prompt[0].text, 'hello');
  // Second prompt reuses the same session.
  await hub.prompt('again');
  assert.equal(made, 1);
  assert.equal(fake.calls.prompt.length, 2);
  hub.dispose();
});

test('createChatHub fans agent events out to SSE subscribers', async () => {
  const { createChatHub } = await chatAgent();
  const fake = makeFakeSession();
  const hub = createChatHub({ createSession: () => fake.session });
  const res = makeFakeRes();
  hub.subscribe(res);
  assert.match(res.frames, /: connected/);
  await hub.prompt('go');
  // session_ready broadcast on creation
  assert.match(res.frames, /event: session\ndata: .*session_ready/);
  fake.session.emit({ type: 'agent_start' });
  fake.session.emit({ type: 'message_update', assistantMessageEvent: { type: 'text_delta', delta: 'hi' } });
  assert.match(res.frames, /event: agent\ndata: .*agent_start/);
  assert.match(res.frames, /text_delta/);
  assert.match(res.frames, /"delta":"hi"/);
  hub.dispose();
  assert.equal(res.ended, true);
});

test('createChatHub routes abort to the session', async () => {
  const { createChatHub } = await chatAgent();
  const fake = makeFakeSession();
  const hub = createChatHub({ createSession: () => fake.session });
  await hub.prompt('x');
  await hub.abort();
  assert.equal(fake.calls.abort, 1);
  hub.dispose();
});

test('createChatHub steers a prompt sent while streaming', async () => {
  const { createChatHub } = await chatAgent();
  const fake = makeFakeSession({ isStreaming: true });
  const hub = createChatHub({ createSession: () => fake.session });
  await hub.prompt('mid-stream message');
  assert.equal(fake.calls.prompt[0].opts.streamingBehavior, 'steer');
  // Explicit behavior wins.
  await hub.prompt('after', { streamingBehavior: 'followUp' });
  assert.equal(fake.calls.prompt[1].opts.streamingBehavior, 'followUp');
  hub.dispose();
});

test('createChatHub newSession disposes and respawns on next prompt', async () => {
  const { createChatHub } = await chatAgent();
  let made = 0;
  const sessions = [];
  const hub = createChatHub({
    createSession: () => {
      made++;
      const f = makeFakeSession();
      sessions.push(f);
      return f.session;
    },
  });
  await hub.prompt('first');
  assert.equal(made, 1);
  await hub.newSession();
  assert.equal(sessions[0].calls.dispose, 1);
  assert.equal(hub.hasSession, false);
  await hub.prompt('second');
  assert.equal(made, 2);
  hub.dispose();
});

test('createChatHub disposes the session after the idle timeout', async () => {
  const { createChatHub } = await chatAgent();
  const fake = makeFakeSession();
  const manual = makeManualTimers();
  const res = makeFakeRes();
  const hub = createChatHub({
    createSession: () => fake.session,
    idleMs: 1000,
    timers: manual.timers,
  });
  hub.subscribe(res);
  await hub.prompt('keep alive');
  assert.equal(hub.hasSession, true);
  manual.trigger(); // simulate idle elapse
  assert.equal(hub.hasSession, false);
  assert.equal(fake.calls.dispose, 1);
  assert.match(res.frames, /session_idle/);
  hub.dispose();
});

test('createChatHub getState reflects session presence', async () => {
  const { createChatHub } = await chatAgent();
  const fake = makeFakeSession();
  const hub = createChatHub({ createSession: () => fake.session });
  let st = await hub.getState();
  assert.equal(st.started, false);
  assert.deepEqual(st.messages, []);
  await hub.prompt('x');
  st = await hub.getState();
  assert.equal(st.started, true);
  assert.equal(st.model.id, 'claude-x');
  assert.equal(st.sessionId, 'sess-1');
  hub.dispose();
});

test('createChatHub getState lists models via listModels (before any session)', async () => {
  const { createChatHub } = await chatAgent();
  const models = [
    { provider: 'github-copilot', id: 'gpt-5-mini', label: 'GPT-5 mini' },
    { provider: 'github-copilot', id: 'claude-haiku-4.5', label: 'Claude Haiku 4.5' },
  ];
  const hub = createChatHub({
    createSession: () => makeFakeSession().session,
    listModels: () => models,
  });
  const st = await hub.getState();
  assert.equal(st.started, false);
  assert.deepEqual(st.models, models);
  hub.dispose();
});

test('createChatHub setModel resolves the model and calls session.setModel', async () => {
  const { createChatHub } = await chatAgent();
  const setCalls = [];
  const target = { provider: 'github-copilot', id: 'gpt-5-mini', label: 'GPT-5 mini' };
  const fake = makeFakeSession({
    modelRegistry: {
      find: (provider, id) => (id === target.id && provider === target.provider ? target : null),
      getAvailable: () => [target],
    },
    async setModel(m) {
      setCalls.push(m);
      this.model = m;
    },
  });
  const hub = createChatHub({ createSession: () => fake.session });
  const st = await hub.setModel({ provider: 'github-copilot', id: 'gpt-5-mini' });
  assert.equal(setCalls.length, 1);
  assert.equal(setCalls[0].id, 'gpt-5-mini');
  assert.equal(st.model.id, 'gpt-5-mini');
  hub.dispose();
});

test('createChatHub setModel rejects an unknown model', async () => {
  const { createChatHub } = await chatAgent();
  const fake = makeFakeSession({
    modelRegistry: { find: () => null, getAvailable: () => [] },
  });
  const hub = createChatHub({ createSession: () => fake.session });
  await assert.rejects(() => hub.setModel({ provider: 'x', id: 'nope' }), /not available/);
  hub.dispose();
});

test('createChatHub setThinkingLevel forwards to the session', async () => {
  const { createChatHub } = await chatAgent();
  const levels = [];
  const fake = makeFakeSession({
    getAvailableThinkingLevels: () => ['off', 'low', 'high'],
    setThinkingLevel(l) {
      levels.push(l);
      this.thinkingLevel = l;
    },
  });
  const hub = createChatHub({ createSession: () => fake.session });
  const st = await hub.setThinkingLevel('high');
  assert.deepEqual(levels, ['high']);
  assert.equal(st.thinkingLevel, 'high');
  assert.deepEqual(st.thinkingLevels, ['off', 'low', 'high']);
  hub.dispose();
});

test('createChatHub broadcasts an error event when prompt rejects', async () => {
  const { createChatHub } = await chatAgent();
  const hub = createChatHub({
    createSession: () => makeFakeSession({ prompt: async () => { throw new Error('kaboom'); } }).session,
  });
  const res = makeFakeRes();
  hub.subscribe(res);
  await assert.rejects(() => hub.prompt('boom'));
  assert.match(res.frames, /event: agent\ndata: .*"type":"error"/);
  assert.match(res.frames, /kaboom/);
  hub.dispose();
});

// ---- pi SDK resolution ----

test('resolvePiSdkUrl resolves an explicit file and an explicit dir', async () => {
  const { resolvePiSdkUrl } = await chatAgent();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-sdk-'));
  const distDir = path.join(dir, 'dist');
  fs.mkdirSync(distDir, { recursive: true });
  const entry = path.join(distDir, 'index.js');
  fs.writeFileSync(entry, 'export const ok = true;');

  // File path → that exact file.
  const fromFile = resolvePiSdkUrl({ sdkPath: entry, env: {} });
  assert.equal(fromFile, pathToFileURL(entry).href);

  // Package dir → dist/index.js inside it.
  const fromDir = resolvePiSdkUrl({ sdkPath: dir, env: {} });
  assert.equal(fromDir, pathToFileURL(entry).href);
});

test('resolvePiSdkUrl finds a colocated SDK whose exports only define import', async (t) => {
  const { resolvePiSdkUrl } = await chatAgent();
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-sdk-tree-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const agentDir = path.join(root, 'node_modules', '@rasmusjosefsson', 'agent-qa');
  const sdkDir = path.join(agentDir, 'node_modules', '@earendil-works', 'pi-coding-agent');
  const entry = path.join(sdkDir, 'dist', 'index.js');
  fs.mkdirSync(path.dirname(entry), { recursive: true });
  fs.writeFileSync(entry, 'export const ok = true;');
  fs.writeFileSync(
    path.join(sdkDir, 'package.json'),
    JSON.stringify({
      name: '@earendil-works/pi-coding-agent',
      type: 'module',
      exports: { '.': { import: './dist/index.js' } },
    }),
  );

  const moduleUrl = pathToFileURL(path.join(agentDir, 'lib', 'chat-agent.mjs')).href;
  const resolved = resolvePiSdkUrl({ env: { PATH: '' }, moduleUrl });
  assert.equal(resolved, pathToFileURL(entry).href);
});

test('__internal helpers: toExistingEntry + whichOnPath', async () => {
  const { __internal } = await chatAgent();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-which-'));
  // whichOnPath
  const exe = path.join(dir, 'pi');
  fs.writeFileSync(exe, '#!/bin/sh\n');
  assert.equal(__internal.whichOnPath('pi', { PATH: dir }), exe);
  assert.equal(__internal.whichOnPath('nope', { PATH: dir }), null);
  // toExistingEntry
  assert.equal(__internal.toExistingEntry(path.join(dir, 'missing')), null);
  assert.equal(__internal.toExistingEntry(exe), exe);
});

// ---- agent-qa awareness (primer + skill discovery) ----

test('AGENT_QA_PRIMER tells the agent it is in agent-qa, vendor-neutrally', async () => {
  const { __internal } = await chatAgent();
  const p = __internal.AGENT_QA_PRIMER;
  assert.match(p, /agent-qa workbench/);
  assert.match(p, /AGENT_BROWSER_SESSION/); // points at the bound session
  assert.match(p, /agent-qa skills get core/); // bootstraps the skill, like the terminal
  assert.match(p, /Never guess a/i); // host/route discipline
  // Self-serve persona sign-in (no manual Connect click required).
  assert.match(p, /AGENT_QA_BASE/);
  assert.match(p, /\/connect/);
  assert.match(p, /default/i);
  // Stays generic: no vendor/product names baked into our code.
  assert.doesNotMatch(p, /outreach/i);
});

test('resolveAgentQaSkillDirs reads [skills] extra-dirs from a walked-up agent-qa.toml', async () => {
  const { __internal } = await chatAgent();
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-skilldirs-'));
  const skillA = path.join(root, 'skills-a');
  const skillB = path.join(root, 'pkgs', 'overlay');
  fs.mkdirSync(skillA, { recursive: true });
  fs.mkdirSync(skillB, { recursive: true });
  const missing = path.join(root, 'gone');
  fs.writeFileSync(
    path.join(root, 'agent-qa.toml'),
    [
      '[skills]',
      'extra-dirs = [',
      `  ${JSON.stringify(skillA)},`,
      `  ${JSON.stringify(skillB)},`,
      `  ${JSON.stringify(missing)},`, // non-existent → filtered out
      ']',
      '',
      '[plugins]',
      'auth = "/somewhere/agent-qa-plugin-x"',
      '',
    ].join('\n'),
  );
  // cwd a couple levels below the toml → walked-up discovery finds it.
  const cwd = path.join(root, 'a', 'b');
  fs.mkdirSync(cwd, { recursive: true });
  const dirs = __internal.resolveAgentQaSkillDirs(cwd);
  assert.ok(dirs.includes(skillA), 'existing dir A present');
  assert.ok(dirs.includes(skillB), 'existing dir B present');
  assert.ok(!dirs.includes(missing), 'non-existent dir filtered out');
});

// ---- /api/chat/* routes ----

function makeStubHub() {
  const calls = { prompt: [], abort: 0, newSession: 0, subscribe: 0, model: [], thinking: [] };
  return {
    calls,
    subscribe(res) {
      calls.subscribe++;
      res.write(': connected\n\n');
    },
    unsubscribe() {},
    async prompt(text, opts) {
      calls.prompt.push({ text, opts });
    },
    async abort() {
      calls.abort++;
    },
    async newSession() {
      calls.newSession++;
    },
    async setModel(sel) {
      calls.model.push(sel);
      return { started: true, streaming: false, messages: [], model: { id: sel.id }, thinkingLevels: [] };
    },
    async setThinkingLevel(level) {
      calls.thinking.push(level);
      return { started: true, streaming: false, messages: [], thinkingLevel: level };
    },
    async getState() {
      return { started: true, streaming: false, messages: [], model: { id: 'm' }, models: [], thinkingLevels: [] };
    },
    dispose() {},
  };
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

test('chat routes drive the hub', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } });
  t.after(() => server.close());

  await t.test('GET /api/chat/state returns available + state', async () => {
    const res = await fetch(`${base}/api/chat/state`);
    assert.equal(res.status, 200);
    const body = await res.json();
    assert.equal(body.available, true);
    assert.equal(body.started, true);
    assert.equal(body.model.id, 'm');
  });

  await t.test('POST /api/chat/prompt accepts and routes text', async () => {
    const res = await fetch(`${base}/api/chat/prompt`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: 'do a thing', streamingBehavior: 'steer' }),
    });
    assert.equal(res.status, 202);
    assert.equal(hub.calls.prompt.length, 1);
    assert.equal(hub.calls.prompt[0].text, 'do a thing');
    assert.equal(hub.calls.prompt[0].opts.streamingBehavior, 'steer');
  });

  await t.test('POST /api/chat/prompt rejects empty text', async () => {
    const res = await fetch(`${base}/api/chat/prompt`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ text: '   ' }),
    });
    assert.equal(res.status, 400);
  });

  await t.test('POST /api/chat/abort and /new route through', async () => {
    const a = await fetch(`${base}/api/chat/abort`, { method: 'POST' });
    assert.equal(a.status, 200);
    assert.equal(hub.calls.abort, 1);
    const n = await fetch(`${base}/api/chat/new`, { method: 'POST' });
    assert.equal(n.status, 200);
    assert.equal(hub.calls.newSession, 1);
  });

  await t.test('GET /api/chat/stream opens an SSE channel', async () => {
    const res = await fetch(`${base}/api/chat/stream`);
    assert.equal(res.status, 200);
    assert.equal(res.headers.get('content-type'), 'text/event-stream');
    assert.ok(hub.calls.subscribe >= 1);
    // Read a little then bail so the connection doesn't hang the test.
    const reader = res.body.getReader();
    const { value } = await reader.read();
    assert.match(Buffer.from(value).toString('utf8'), /connected/);
    await reader.cancel();
  });
});

test('chat is unavailable when no SDK/config is wired', async (t) => {
  const { server, base } = await boot('/tmp/whatever', {});
  t.after(() => server.close());

  const state = await fetch(`${base}/api/chat/state`);
  assert.equal(state.status, 200);
  const body = await state.json();
  assert.equal(body.available, false);

  const prompt = await fetch(`${base}/api/chat/prompt`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ text: 'hi' }),
  });
  assert.equal(prompt.status, 503);
});

test('chat reports unavailable when the hub factory throws', async (t) => {
  const { server, base } = await boot('/tmp/whatever', {
    chat: {
      createHub: () => {
        throw new Error('SDK not found');
      },
    },
  });
  t.after(() => server.close());

  const state = await fetch(`${base}/api/chat/state`);
  const body = await state.json();
  assert.equal(body.available, false);
  assert.match(body.reason, /SDK not found/);

  const prompt = await fetch(`${base}/api/chat/prompt`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ text: 'hi' }),
  });
  assert.equal(prompt.status, 503);
});

test('GET /api/root advertises chat availability', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } });
  t.after(() => server.close());
  const res = await fetch(`${base}/api/root`);
  const body = await res.json();
  assert.equal(body.chat, true);
});

test('GET /api/chat/browser-stream subscribes the live screencast bridge', async (t) => {
  const hub = makeStubHub();
  const seen = [];
  const fakeBridge = {
    subscribe: (res) => res.write('data: {"data":"BBBB"}\n\n'),
    unsubscribe: () => {},
  };
  const { server, base } = await boot('/tmp/whatever', {
    chat: { hub },
    liveForSession: (session) => {
      seen.push(session);
      return fakeBridge;
    },
  });
  t.after(() => server.close());

  const res = await fetch(`${base}/api/chat/browser-stream`);
  assert.equal(res.status, 200);
  assert.match(res.headers.get('content-type'), /text\/event-stream/);
  const reader = res.body.getReader();
  const chunk = await reader.read();
  assert.match(Buffer.from(chunk.value).toString('utf8'), /BBBB/);
  await reader.cancel();
  // Defaults to the agent-browser "default" session.
  assert.deepEqual(seen, ['default']);
});

test('GET /api/chat/browser-stream honors ?session and rejects unsafe names', async (t) => {
  const hub = makeStubHub();
  const seen = [];
  const { server, base } = await boot('/tmp/whatever', {
    chat: { hub },
    liveForSession: (session) => {
      seen.push(session);
      return { subscribe: (res) => res.write(': ok\n\n'), unsubscribe: () => {} };
    },
  });
  t.after(() => server.close());

  const ok = await fetch(`${base}/api/chat/browser-stream?session=replay-abc`);
  assert.equal(ok.status, 200);
  await ok.body.getReader().cancel();
  assert.deepEqual(seen, ['replay-abc']);

  const bad = await fetch(`${base}/api/chat/browser-stream?session=../etc`);
  assert.equal(bad.status, 400);
});

test('GET /api/chat/browser-stream without a CLI runner → 503', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } }); // no liveForSession
  t.after(() => server.close());
  const res = await fetch(`${base}/api/chat/browser-stream`);
  assert.equal(res.status, 503);
});

test('GET /api/root advertises liveBrowser when a session bridge exists', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', {
    chat: { hub },
    liveForSession: () => ({ subscribe: () => {}, unsubscribe: () => {} }),
  });
  t.after(() => server.close());
  const res = await fetch(`${base}/api/root`);
  const body = await res.json();
  assert.equal(body.liveBrowser, true);
});

test('POST /api/chat/model forwards the selection to hub.setModel', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } });
  t.after(() => server.close());
  const res = await fetch(`${base}/api/chat/model`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ provider: 'github-copilot', id: 'gpt-5-mini' }),
  });
  assert.equal(res.status, 200);
  const body = await res.json();
  assert.equal(body.ok, true);
  assert.deepEqual(hub.calls.model, [{ provider: 'github-copilot', id: 'gpt-5-mini' }]);
});

test('POST /api/chat/thinking forwards the level to hub.setThinkingLevel', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } });
  t.after(() => server.close());
  const res = await fetch(`${base}/api/chat/thinking`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ level: 'high' }),
  });
  assert.equal(res.status, 200);
  assert.deepEqual(hub.calls.thinking, ['high']);
});

test('POST /api/chat/browser-navigate drives the live bridge', async (t) => {
  const hub = makeStubHub();
  const inputs = [];
  const { server, base } = await boot('/tmp/whatever', {
    chat: { hub },
    liveForSession: () => ({
      subscribe: () => {},
      unsubscribe: () => {},
      input: (evt) => {
        inputs.push(evt);
        return true;
      },
    }),
  });
  t.after(() => server.close());

  const nav = await fetch(`${base}/api/chat/browser-navigate`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url: 'example.com' }),
  });
  assert.equal(nav.status, 200);
  // bare host gets https:// and is dispatched as a navigate event
  assert.deepEqual(inputs, [{ type: 'navigate', url: 'https://example.com' }]);

  const back = await fetch(`${base}/api/chat/browser-navigate`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ action: 'back' }),
  });
  assert.equal(back.status, 200);
  assert.deepEqual(inputs[1], { type: 'back' });
});

test('POST /api/chat/browser-navigate → 409 when the bridge is not connected', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', {
    chat: { hub },
    liveForSession: () => ({ subscribe: () => {}, unsubscribe: () => {}, input: () => false }),
  });
  t.after(() => server.close());
  const res = await fetch(`${base}/api/chat/browser-navigate`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url: 'https://example.com' }),
  });
  assert.equal(res.status, 409);
});

test('POST /api/chat/browser-navigate without a CLI runner → 503', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } }); // no liveForSession
  t.after(() => server.close());
  const res = await fetch(`${base}/api/chat/browser-navigate`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ url: 'https://example.com' }),
  });
  assert.equal(res.status, 503);
});

test('multi-chat: list / create / per-chat routes / delete', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } });
  t.after(() => server.close());

  // no chats until one is created
  let list = await (await fetch(`${base}/api/chat/list`)).json();
  assert.deepEqual(list.chats, []);

  // create two chats with distinct ids + distinct browser sessions
  const c1 = await (await fetch(`${base}/api/chat/create`, { method: 'POST' })).json();
  const c2 = await (await fetch(`${base}/api/chat/create`, { method: 'POST' })).json();
  assert.match(c1.id, /^[0-9a-f]+$/);
  assert.notEqual(c1.id, c2.id);
  assert.match(c1.session, /^chat-[0-9a-f]+$/);
  assert.notEqual(c1.session, c2.session);

  list = await (await fetch(`${base}/api/chat/list`)).json();
  assert.equal(list.chats.length, 2);

  // per-chat active-session returns that chat's own session (not recording)
  const as1 = await (await fetch(`${base}/api/chat/c/${c1.id}/active-session`)).json();
  assert.equal(as1.session, c1.session);
  assert.equal(as1.recording, false);
  const connection = await (await fetch(`${base}/api/chat/c/${c1.id}/connection`)).json();
  assert.deepEqual(connection, {
    state: 'disconnected',
    personaId: null,
    environmentId: null,
    profile: null,
  });

  // per-chat prompt routes through the chat's hub
  const p = await fetch(`${base}/api/chat/c/${c1.id}/prompt`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ text: 'hi there' }),
  });
  assert.equal(p.status, 202);
  assert.equal(hub.calls.prompt.at(-1).text, 'hi there');

  // unknown chat id → 404
  assert.equal((await fetch(`${base}/api/chat/c/deadbeef/active-session`)).status, 404);

  // delete a chat → list shrinks
  const d = await fetch(`${base}/api/chat/c/${c1.id}/delete`, { method: 'POST' });
  assert.equal(d.status, 200);
  list = await (await fetch(`${base}/api/chat/list`)).json();
  assert.equal(list.chats.length, 1);
  assert.equal(list.chats[0].id, c2.id);
});

test('flat chat routes operate on the auto-created primary chat', async (t) => {
  const hub = makeStubHub();
  const { server, base } = await boot('/tmp/whatever', { chat: { hub } });
  t.after(() => server.close());

  // hitting a flat route auto-creates the primary chat
  const st = await fetch(`${base}/api/chat/state`);
  assert.equal(st.status, 200);
  const list = await (await fetch(`${base}/api/chat/list`)).json();
  assert.equal(list.chats.length, 1);
});

test('multi-chat: per-chat live recording view + artifacts', async (t) => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'aqa-rec-'));
  const scenariosRoot = path.join(tmp, 'scenarios');
  const recordRoot = path.join(tmp, 'record');
  fs.mkdirSync(scenariosRoot, { recursive: true });
  fs.mkdirSync(recordRoot, { recursive: true });
  const hub = makeStubHub();
  const { server, base } = await boot(scenariosRoot, { chat: { hub }, recordRoot });
  t.after(() => {
    server.close();
    fs.rmSync(tmp, { recursive: true, force: true });
  });

  const c = await (await fetch(`${base}/api/chat/create`, { method: 'POST' })).json();

  // no recording yet → empty, not recording
  let rec = await (await fetch(`${base}/api/chat/c/${c.id}/recording`)).json();
  assert.equal(rec.recording, false);
  assert.equal(rec.sid, null);
  assert.deepEqual(rec.steps, []);

  // simulate an in-progress recording in this chat's record scratch
  const sid = 's-test__abc123';
  const chatRecDir = path.join(recordRoot, c.session);
  fs.mkdirSync(chatRecDir, { recursive: true });
  fs.writeFileSync(
    path.join(chatRecDir, 'recorder-state.json'),
    JSON.stringify({ sid, intent: 'probe form', session: c.session, baseline: 'fresh', startedAt: '2026-01-01T00:00:00.000Z', steps: [{ id: 's0', intent: 'open page', kind: 'do', verb: 'goto', value: { from: 'literal', literal: 'https://example.com/' } }] })
  );
  const shotDir = path.join(scenariosRoot, sid, 'recording', 'screenshots');
  fs.mkdirSync(shotDir, { recursive: true });
  fs.writeFileSync(path.join(shotDir, 's0.png'), Buffer.from('89504e470d0a1a0a', 'hex'));

  rec = await (await fetch(`${base}/api/chat/c/${c.id}/recording`)).json();
  assert.equal(rec.recording, true); // not flushed yet
  assert.equal(rec.sid, sid);
  assert.equal(rec.intent, 'probe form');
  assert.equal(rec.session, c.session);
  assert.equal(rec.flushed, false);
  assert.equal(rec.steps.length, 1);
  assert.equal(rec.steps[0].kind, 'do');

  // screenshot artifact serves as image/png
  const shot = await fetch(`${base}/api/chat/c/${c.id}/recording/step/s0/screenshot`);
  assert.equal(shot.status, 200);
  assert.equal(shot.headers.get('content-type'), 'image/png');

  // missing step → 404; path traversal → rejected
  assert.equal((await fetch(`${base}/api/chat/c/${c.id}/recording/step/s9/screenshot`)).status, 404);
  const esc = await fetch(
    `${base}/api/chat/c/${c.id}/recording/step/${encodeURIComponent('../../x')}/screenshot`
  );
  assert.ok(esc.status === 400 || esc.status === 404);

  // once flushed (scenario.json exists) → recording=false, flushed=true
  fs.writeFileSync(path.join(scenariosRoot, sid, 'scenario.json'), JSON.stringify({ steps: [] }));
  rec = await (await fetch(`${base}/api/chat/c/${c.id}/recording`)).json();
  assert.equal(rec.flushed, true);
  assert.equal(rec.recording, false);

  // The panel keeps showing the saved scenario after the active state is removed.
  fs.unlinkSync(path.join(chatRecDir, 'recorder-state.json'));
  fs.writeFileSync(path.join(chatRecDir, 'scenario.last'), sid + '\n');
  fs.writeFileSync(
    path.join(scenariosRoot, sid, 'scenario.json'),
    JSON.stringify({
      intent: 'probe form',
      id: sid,
      steps: [
        {
          id: 's0',
          kind: 'do',
          verb: 'goto',
          intent: 'navigate to https://example.com/',
          value: { from: 'literal', literal: 'https://example.com/' },
        },
      ],
    })
  );
  rec = await (await fetch(`${base}/api/chat/c/${c.id}/recording`)).json();
  assert.equal(rec.sid, sid);
  assert.equal(rec.flushed, true);
  assert.equal(rec.recording, false);
  assert.equal(rec.intent, 'probe form');
  assert.equal(rec.steps.length, 1);
  assert.equal(rec.steps[0].kind, 'do');
  // the keyframe still serves via the scenario.last fallback
  const shot2 = await fetch(`${base}/api/chat/c/${c.id}/recording/step/s0/screenshot`);
  assert.equal(shot2.status, 200);
});
