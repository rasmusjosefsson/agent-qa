'use strict';
// Tests for the live-browser bridge (lib/live-bridge.js).
//
// The bridge connects to a Chrome CDP page target, polls captureScreenshot,
// broadcasts frames to SSE subscribers, and forwards normalized input as
// CSS-pixel CDP events. These tests drive it with a FAKE WebSocket + fetch
// so there is no real browser — the assertions lock the CDP wire contract
// (which methods, which coordinate math) the editor depends on.

const test = require('node:test');
const assert = require('node:assert/strict');

const { createLiveBridge } = require('../lib/live-bridge.js');

// A fake CDP socket. Records everything sent; lets the test drive open/
// message/close and inspect the JSON-RPC frames.
class FakeWS {
  constructor(url) {
    this.url = url;
    this.readyState = 0;
    this.sent = [];
    this._lis = {};
    FakeWS.instances.push(this);
  }
  addEventListener(t, f) {
    (this._lis[t] || (this._lis[t] = [])).push(f);
  }
  _emit(t, e) {
    (this._lis[t] || []).forEach((f) => f(e));
  }
  fireOpen() {
    this.readyState = 1;
    this._emit('open', {});
  }
  send(s) {
    this.sent.push(JSON.parse(s));
  }
  recv(obj) {
    this._emit('message', { data: JSON.stringify(obj) });
  }
  close() {
    this.readyState = 3;
    this._emit('close', {});
  }
  sentMethod(method) {
    return this.sent.find((m) => m.method === method);
  }
}
FakeWS.instances = [];

const fakeFetch = async () => ({
  json: async () => [
    { type: 'page', url: 'https://start.example/', webSocketDebuggerUrl: 'ws://h:1/devtools/page/abc' },
  ],
});

const flush = () => new Promise((r) => setTimeout(r, 0));

function makeBridge() {
  FakeWS.instances = [];
  return createLiveBridge({
    getCdpUrl: async () => 'ws://127.0.0.1:1/devtools/browser/x',
    WebSocketImpl: FakeWS,
    fetchImpl: fakeFetch,
    captureMs: 100000, // never auto-fire; the immediate first frame is enough
    downloadHoldMs: 0, // clicks commit immediately — download window covered by its own tests
  });
}

function makeRecordingBridge(onRecord) {
  FakeWS.instances = [];
  return createLiveBridge({
    getCdpUrl: async () => 'ws://127.0.0.1:1/devtools/browser/x',
    WebSocketImpl: FakeWS,
    fetchImpl: fakeFetch,
    captureMs: 100000,
    onRecord,
    downloadHoldMs: 0,
  });
}

// Connect helper: subscribe, let the socket construct, fire open, await.
async function connect(bridge, res) {
  const p = bridge.subscribe(res);
  await flush(); // getCdpUrl + findPageWs resolve, FakeWS constructed
  const sock = FakeWS.instances.at(-1);
  sock.fireOpen();
  await p;
  return sock;
}

test('subscribe connects to the page target and enables Page domain', async () => {
  const bridge = makeBridge();
  const frames = [];
  const res = { write: (s) => frames.push(s), end() {} };
  const sock = await connect(bridge, res);

  assert.ok(sock.sentMethod('Page.enable'), 'enables Page');
  assert.ok(sock.sentMethod('Page.getLayoutMetrics'), 'asks for the CSS viewport');
  assert.ok(sock.sentMethod('Page.captureScreenshot'), 'requests an immediate first frame');
  assert.equal(bridge.subscriberCount, 1);
});

test('a captured frame is broadcast to subscribers as an SSE data line', async () => {
  const bridge = makeBridge();
  const frames = [];
  const res = { write: (s) => frames.push(s), end() {} };
  const sock = await connect(bridge, res);

  const capId = sock.sent.find((m) => m.method === 'Page.captureScreenshot').id;
  sock.recv({ id: capId, result: { data: 'JPEGBASE64' } });

  const frame = frames.find((f) => f.includes('JPEGBASE64'));
  assert.ok(frame, 'frame reached the subscriber');
  assert.match(frame, /^data: /);
  assert.deepEqual(JSON.parse(frame.slice(6)), { data: 'JPEGBASE64' });
});

test('input maps normalized coords to CSS pixels using the layout viewport', async () => {
  const bridge = makeBridge();
  const sock = await connect(bridge, { write() {}, end() {} });

  // Feed the viewport size: 800 x 600 CSS px.
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });

  sock.sent.length = 0; // isolate the click's events
  const ok = bridge.input({ type: 'click', nx: 0.5, ny: 0.25 });
  assert.equal(ok, true);

  const press = sock.sent.find((m) => m.params && m.params.type === 'mousePressed');
  assert.equal(press.method, 'Input.dispatchMouseEvent');
  assert.equal(press.params.x, 400); // 0.5 * 800
  assert.equal(press.params.y, 150); // 0.25 * 600
  assert.equal(press.params.button, 'left');
  // press + release + the preceding move
  assert.ok(sock.sent.some((m) => m.params.type === 'mouseMoved'));
  assert.ok(sock.sent.some((m) => m.params.type === 'mouseReleased'));
});

