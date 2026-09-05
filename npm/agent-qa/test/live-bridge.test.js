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
