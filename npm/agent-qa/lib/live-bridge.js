'use strict';
// Live-browser bridge for the authoring editor.
//
// Screencasts the running agent-browser CDP session into the editor's
// "Live page" pane and forwards clicks/keystrokes back. It is a read-only
// RELAY: it streams frames and dispatches synthetic input, but every
// scenario MUTATION still goes through the Rust CLI (record/run/flush).
// Localhost-only, and it only holds a CDP connection while the editor has
// the stream open — no background daemon.
//
// Frame source is a `Page.captureScreenshot` poll (a few fps) rather than
// `Page.startScreencast`: screencast only emits on compositor repaints, so
// a static page would show nothing to a late-joining viewer. Polling always
// delivers a current frame, headless or not.
//
// Input is sent in NORMALIZED coords (nx,ny in 0..1 of the frame). The
// bridge multiplies by the page's CSS layout viewport (from
// Page.getLayoutMetrics) so mapping is independent of devicePixelRatio.

// Roles whose click is a meaningful, recordable action. Clicking plain text
// (heading, paragraph, generic) is NOT recorded — same as Playwright/Chrome
// codegen. Text fields are excluded here because typing records a fill step.
const CLICKABLE_ROLES = new Set([
  'button',
  'link',
  'checkbox',
  'radio',
  'switch',
  'tab',
  'menuitem',
  'menuitemcheckbox',
  'menuitemradio',
  'option',
  'treeitem',
  'togglebutton',
  'disclosuretriangle',
]);
// Broader set used only to tint the hover highlight (these accept input even
// if a bare click isn't what gets recorded).
const INTERACTIVE_ROLES = new Set([
  ...CLICKABLE_ROLES,
  'textbox',
  'searchbox',
  'combobox',
  'slider',
  'spinbutton',
  'listbox',
]);

// A few non-printable keys we translate to CDP keyDown/up with the codes
// Chrome expects; everything else printable goes through as a `char`.
const KEYMAP = {
  Enter: { code: 'Enter', windowsVirtualKeyCode: 13 },
  Backspace: { code: 'Backspace', windowsVirtualKeyCode: 8 },
  Tab: { code: 'Tab', windowsVirtualKeyCode: 9 },
  ArrowLeft: { code: 'ArrowLeft', windowsVirtualKeyCode: 37 },
  ArrowUp: { code: 'ArrowUp', windowsVirtualKeyCode: 38 },
  ArrowRight: { code: 'ArrowRight', windowsVirtualKeyCode: 39 },
  ArrowDown: { code: 'ArrowDown', windowsVirtualKeyCode: 40 },
  Escape: { code: 'Escape', windowsVirtualKeyCode: 27 },
};