test('a click refuses (no events) until the viewport size is known', async () => {
  const bridge = makeBridge();
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.sent.length = 0;
  // No getLayoutMetrics reply yet → css unknown → refuse rather than misclick.
  const ok = bridge.input({ type: 'click', nx: 0.5, ny: 0.5 });
  assert.equal(ok, false);
  assert.equal(sock.sent.length, 0);
});

test('printable keys go through as char; named keys as keyDown/up', async () => {
  const bridge = makeBridge();
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.sent.length = 0;

  bridge.input({ type: 'key', text: 'a' });
  const ch = sock.sent.find((m) => m.params.type === 'char');
  assert.equal(ch.params.text, 'a');

  sock.sent.length = 0;
  bridge.input({ type: 'key', key: 'Enter' });
  assert.ok(sock.sent.some((m) => m.params.type === 'keyDown' && m.params.key === 'Enter'));
  assert.ok(sock.sent.some((m) => m.params.type === 'keyUp' && m.params.key === 'Enter'));
});

test('input is a no-op (false) when not connected', () => {
  const bridge = makeBridge();
  assert.equal(bridge.input({ type: 'click', nx: 0.5, ny: 0.5 }), false);
});

test('pick hit-tests a point and resolves the accessible role + name', async () => {
  const bridge = makeBridge();
  const sock = await connect(bridge, { write() {}, end() {} });
  // Viewport so coords resolve.
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 1000, clientHeight: 500 } } });

  const p = bridge.pick(0.3, 0.4);
  await flush();
  // DOM.getNodeForLocation → backendNodeId
  const locReq = sock.sent.find((m) => m.method === 'DOM.getNodeForLocation');
  assert.equal(locReq.params.x, 300); // 0.3 * 1000
  assert.equal(locReq.params.y, 200); // 0.4 * 500
  sock.recv({ id: locReq.id, result: { backendNodeId: 42 } });
  await flush();
  // Accessibility.getPartialAXTree → role + name
  const axReq = sock.sent.find((m) => m.method === 'Accessibility.getPartialAXTree');
  assert.equal(axReq.params.backendNodeId, 42);
  sock.recv({
    id: axReq.id,
    result: { nodes: [{ role: { value: 'button' }, name: { value: 'Sign in' } }] },
  });
  await flush();
  const boxReq = sock.sent.find((m) => m.method === 'DOM.getBoxModel');
  assert.equal(boxReq.params.backendNodeId, 42);
  sock.recv({ id: boxReq.id, result: { model: { content: [0, 0, 100, 0, 100, 50, 0, 50] } } });
  const el = await p;
  assert.equal(el.role, 'button');
  assert.equal(el.name, 'Sign in');
  assert.ok(el.box, 'a normalized bounding box is returned');
});

test('auto-record: a click with record:true emits a direct do recordable event', async () => {
  const bridge = makeBridge();
  const events = [];
  const res = { write: (s) => events.push(s), end() {} };
  const sock = await connect(bridge, res);
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });

  bridge.input({ type: 'click', nx: 0.5, ny: 0.5, record: true });
  // The element is hit-tested BEFORE the click is dispatched (so a navigating
  // click can still be resolved), then the click fires and it records.
  await flush();
  const loc = sock.sent.find((m) => m.method === 'DOM.getNodeForLocation');
  sock.recv({ id: loc.id, result: { backendNodeId: 7 } });
  await flush();
  const ax = sock.sent.find((m) => m.method === 'Accessibility.getPartialAXTree');
  sock.recv({ id: ax.id, result: { nodes: [{ role: { value: 'link' }, name: { value: 'Details' } }] } });
  await flush();
  const box = sock.sent.find((m) => m.method === 'DOM.getBoxModel');
  if (box) sock.recv({ id: box.id, result: { model: { content: [0, 0, 10, 0, 10, 10, 0, 10] } } });
  await flush();

  // The click is dispatched after the hit-test...
  assert.ok(sock.sent.some((m) => m.method === 'Input.dispatchMouseEvent'), 'click dispatched');
  // ...and a direct click step is recorded.
  const rec = events.find((e) => e.includes('event: recordable'));
  assert.ok(rec, 'a recordable event was broadcast');
  assert.ok(rec.includes('"kind":"do"') && rec.includes('"verb":"click"') && rec.includes('Details'));
});

test('auto-record ignores clicks on non-interactive elements (e.g. a heading)', async () => {
  const bridge = makeBridge();
  const events = [];
  const res = { write: (s) => events.push(s), end() {} };
  const sock = await connect(bridge, res);
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });

  bridge.input({ type: 'click', nx: 0.5, ny: 0.2, record: true });
  await flush();
  const loc = sock.sent.find((m) => m.method === 'DOM.getNodeForLocation');
  sock.recv({ id: loc.id, result: { backendNodeId: 9 } });
  await flush();
  const ax = sock.sent.find((m) => m.method === 'Accessibility.getPartialAXTree');
  sock.recv({ id: ax.id, result: { nodes: [{ role: { value: 'heading' }, name: { value: 'Welcome' } }] } });
  await flush();
  const box = sock.sent.find((m) => m.method === 'DOM.getBoxModel');
  if (box) sock.recv({ id: box.id, result: { model: { content: [0, 0, 10, 0, 10, 10, 0, 10] } } });
  await flush();

  // Click still went through, but nothing was recorded.
  assert.ok(sock.sent.some((m) => m.method === 'Input.dispatchMouseEvent'), 'click dispatched');
  assert.ok(
    !events.some((e) => e.includes('event: recordable')),
    'a heading click is not recorded',
  );
});

test('with a server recorder, a click records ONCE (no per-tab recordable)', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (kind, payload) => recorded.push({ kind, payload }));
  const events = [];
  const res = { write: (s) => events.push(s), end() {} };
  const sock = await connect(bridge, res);
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });

  bridge.input({ type: 'click', nx: 0.5, ny: 0.5, record: true });
  await flush();
  const loc = sock.sent.find((m) => m.method === 'DOM.getNodeForLocation');
  sock.recv({ id: loc.id, result: { backendNodeId: 3 } });
  await flush();
  const ax = sock.sent.find((m) => m.method === 'Accessibility.getPartialAXTree');
  sock.recv({ id: ax.id, result: { nodes: [{ role: { value: 'button' }, name: { value: 'Save' } }] } });
  await flush();
  const box = sock.sent.find((m) => m.method === 'DOM.getBoxModel');
  if (box) sock.recv({ id: box.id, result: { model: { content: [0, 0, 10, 0, 10, 10, 0, 10] } } });
  await flush();

  // Recorded exactly once, server-side; tabs get a buffer-changed refresh,
  // and no per-tab 'recordable' event is emitted (which is what duplicated).
  assert.equal(recorded.length, 1);
  assert.deepEqual(recorded[0], { kind: 'do', payload: { intent: 'click Save', verb: 'click', on: { role: 'button', name: 'Save' } } });
  assert.ok(events.some((e) => e.includes('event: buffer-changed')), 'tabs told to refresh');
  assert.ok(!events.some((e) => e.includes('event: recordable')), 'no client-side record path');
});

test('navigate adds a scheme to bare hosts and rejects non-http schemes', async () => {
  const bridge = makeBridge();
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.sent.length = 0;

  assert.equal(bridge.input({ type: 'navigate', url: 'example.org/login' }), true);
  const nav = sock.sentMethod('Page.navigate');
  assert.equal(nav.params.url, 'https://example.org/login');

  sock.sent.length = 0;
  assert.equal(bridge.input({ type: 'navigate', url: 'file:///etc/passwd' }), false);
  assert.equal(sock.sentMethod('Page.navigate'), undefined, 'file:// is refused');
});

test('the address bar tracks the page: initial url + frameNavigated', async () => {
  const bridge = makeBridge();
  const frames = [];
  const res = { write: (s) => frames.push(s), end() {} };
  const sock = await connect(bridge, res);

  // Initial url came from /json/list at connect.
  assert.equal(bridge.currentUrl, 'https://start.example/');
  assert.ok(frames.some((f) => f.includes('event: url') && f.includes('start.example')));

  // A main-frame navigation pushes a new url event.
  sock.recv({ method: 'Page.frameNavigated', params: { frame: { id: 'f1', url: 'https://next.example/' } } });
  assert.equal(bridge.currentUrl, 'https://next.example/');
  assert.ok(frames.some((f) => f.includes('next.example')));
});

test('auto-record installs the page change-listener binding at connect', async () => {
  const bridge = makeBridge();
  const sock = await connect(bridge, { write() {}, end() {} });
  assert.equal(sock.sentMethod('Runtime.enable') !== undefined, true);
  const bind = sock.sentMethod('Runtime.addBinding');
  assert.equal(bind.params.name, '__aqRecord');
  const inject = sock.sentMethod('Page.addScriptToEvaluateOnNewDocument');
  assert.ok(inject.params.source.includes('__aqRecInstalled'), 'idempotent listener injected');
  const evald = sock.sentMethod('Runtime.evaluate');
  assert.ok(evald.params.expression.includes('addEventListener'), 'current page gets the listener too');
});

test('auto-record maps a select change to a select step', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: { name: '__aqRecord', payload: JSON.stringify({ kind: 'select', name: 'Country', value: 'Sweden' }) },
  });
  await flush();
  assert.equal(recorded.length, 1);
  assert.deepEqual(recorded[0].payload, {
    intent: 'select Sweden in Country',
    verb: 'select',
    on: { role: 'combobox', name: 'Country' },
    value: { from: 'literal', literal: 'Sweden' },
  });
});