function createLiveBridge({
  getCdpUrl,
  onRecord = null,
  WebSocketImpl = globalThis.WebSocket,
  fetchImpl = globalThis.fetch,
  captureMs = 300,
  logger = () => {},
}) {
  if (typeof getCdpUrl !== 'function') throw new TypeError('getCdpUrl is required');

  const subscribers = new Set();
  let ws = null;
  let connecting = null;
  let pollTimer = null;
  let capturing = false;
  let nextId = 1;
  const pending = new Map(); // id -> 'capture' | 'metrics'
  const calls = new Map(); // id -> { resolve, reject }  (request/response)
  let css = { width: 0, height: 0 }; // CSS layout viewport
  let pollCount = 0;
  let currentUrl = null;
  let stopped = false;
  let reconnectTimer = null;
  let captureSentAt = 0;
  // Auto-record (codegen) state.
  let typingDirty = false;
  let typingTimer = null;
  let lastFill = null;

  // Reads the focused field's accessible label + current value so typing can
  // be recorded as one fillByLabel step instead of per-keystroke noise.
  const ACTIVE_FIELD_JS = `(() => {
    const el = document.activeElement;
    if (!el) return null;
    const tag = (el.tagName || '').toLowerCase();
    const editable = tag === 'input' || tag === 'textarea' || el.getAttribute('contenteditable') === 'true';
    if (!editable) return null;
    const lbl = (el.labels && el.labels[0] && el.labels[0].innerText) || '';
    const name = (el.getAttribute('aria-label') || lbl || el.getAttribute('placeholder') || el.name || '').trim();
    const value = (el.value != null ? el.value : el.innerText) || '';
    return { name, value };
  })()`;

  async function findPage(cdpUrl) {
    const u = new URL(cdpUrl);
    const list = await (await fetchImpl(`http://${u.host}/json/list`)).json();
    const pages = (Array.isArray(list) ? list : []).filter(
      (t) => t.type === 'page' && t.webSocketDebuggerUrl,
    );
    // Prefer the real content tab agent-browser is driving over Chrome's
    // startup cruft (chrome://newtab, a stray about:blank), so the live view
    // and the ARIA snapshot show the same page.
    const page =
      pages.find((p) => /^https?:/i.test(p.url || '')) ||
      pages.find((p) => p.url && p.url !== 'about:blank' && !/^chrome:/i.test(p.url)) ||
      pages[0];
    if (!page) throw new Error('no CDP page target found');
    return page;
  }

  // Re-establish the CDP connection while the editor is still watching. This
  // is what makes clicking "Start" (which replaces the browser session) recover
  // on its own instead of getting stuck on "connecting…".
  function scheduleReconnect() {
    if (stopped || reconnectTimer || ws || connecting) return;
    if (subscribers.size === 0) return;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      if (stopped || ws || connecting || subscribers.size === 0) return;
      ensureConnected().catch((e) => {
        broadcastEvent('bridge-error', { error: String((e && e.message) || e) });
        scheduleReconnect();
      });
    }, 500);
    if (reconnectTimer.unref) reconnectTimer.unref();
  }

  function send(method, params) {
    if (!ws || ws.readyState !== 1) return null;
    const id = nextId++;
    ws.send(JSON.stringify({ id, method, params: params || {} }));
    return id;
  }

  // Request/response CDP call: resolves with msg.result for this id.
  function call(method, params) {
    return new Promise((resolve, reject) => {
      if (!ws || ws.readyState !== 1) return reject(new Error('live browser not connected'));
      const id = nextId++;
      calls.set(id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params: params || {} }));
      setTimeout(() => {
        if (calls.has(id)) {
          calls.delete(id);
          reject(new Error('CDP call timed out'));
        }
      }, 5000).unref?.();
    });
  }

  function broadcast(data) {
    const payload = `data: ${JSON.stringify({ data })}\n\n`;
    for (const res of subscribers) {
      try {
        res.write(payload);
      } catch {
        subscribers.delete(res);
      }
    }
  }

  function broadcastEvent(event, obj) {
    const payload = `event: ${event}\ndata: ${JSON.stringify(obj)}\n\n`;
    for (const res of subscribers) {
      try {
        res.write(payload);
      } catch {
        subscribers.delete(res);
      }
    }
  }

  function setUrl(url) {
    if (!url || url === currentUrl) return;
    currentUrl = url;
    broadcastEvent('url', { url });
  }

  function requestMetrics() {
    const id = send('Page.getLayoutMetrics');
    if (id) pending.set(id, 'metrics');
  }

  function requestFrame() {
    if (!ws || ws.readyState !== 1) return;
    // Watchdog: if a capture has been outstanding too long, assume the
    // response was lost and let polling resume rather than stalling forever.
    if (capturing) {
      if (Date.now() - captureSentAt > 2000) capturing = false;
      else return;
    }
    capturing = true;
    captureSentAt = Date.now();
    const id = send('Page.captureScreenshot', { format: 'jpeg', quality: 60 });
    if (id) pending.set(id, 'capture');
    else capturing = false;
    if (pollCount++ % 20 === 0) requestMetrics();
  }

  function startPolling() {
    if (pollTimer) return;
    pollTimer = setInterval(requestFrame, captureMs);
    if (pollTimer.unref) pollTimer.unref();
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  // Wipe per-connection state so a reconnect starts clean. Without this, a
  // screenshot in flight when the socket closes (e.g. the session is replaced
  // by `start`) would leave `capturing` stuck true and the new connection
  // would never request another frame -> permanent "connecting…".
  function resetConnState() {
    capturing = false;
    typingDirty = false;
    lastFill = null;
    if (typingTimer) {
      clearTimeout(typingTimer);
      typingTimer = null;
    }
    pending.clear();
    for (const { reject } of calls.values()) {
      try {
        reject(new Error('live browser disconnected'));
      } catch {
        /* ignore */
      }
    }
    calls.clear();
  }

  function onMessage(ev) {
    let msg;
    try {
      msg = JSON.parse(ev.data);
    } catch {
      return;
    }
    // Main-frame navigations keep the address bar in sync.
    if (msg.method === 'Page.frameNavigated' && msg.params && msg.params.frame && !msg.params.frame.parentId) {
      setUrl(msg.params.frame.url);
      return;
    }
    // Page finished loading -> tell the editor to refresh the element picker
    // (covers same-URL reloads and pages whose DOM settles after navigation).
    if (msg.method === 'Page.loadEventFired' || msg.method === 'Page.frameStoppedLoading') {
      broadcastEvent('loaded', {});
      return;
    }
    if (!msg.id) return;
    if (calls.has(msg.id)) {
      const { resolve, reject } = calls.get(msg.id);
      calls.delete(msg.id);
      if (msg.error) reject(new Error(msg.error.message || 'CDP error'));
      else resolve(msg.result);
      return;
    }
    const kind = pending.get(msg.id);
    if (!kind) return;
    pending.delete(msg.id);
    if (kind === 'capture') {
      capturing = false;
      const data = msg.result && msg.result.data;
      if (data) broadcast(data);
    } else if (kind === 'metrics') {
      const vp =
        (msg.result && (msg.result.cssLayoutViewport || msg.result.layoutViewport)) || null;
      if (vp && vp.clientWidth && vp.clientHeight) {
        css = { width: vp.clientWidth, height: vp.clientHeight };
      }
    }
  }

  function ensureConnected() {
    if (ws && ws.readyState === 1) return Promise.resolve();
    if (connecting) return connecting;
    connecting = (async () => {
      resetConnState();
      const cdpUrl = await getCdpUrl();
      const page = await findPage(cdpUrl);
      await new Promise((resolve, reject) => {
        const sock = new WebSocketImpl(page.webSocketDebuggerUrl);
        sock.addEventListener('open', () => {
          ws = sock;
          send('Page.enable');
          send('DOM.enable');
          send('Accessibility.enable');
          requestMetrics();
          requestFrame(); // instant first frame
          startPolling();
          if (page.url) setUrl(page.url);
          resolve();
        });
        sock.addEventListener('message', onMessage);
        sock.addEventListener('close', () => {
          if (ws === sock) {
            ws = null;
            stopPolling();
            resetConnState();
            scheduleReconnect();
          }
        });
        sock.addEventListener('error', (e) =>
          reject(new Error('CDP socket error: ' + ((e && e.message) || 'failed'))),
        );
      });
    })()
      .catch((e) => {
        logger('live-bridge connect failed: ' + (e.message || e));
        throw e;
      })
      .finally(() => {
        connecting = null;
      });
    return connecting;
  }

  async function subscribe(res) {
    subscribers.add(res);
    try {
      await ensureConnected();
      // Late joiners get the current address immediately.
      if (currentUrl) {
        try {
          res.write(`event: url\ndata: ${JSON.stringify({ url: currentUrl })}\n\n`);
        } catch {
          /* client gone */
        }
      }
    } catch (e) {
      try {
        res.write(`event: bridge-error\ndata: ${JSON.stringify({ error: String(e.message || e) })}\n\n`);
      } catch {
        /* client already gone */
      }
    }
  }

  function unsubscribe(res) {
    subscribers.delete(res);
    if (subscribers.size === 0) {
      stopPolling();
      if (ws) {
        try {
          ws.close();
        } catch {
          /* ignore */
        }
        ws = null;
      }
    }
  }

  function broadcastRecordable(kind, payload) {
    broadcastEvent('recordable', { kind, payload });
  }

  // Commit an auto-recorded step. When the host provides onRecord (the real
  // server), the step is recorded ONCE there via the Rust CLI and we just tell
  // every tab to refresh. Without onRecord (tests / no server) we fall back to
  // broadcasting a 'recordable' event for a client to persist.
  async function emitRecord(kind, payload) {
    if (!onRecord) {
      broadcastRecordable(kind, payload);
      return;
    }
    try {
      await onRecord(kind, payload);
      broadcastEvent('buffer-changed', { kind, payload });
    } catch (e) {
      broadcastEvent('record-skip', { reason: String((e && e.message) || e) });
    }
  }

  // Flush buffered typing into a single fillByLabel step (deduped).
  async function finalizeFill() {
    if (typingTimer) {
      clearTimeout(typingTimer);
      typingTimer = null;
    }
    if (!typingDirty) return;
    typingDirty = false;
    let info = null;
    try {
      const r = await call('Runtime.evaluate', { expression: ACTIVE_FIELD_JS, returnByValue: true });
      info = r && r.result && r.result.value;
    } catch {
      return;
    }
    if (!info || !info.name || info.value === '') return;
    if (lastFill && lastFill.name === info.name && lastFill.value === info.value) return;
    lastFill = { name: info.name, value: info.value };
    emitRecord('action', { method: 'fillByLabel', args: [info.name, info.value] });
  }

  function bumpTyping() {
    typingDirty = true;
    if (typingTimer) clearTimeout(typingTimer);
    typingTimer = setTimeout(() => {
      finalizeFill().catch(() => {});
    }, 900);
    if (typingTimer.unref) typingTimer.unref();
  }

  // Resolve role+name at a point and emit a recordable clickRole step — but
  // only for genuinely clickable elements (a heading/text click is ignored).
  // The hit-test runs BEFORE the click is dispatched, because a click on a
  // link/button often navigates or rebuilds the DOM, which would otherwise
  // make the original target un-resolvable.
  async function recordAndClick(nx, ny, x, y) {
    await finalizeFill(); // commit any pending field first, so order is right
    let el = null;
    try {
      el = await pick(nx, ny);
    } catch {
      /* nothing pickable */
    }
    dispatchClick(x, y); // now actually click
    if (el && CLICKABLE_ROLES.has(el.role)) {
      if (el.name) {
        emitRecord('action', { method: 'clickRole', args: [el.role, el.name] });
      } else {
        broadcastEvent('record-skip', { reason: `${el.role} has no accessible name` });
      }
    }
    lastFill = null; // new context after a click
  }

  function dispatchClick(x, y) {
    send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, buttons: 0 });
    send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1 });
    send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', buttons: 0, clickCount: 1 });
  }

  // Normalized input -> CSS-pixel CDP events. Returns false if not connected.
  function input(evt) {
    if (!ws || ws.readyState !== 1) return false;
    const px = () => ({
      x: Math.round((Number(evt.nx) || 0) * css.width),
      y: Math.round((Number(evt.ny) || 0) * css.height),
    });
    switch (evt && evt.type) {
      case 'click': {
        if (!css.width) return false; // no viewport yet — refuse rather than misclick
        const { x, y } = px();
        if (evt.record) {
          // hit-test then click then record (handles navigating clicks)
          recordAndClick(Number(evt.nx) || 0, Number(evt.ny) || 0, x, y);
        } else {
          dispatchClick(x, y);
        }
        return true;
      }
      case 'move': {
        if (!css.width) return false;
        const { x, y } = px();
        send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, buttons: 0 });
        return true;
      }
      case 'scroll': {
        const { x, y } = px();
        send('Input.dispatchMouseEvent', {
          type: 'mouseWheel',
          x,
          y,
          deltaX: Number(evt.dx) || 0,
          deltaY: Number(evt.dy) || 0,
        });
        return true;
      }
      case 'navigate': {
        const url = normalizeUrl(evt.url);
        if (!url) return false;
        send('Page.navigate', { url });
        if (evt.record) emitRecord('navigation', { route: url });
        return true;
      }
      case 'reload': {
        send('Page.reload');
        return true;
      }
      case 'back': {
        send('Runtime.evaluate', { expression: 'history.back()' });
        return true;
      }
      case 'forward': {
        send('Runtime.evaluate', { expression: 'history.forward()' });
        return true;
      }
      case 'key': {
        if (typeof evt.text === 'string' && evt.text.length === 1) {
          send('Input.dispatchKeyEvent', { type: 'char', text: evt.text, key: evt.text });
          if (evt.record) bumpTyping();
          return true;
        }
        const m = KEYMAP[evt.key];
        if (m) {
          send('Input.dispatchKeyEvent', { type: 'keyDown', key: evt.key, ...m });
          send('Input.dispatchKeyEvent', { type: 'keyUp', key: evt.key, ...m });
          if (evt.record) {
            if (evt.key === 'Backspace') {
              bumpTyping();
            } else if (evt.key === 'Enter') {
              finalizeFill()
                .then(() => emitRecord('action', { method: 'pressKey', args: ['Enter'] }))
                .catch(() => {});
            }
          }
          return true;
        }
        return false;
      }
      default:
        return false;
    }
  }

  // Hit-test a normalized point and resolve the element's ACCESSIBLE role +
  // name (as the a11y tree computes them) so a picked element becomes a
  // replayable clickRole(role, name) step. Throws if not connected / no
  // viewport / nothing at that point.
  async function pick(nx, ny) {
    if (!ws || ws.readyState !== 1) throw new Error('live browser not connected');
    if (!css.width) throw new Error('viewport size unknown yet');
    const x = Math.round((Number(nx) || 0) * css.width);
    const y = Math.round((Number(ny) || 0) * css.height);
    const loc = await call('DOM.getNodeForLocation', { x, y, includeUserAgentShadowDOM: false });
    const backendNodeId = loc && loc.backendNodeId;
    if (!backendNodeId) throw new Error('no element at that point');
    let role = '';
    let name = '';
    try {
      const ax = await call('Accessibility.getPartialAXTree', { backendNodeId, fetchRelatives: false });
      const nodes = (ax && ax.nodes) || [];
      const named = nodes.find((n) => n.name && n.name.value && n.role && n.role.value);
      const node = named || nodes[0];
      if (node) {
        role = (node.role && node.role.value) || '';
        name = (node.name && node.name.value) || '';
      }
    } catch {
      /* a11y lookup failed — return coords only */
    }
    // Bounding box (normalized 0..1 of the CSS viewport) so the editor can draw
    // a hover highlight independent of the canvas display size.
    let box = null;
    try {
      const bm = await call('DOM.getBoxModel', { backendNodeId });
      const q = bm && bm.model && (bm.model.border || bm.model.content);
      if (q && q.length === 8 && css.width && css.height) {
        const xs = [q[0], q[2], q[4], q[6]];
        const ys = [q[1], q[3], q[5], q[7]];
        const minX = Math.min(...xs);
        const minY = Math.min(...ys);
        const maxX = Math.max(...xs);
        const maxY = Math.max(...ys);
        box = {
          nx: minX / css.width,
          ny: minY / css.height,
          nw: (maxX - minX) / css.width,
          nh: (maxY - minY) / css.height,
        };
      }
    } catch {
      /* no box model — highlight just won't draw */
    }
    return { role, name, x, y, box, interactive: INTERACTIVE_ROLES.has(role) };
  }

  function stop() {
    stopped = true;
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    stopPolling();
    for (const res of subscribers) {
      try {
        res.end();
      } catch {
        /* ignore */
      }
    }
    subscribers.clear();
    if (ws) {
      try {
        ws.close();
      } catch {
        /* ignore */
      }
      ws = null;
    }
  }

  return {
    subscribe,
    unsubscribe,
    input,
    pick,
    stop,
    get currentUrl() {
      return currentUrl;
    },
    get subscriberCount() {
      return subscribers.size;
    },
  };
}

// Add a scheme if the user typed a bare host, and reject anything that
// isn't http(s)/about so the address bar can't be coaxed into file:// etc.
function normalizeUrl(raw) {
  const s = String(raw == null ? '' : raw).trim();
  if (!s) return null;
  if (/^about:/i.test(s)) return s;
  const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(s) ? s : `https://${s}`;
  try {
    const u = new URL(withScheme);
    if (u.protocol !== 'http:' && u.protocol !== 'https:') return null;
    return u.toString();
  } catch {
    return null;
  }
}

module.exports = { createLiveBridge };