test('auto-record maps checkbox toggles to check/uncheck, not click', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: { name: '__aqRecord', payload: JSON.stringify({ kind: 'check', name: 'Agree', role: 'checkbox' }) },
  });
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: { name: '__aqRecord', payload: JSON.stringify({ kind: 'uncheck', name: 'Agree', role: 'checkbox' }) },
  });
  await flush();
  assert.equal(recorded.length, 2);
  assert.equal(recorded[0].payload.verb, 'check');
  assert.equal(recorded[1].payload.verb, 'uncheck');
  assert.deepEqual(recorded[0].payload.on, { role: 'checkbox', name: 'Agree' });
});

test('auto-record maps a file-input change to an upload step with basenames', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: {
      name: '__aqRecord',
      payload: JSON.stringify({
        kind: 'upload',
        name: 'Attachment',
        selector: '#file',
        files: ['a.txt', 'b.png'],
      }),
    },
  });
  await flush();
  assert.equal(recorded.length, 1);
  const p = recorded[0].payload;
  assert.equal(p.verb, 'upload');
  assert.deepEqual(p.on, { raw: { kind: 'css', value: '#file' }, reason: 'file input' });
  assert.deepEqual(p.value, { from: 'literal', literal: ['a.txt', 'b.png'] });
  assert.match(p.intent, /place the file/);
});

test('auto-record upload falls back to role+name when no selector', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: {
      name: '__aqRecord',
      payload: JSON.stringify({ kind: 'upload', name: 'Attachment', files: ['a.txt'] }),
    },
  });
  await flush();
  assert.equal(recorded.length, 1);
  const p = recorded[0].payload;
  assert.deepEqual(p.on, { role: 'button', name: 'Attachment' });
  assert.equal(p.value.literal, 'a.txt');
});

test('auto-record maps a contextmenu event to a rightclick step', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: {
      name: '__aqRecord',
      payload: JSON.stringify({
        kind: 'rightclick',
        name: 'Canvas',
        selector: '#canvas',
      }),
    },
  });
  await flush();
  assert.equal(recorded.length, 1);
  const p = recorded[0].payload;
  assert.equal(p.verb, 'rightclick');
  assert.deepEqual(p.on, { raw: { kind: 'css', value: '#canvas' }, reason: 'right-click target' });
});

test('auto-record rightclick still lands when the target has no name', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: {
      name: '__aqRecord',
      payload: JSON.stringify({ kind: 'rightclick', name: '', selector: 'div.icon-btn' }),
    },
  });
  await flush();
  assert.equal(recorded.length, 1);
  assert.equal(recorded[0].payload.verb, 'rightclick');
  assert.equal(recorded[0].payload.intent, 'right-click div.icon-btn');
});

test('auto-record reports a record-skip for a nameless change target', async () => {
  const bridge = makeBridge();
  const events = [];
  const res = { write: (s) => events.push(s), end() {} };
  const sock = await connect(bridge, res);
  sock.recv({
    method: 'Runtime.bindingCalled',
    params: { name: '__aqRecord', payload: JSON.stringify({ kind: 'select', name: '', value: 'x' }) },
  });
  await flush();
  const skip = events.find((e) => e.includes('event: record-skip'));
  assert.ok(skip, 'record-skip broadcast');
  assert.ok(!events.some((e) => e.includes('event: recordable')));
});

test('a checkbox click dispatches but is not click-recorded (change drives it)', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });

  bridge.input({ type: 'click', nx: 0.5, ny: 0.5, record: true });
  await flush();
  const loc = sock.sent.find((m) => m.method === 'DOM.getNodeForLocation');
  sock.recv({ id: loc.id, result: { backendNodeId: 5 } });
  await flush();
  const ax = sock.sent.find((m) => m.method === 'Accessibility.getPartialAXTree');
  sock.recv({ id: ax.id, result: { nodes: [{ role: { value: 'checkbox' }, name: { value: 'Agree' } }] } });
  await flush();
  const box = sock.sent.find((m) => m.method === 'DOM.getBoxModel');
  if (box) sock.recv({ id: box.id, result: { model: { content: [0, 0, 10, 0, 10, 10, 0, 10] } } });
  await flush();

  assert.ok(sock.sent.some((m) => m.method === 'Input.dispatchMouseEvent'), 'click dispatched');
  assert.equal(recorded.length, 0, 'no click record — the change event emits check/uncheck');
});

test('auto-record turns named keys (Tab/Escape/arrows) into press steps', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });

  bridge.input({ type: 'key', key: 'Tab', record: true });
  bridge.input({ type: 'key', key: 'Escape', record: true });
  bridge.input({ type: 'key', key: 'ArrowDown', record: true });
  // finalizeFill → Runtime.evaluate for the active field — answer each.
  for (let i = 0; i < 3; i++) {
    await flush();
    const ev = sock.sent.filter((m) => m.method === 'Runtime.evaluate').at(-1);
    if (ev) sock.recv({ id: ev.id, result: { result: { value: null } } });
  }
  await flush();
  await flush();

  const verbs = recorded.map((r) => r.payload.verb);
  const keys = recorded.map((r) => r.payload.value && r.payload.value.literal);
  assert.deepEqual(verbs, ['press', 'press', 'press']);
  assert.deepEqual(keys, ['Tab', 'Escape', 'ArrowDown']);
});

test('chorded keys dispatch with the modifiers bitmask and record press chords', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });

  bridge.input({ type: 'key', text: 'a', mods: { ctrl: true }, record: true });
  bridge.input({ type: 'key', key: 'Tab', mods: { shift: true }, record: true });
  bridge.input({ type: 'key', key: 's', mods: { ctrl: true, shift: true }, record: true });
  // Bare modifier keydowns dispatch nothing and record nothing.
  bridge.input({ type: 'key', key: 'Control', record: true });
  // Shift+char is typing, not a chord.
  bridge.input({ type: 'key', text: 'A', mods: { shift: true }, record: true });
  for (let i = 0; i < 6; i++) {
    await flush();
    const ev = sock.sent.filter((m) => m.method === 'Runtime.evaluate').at(-1);
    if (ev) sock.recv({ id: ev.id, result: { result: { value: null } } });
  }
  await flush();
  await flush();

  const downs = sock.sent.filter((m) => m.method === 'Input.dispatchKeyEvent' && m.params.type === 'keyDown');
  const ctrlA = downs.find((m) => m.params.key === 'a');
  assert.equal(ctrlA.params.modifiers, 2);
  assert.equal(ctrlA.params.code, 'KeyA');
  const shiftTab = downs.find((m) => m.params.key === 'Tab');
  assert.equal(shiftTab.params.modifiers, 8);
  assert.equal(shiftTab.params.code, 'Tab');
  assert.ok(!downs.some((m) => m.params.key === 'Control'), 'bare Control not dispatched');
  // 'A' with shift goes down the char path, not keyDown.
  const chars = sock.sent.filter((m) => m.method === 'Input.dispatchKeyEvent' && m.params.type === 'char');
  assert.ok(chars.some((m) => m.params.text === 'A'), 'shifted char still types');

  const presses = recorded.filter((r) => r.payload.verb === 'press').map((r) => r.payload.value.literal);
  assert.deepEqual(presses, ['Control+a', 'Shift+Tab', 'Control+Shift+s']);
});

test('previously unmapped named keys (Delete/F-keys) dispatch and record', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  bridge.input({ type: 'key', key: 'Delete', record: true });
  bridge.input({ type: 'key', key: 'F5', record: true });
  for (let i = 0; i < 4; i++) {
    await flush();
    const ev = sock.sent.filter((m) => m.method === 'Runtime.evaluate').at(-1);
    if (ev) sock.recv({ id: ev.id, result: { result: { value: null } } });
  }
  await flush();
  await flush();
  const downs = sock.sent.filter((m) => m.method === 'Input.dispatchKeyEvent' && m.params.type === 'keyDown');
  assert.ok(downs.some((m) => m.params.key === 'Delete' && m.params.windowsVirtualKeyCode === 46));
  assert.ok(downs.some((m) => m.params.key === 'F5' && m.params.windowsVirtualKeyCode === 116));
  assert.deepEqual(recorded.map((r) => r.payload.value.literal), ['Delete', 'F5']);
});

test('last unsubscribe closes the CDP socket', async () => {
  const bridge = makeBridge();
  const res = { write() {}, end() {} };
  const sock = await connect(bridge, res);
  assert.equal(sock.readyState, 1);
  bridge.unsubscribe(res);
  assert.equal(bridge.subscriberCount, 0);
  assert.equal(sock.readyState, 3, 'socket closed when the last viewer left');
});

test('a failed CDP connect surfaces a bridge-error SSE event', async () => {
  FakeWS.instances = [];
  const bridge = createLiveBridge({
    getCdpUrl: async () => {
      throw new Error('no live session');
    },
    WebSocketImpl: FakeWS,
    fetchImpl: fakeFetch,
  });
  const frames = [];
  await bridge.subscribe({ write: (s) => frames.push(s), end() {} });
  const err = frames.find((f) => f.includes('bridge-error'));
  assert.ok(err, 'subscriber told the bridge could not attach');
  assert.match(err, /no live session/);
  bridge.stop();
});

test('a subscriber reconnects when the browser opens after the initial CDP miss', async () => {
  FakeWS.instances = [];
  let attempts = 0;
  const bridge = createLiveBridge({
    getCdpUrl: async () => {
      attempts++;
      if (attempts === 1) throw new Error('no live session');
      return 'ws://127.0.0.1:1/devtools/browser/x';
    },
    WebSocketImpl: FakeWS,
    fetchImpl: fakeFetch,
    captureMs: 100000,
    reconnectMs: 10,
  });
  const frames = [];
  await bridge.subscribe({ write: (s) => frames.push(s), end() {} });
  assert.ok(frames.some((f) => f.includes('bridge-error')), 'initial miss is surfaced');

  await new Promise((resolve) => setTimeout(resolve, 30));
  const sock = FakeWS.instances.at(-1);
  assert.ok(sock, 'the bridge retried after the browser became available');
  sock.fireOpen();
  await flush();

  assert.equal(attempts, 2);
  assert.ok(sock.sentMethod('Page.captureScreenshot'), 'the retry starts frame capture');
  bridge.stop();
});

test('input drag: resolves both endpoints, runs the DOM gesture on the nodes, records a drag step', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (kind, payload) => recorded.push({ kind, payload }));
  const events = [];
  const res = { write: (s) => events.push(s), end() {} };
  const sock = await connect(bridge, res);
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });

  bridge.input({ type: 'drag', nx0: 0.1, ny0: 0.1, nx1: 0.5, ny1: 0.6, record: true });
  // Two endpoint picks run SEQUENTIALLY before the gesture dispatches —
  // answer the first endpoint's calls before the second is even sent.
  const answerPick = async (backendNodeId, role, name) => {
    await flush();
    const loc = sock.sent.filter((m) => m.method === 'DOM.getNodeForLocation').at(-1);
    sock.recv({ id: loc.id, result: { backendNodeId } });
    await flush();
    const ax = sock.sent.filter((m) => m.method === 'Accessibility.getPartialAXTree').at(-1);
    sock.recv({ id: ax.id, result: { nodes: [{ role: { value: role }, name: { value: name } }] } });
    await flush();
    const box = sock.sent.filter((m) => m.method === 'DOM.getBoxModel').at(-1);
    if (box) sock.recv({ id: box.id, result: { model: { content: [0, 0, 10, 0, 10, 10, 0, 10] } } });
    await flush();
    return loc;
  };
  const loc0 = await answerPick(11, 'listitem', 'Card A');
  const loc1 = await answerPick(22, 'list', 'Done');
  assert.deepEqual([loc0.params.x, loc0.params.y], [80, 60]);
  assert.deepEqual([loc1.params.x, loc1.params.y], [400, 360]);

  // Then the gesture: each endpoint's backendNodeId resolves to a JS object,
  // then one callFunctionOn runs the drag chain on src with dst as argument.
  for (const backendNodeId of [11, 22]) {
    const rs = sock.sent.filter((m) => m.method === 'DOM.resolveNode').at(-1);
    assert.equal(rs.params.backendNodeId, backendNodeId);
    sock.recv({ id: rs.id, result: { object: { objectId: `obj-${backendNodeId}` } } });
    await flush();
  }
  const fn = sock.sent.filter((m) => m.method === 'Runtime.callFunctionOn').at(-1);
  assert.equal(fn.params.objectId, 'obj-11');
  assert.equal(fn.params.arguments[0].objectId, 'obj-22');
  assert.match(fn.params.functionDeclaration, /dragstart|drop/);
  sock.recv({ id: fn.id, result: { result: { value: true } } });

  await flush();
  assert.equal(recorded.length, 1);
  assert.deepEqual(recorded[0].payload.verb, 'drag');
  assert.deepEqual(recorded[0].payload.on, { role: 'listitem', name: 'Card A' });
  assert.deepEqual(recorded[0].payload.params.to, { role: 'list', name: 'Done' });
});

test('input drag without record dispatches the DOM gesture on the endpoint nodes', async () => {
  const bridge = makeBridge();
  const sock = await connect(bridge, { write() {}, end() {} });
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });
  assert.equal(bridge.input({ type: 'drag', nx0: 0.2, ny0: 0.2, nx1: 0.6, ny1: 0.6 }), true);
  await flush();
  const locs = sock.sent.filter((m) => m.method === 'DOM.getNodeForLocation');
  assert.equal(locs.length, 1, 'hit-tests the source point first');
  sock.recv({ id: locs[0].id, result: { backendNodeId: 31 } });
  await flush();
  const loc2 = sock.sent.filter((m) => m.method === 'DOM.getNodeForLocation').at(-1);
  sock.recv({ id: loc2.id, result: { backendNodeId: 32 } });
  await flush();
  for (const backendNodeId of [31, 32]) {
    const rs = sock.sent.filter((m) => m.method === 'DOM.resolveNode').at(-1);
    assert.equal(rs.params.backendNodeId, backendNodeId);
    sock.recv({ id: rs.id, result: { object: { objectId: `obj-${backendNodeId}` } } });
    await flush();
  }
  const fn = sock.sent.filter((m) => m.method === 'Runtime.callFunctionOn').at(-1);
  assert.equal(fn.params.objectId, 'obj-31');
  assert.equal(fn.params.arguments[0].objectId, 'obj-32');
  assert.ok(
    !sock.sent.some((m) => m.method === 'Accessibility.getPartialAXTree'),
    'no a11y lookup when not recording',
  );
});

// --- state capture: dump page storage+cookies into a do/state step ---

test('captureState emits a do/state step with storage and cookies', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  assert.equal(bridge.input({ type: 'captureState' }), true);
  await flush();
  const evalId = sock.sent.find((m) => m.method === 'Runtime.evaluate' && m.params.expression.includes('localStorage')).id;
  sock.recv({
    id: evalId,
    result: {
      result: {
        value: {
          localStorage: { token: 'abc', theme: 'dark' },
          sessionStorage: { step: '2' },
          cookies: [{ name: 'sid', value: 'x%3Dy' }],
        },
      },
    },
  });
  await flush();
  assert.equal(recorded.length, 1);
  const step = recorded[0].payload;
  assert.equal(recorded[0].kind, 'do');
  assert.equal(step.verb, 'state');
  assert.deepEqual(step.params.localStorage, { token: 'abc', theme: 'dark' });
  assert.deepEqual(step.params.sessionStorage, { step: '2' });
  assert.deepEqual(step.params.cookies, [{ name: 'sid', value: 'x%3Dy' }]);
  assert.match(step.intent, /2 local, 1 session, 1 cookie/);
});

test('captureState skips empty stores and omits empty maps', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  bridge.input({ type: 'captureState' });
  await flush();
  const evalId = sock.sent.find((m) => m.method === 'Runtime.evaluate' && m.params.expression.includes('localStorage')).id;
  sock.recv({
    id: evalId,
    result: { result: { value: { localStorage: { a: '1' }, sessionStorage: {}, cookies: [] } } },
  });
  await flush();
  assert.equal(recorded.length, 1);
  assert.deepEqual(recorded[0].payload.params, { localStorage: { a: '1' } }, 'empty stores are omitted');
});

test('captureState on a stateless page reports a record-skip, not an empty step', async () => {
  const bridge = makeRecordingBridge(async () => {});
  const events = [];
  const sock = await connect(bridge, { write: (s) => events.push(s), end() {} });
  bridge.input({ type: 'captureState' });
  await flush();
  const evalId = sock.sent.find((m) => m.method === 'Runtime.evaluate' && m.params.expression.includes('localStorage')).id;
  sock.recv({ id: evalId, result: { result: { value: { localStorage: {}, sessionStorage: {}, cookies: [] } } } });
  await flush();
  assert.ok(events.some((e) => e.includes('event: record-skip') && e.includes('no storage or cookies')));
});

// --- native dialog capture: auto-accept + record the check/resolve pair ---

test('a javascriptDialogOpening records check+accept and answers the dialog', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Page.javascriptDialogOpening',
    params: { type: 'confirm', message: 'Delete this file?', url: 'https://x/' },
  });
  await flush();
  const answered = sock.sent.find((m) => m.method === 'Page.handleJavaScriptDialog');
  assert.ok(answered, 'the dialog is answered — the page unfreezes');
  assert.equal(answered.params.accept, true);
  await flush();
  assert.equal(recorded.length, 2, 'check then resolve, in order');
  const [check, resolve] = recorded;
  assert.equal(check.kind, 'check');
  assert.deepEqual(check.payload.claim, {
    subject: { dialog: true },
    predicate: 'contains',
    value: 'Delete this file?',
  });
  assert.equal(resolve.kind, 'do');
  assert.equal(resolve.payload.verb, 'dialog');
  assert.deepEqual(resolve.payload.params, { action: 'accept' });
});

test('a prompt dialog records its default as the prompt text', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Page.javascriptDialogOpening',
    params: { type: 'prompt', message: 'Your name?', defaultPrompt: 'ada', url: 'https://x/' },
  });
  await flush();
  const answered = sock.sent.find((m) => m.method === 'Page.handleJavaScriptDialog');
  assert.equal(answered.params.promptText, 'ada', 'prompt answered with the shown default');
  await flush();
  assert.equal(recorded.length, 2);
  assert.deepEqual(recorded[1].payload.params, { action: 'accept', text: 'ada' });
});

test('beforeunload dialogs are answered but not recorded', async () => {
  const recorded = [];
  const bridge = makeRecordingBridge(async (k, p) => recorded.push({ kind: k, payload: p }));
  const sock = await connect(bridge, { write() {}, end() {} });
  sock.recv({
    method: 'Page.javascriptDialogOpening',
    params: { type: 'beforeunload', message: 'Leave?', url: 'https://x/' },
  });
  await flush();
  assert.ok(sock.sent.some((m) => m.method === 'Page.handleJavaScriptDialog'));
  await flush();
  assert.equal(recorded.length, 0);
});

// -- download capture -------------------------------------------------------

// Drives a recorded click's whole pick() chain so the click lands held in the
// download-suppression window (or committed, when the hold is disabled).
async function driveRecordedClick(sock, bridge) {
  const metId = sock.sent.find((m) => m.method === 'Page.getLayoutMetrics').id;
  sock.recv({ id: metId, result: { cssLayoutViewport: { clientWidth: 800, clientHeight: 600 } } });
  bridge.input({ type: 'click', nx: 0.5, ny: 0.5, record: true });
  await flush();
  const loc = sock.sent.find((m) => m.method === 'DOM.getNodeForLocation');
  sock.recv({ id: loc.id, result: { backendNodeId: 7 } });
  await flush();
  const ax = sock.sent.find((m) => m.method === 'Accessibility.getPartialAXTree');
  sock.recv({
    id: ax.id,
    result: { nodes: [{ role: { value: 'link' }, name: { value: 'Get report' } }] },
  });
  await flush();
  const box = sock.sent.find((m) => m.method === 'DOM.getBoxModel');
  if (box) sock.recv({ id: box.id, result: { model: { content: [0, 0, 10, 0, 10, 10, 0, 10] } } });
  await flush();
}

test('recording bridge arms download capture on connect', async () => {
  const bridge = makeRecordingBridge(async () => {});
  await connect(bridge, { write() {}, end() {} });
  const sock = FakeWS.instances.at(-1);
  const dl = sock.sentMethod('Page.setDownloadBehavior');
  assert.ok(dl, 'setDownloadBehavior sent');
  assert.equal(dl.params.behavior, 'allowAndName');
  assert.ok(dl.params.downloadPath, 'a scratch dir is provided');
  bridge.stop();
});

test('a click that starts a download records do/download, not do/click', async () => {
  const recorded = [];
  FakeWS.instances = [];
  const bridge = createLiveBridge({
    getCdpUrl: async () => 'ws://127.0.0.1:1/devtools/browser/x',
    WebSocketImpl: FakeWS,
    fetchImpl: fakeFetch,
    captureMs: 100000,
    downloadHoldMs: 5000, // long enough that only the download event resolves it
    onRecord: async (kind, payload) => recorded.push({ kind, ...payload }),
  });
  const sock = await connect(bridge, { write() {}, end() {} });
  await driveRecordedClick(sock, bridge);

  // Download starts inside the suppression window.
  sock.recv({
    method: 'Page.downloadWillBegin',
    params: { guid: 'g1', url: 'https://x.example/report.pdf', suggestedFilename: 'report.pdf' },
  });
  await flush();

  assert.equal(recorded.length, 1);
  assert.equal(recorded[0].kind, 'do');
  assert.equal(recorded[0].verb, 'download');
  assert.deepEqual(recorded[0].on, { role: 'link', name: 'Get report' });
  assert.equal(recorded[0].value.literal, 'downloads/report.pdf');
  bridge.stop();
});

test('a download with no recorded click inside the window is skipped, not mis-recorded', async () => {
  const recorded = [];
  const events = [];
  FakeWS.instances = [];
  const bridge = createLiveBridge({
    getCdpUrl: async () => 'ws://127.0.0.1:1/devtools/browser/x',
    WebSocketImpl: FakeWS,
    fetchImpl: fakeFetch,
    captureMs: 100000,
    downloadHoldMs: 5000,
    onRecord: async (kind, payload) => recorded.push({ kind, ...payload }),
  });
  const sock = await connect(bridge, { write: (s) => events.push(s), end() {} });

  sock.recv({
    method: 'Page.downloadWillBegin',
    params: { guid: 'g2', url: 'https://x.example/x.zip', suggestedFilename: 'x.zip' },
  });
  await flush();

  assert.equal(recorded.length, 0, 'no step recorded for an untriggered download');
  assert.ok(
    events.some((e) => e.includes('record-skip')),
    'a record-skip notice explains the skip',
  );
  bridge.stop();
});

test('a late download after the click committed still records the step and flags the duplicate', async () => {
  const recorded = [];
  const events = [];
  FakeWS.instances = [];
  const bridge = createLiveBridge({
    getCdpUrl: async () => 'ws://127.0.0.1:1/devtools/browser/x',
    WebSocketImpl: FakeWS,
    fetchImpl: fakeFetch,
    captureMs: 100000,
    downloadHoldMs: 0, // click commits immediately
    onRecord: async (kind, payload) => recorded.push({ kind, ...payload }),
  });
  const sock = await connect(bridge, { write: (s) => events.push(s), end() {} });
  await driveRecordedClick(sock, bridge);
  assert.equal(recorded.length, 1);
  assert.equal(recorded[0].verb, 'click', 'the click committed as a plain click');

  // The response head arrives late — inside the 3s orphan window.
  sock.recv({
    method: 'Page.downloadWillBegin',
    params: { guid: 'g3', url: 'https://x.example/r.pdf', suggestedFilename: 'r.pdf' },
  });
  await flush();

  assert.equal(recorded.length, 2);
  assert.equal(recorded[1].verb, 'download');
  assert.equal(recorded[1].value.literal, 'downloads/r.pdf');
  assert.ok(
    events.some((e) => e.includes('record-skip') && e.includes('duplicate')),
    'the duplicate click trigger is flagged',
  );
  bridge.stop();
});
