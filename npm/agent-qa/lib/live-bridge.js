'use strict';
// Live-browser bridge for the authoring editor.
//
// Downloads: when the bridge records (an onRecord host), it arms
// `Page.setDownloadBehavior` to allow+name into a scratch dir so
// `Page.downloadWillBegin` events fire. A click held in the suppression
// window when the event lands records `do/download` (on the clicked
// locator, value `downloads/<file>`) instead of `do/click`.
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
  Delete: { code: 'Delete', windowsVirtualKeyCode: 46 },
  Home: { code: 'Home', windowsVirtualKeyCode: 36 },
  End: { code: 'End', windowsVirtualKeyCode: 35 },
  PageUp: { code: 'PageUp', windowsVirtualKeyCode: 33 },
  PageDown: { code: 'PageDown', windowsVirtualKeyCode: 34 },
  F1: { code: 'F1', windowsVirtualKeyCode: 112 },
  F2: { code: 'F2', windowsVirtualKeyCode: 113 },
  F3: { code: 'F3', windowsVirtualKeyCode: 114 },
  F4: { code: 'F4', windowsVirtualKeyCode: 115 },
  F5: { code: 'F5', windowsVirtualKeyCode: 116 },
  F6: { code: 'F6', windowsVirtualKeyCode: 117 },
  F7: { code: 'F7', windowsVirtualKeyCode: 118 },
  F8: { code: 'F8', windowsVirtualKeyCode: 119 },
  F9: { code: 'F9', windowsVirtualKeyCode: 120 },
  F10: { code: 'F10', windowsVirtualKeyCode: 121 },
  F11: { code: 'F11', windowsVirtualKeyCode: 122 },
  F12: { code: 'F12', windowsVirtualKeyCode: 123 },
};

// keydown events for the modifier keys themselves — a lone modifier press
// does nothing, so it is neither dispatched nor recorded.
const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta']);

// CDP Input.dispatchKeyEvent modifier bitmask (Alt=1 Ctrl=2 Meta=4 Shift=8)
// and the canonical `press` chord spelling (agent-browser order: Control,
// Alt, Shift, Meta). ctrl→Control ordering differs between the two maps on
// purpose: CDP wants a number, the verb wants a readable literal.
const MOD_BITS = { alt: 1, ctrl: 2, meta: 4, shift: 8 };
const MOD_ORDER = ['ctrl', 'alt', 'shift', 'meta'];
const MOD_LABEL = { ctrl: 'Control', alt: 'Alt', shift: 'Shift', meta: 'Meta' };

function modsMask(mods) {
  if (!mods) return 0;
  let bits = 0;
  for (const k of Object.keys(MOD_BITS)) if (mods[k]) bits |= MOD_BITS[k];
  return bits;
}

// `press` literal for a chorded key: Control+a, Shift+Tab, Control+Shift+s.
// Printable keys are lowercased to match agent-browser's examples.
function chordName(key, mods) {
  const parts = MOD_ORDER.filter((k) => mods && mods[k]).map((k) => MOD_LABEL[k]);
  parts.push(key.length === 1 ? key.toLowerCase() : key);
  return parts.join('+');
}

// code/vk for a printable key when modifiers turn it into a chord
// (a-z → KeyA/65, 0-9 → Digit0/48, else best-effort).
function charCodes(ch) {
  const up = ch.toUpperCase();
  if (up >= 'A' && up <= 'Z') return { code: `Key${up}`, windowsVirtualKeyCode: up.charCodeAt(0) };
  if (up >= '0' && up <= '9') return { code: `Digit${up}`, windowsVirtualKeyCode: up.charCodeAt(0) };
  return { code: '', windowsVirtualKeyCode: up.charCodeAt(0) };
}

function createLiveBridge({
  getCdpUrl,
  onRecord = null,
  WebSocketImpl = globalThis.WebSocket,
  fetchImpl = globalThis.fetch,
  captureMs = 300,
  reconnectMs = 500,
  logger = () => {},
  // How long a recorded click waits for a downloadWillBegin before it is
  // committed as a plain `do/click`. 0 disables the suppression window.
  downloadHoldMs = 1000,
  // Where recorded downloads land. Defaults to a fresh scratch dir; the
  // recorded step always points at `downloads/<name>` scenario-relative,
  // this dir is only so Chrome has somewhere to write at record time.
  downloadDir = null,
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
  let heldClick = null; // { el: {role,name}, timer } — click awaiting a possible download
  let recentClick = null; // { el, ts } — last emitted click, for late download correlation
  let dlDir = downloadDir;
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

  // Change events the input stream can't see: a <select> commits through a
  // native popup (no DOM click), and a checkbox/radio's meaningful action is
  // check/uncheck, not click. Installed into every document (auto-injected for
  // new navigations + evaluated once for the current page); idempotent.
  const RECORD_LISTENER_JS = `(() => {
    if (window.__aqRecInstalled) return;
    window.__aqRecInstalled = true;
    const aqName = (t) => {
      const lbl = (s) => (s || '').replace(/\\s+/g, ' ').trim();
      const aria = lbl(t.getAttribute('aria-label'));
      if (aria) return aria;
      const by = t.getAttribute('aria-labelledby');
      if (by) {
        const txt = lbl(by.split(/\\s+/).map((id) => {
          const e = document.getElementById(id);
          return e ? e.textContent : '';
        }).join(' '));
        if (txt) return txt;
      }
      if (t.labels && t.labels.length) { const s = lbl(t.labels[0].textContent); if (s) return s; }
      return lbl(t.getAttribute('placeholder') || t.getAttribute('name') || t.getAttribute('id'));
    };
    const aqSel = (t) => {
      if (t.id) return '#' + CSS.escape(t.id);
      const parts = [];
      for (let n = t; n && n !== document.body && parts.length < 4; n = n.parentElement) {
        if (n.id) { parts.unshift('#' + CSS.escape(n.id)); break; }
        const cls = Array.from(n.classList || []).slice(0, 2)
          .map((c) => '.' + CSS.escape(c)).join('');
        parts.unshift(n.tagName.toLowerCase() + cls);
      }
      return parts.join(' ');
    };
    // Secondary-click gestures produce no change event — listen for
    // contextmenu directly so a right-click records as do/rightclick.
    document.addEventListener('contextmenu', (ev) => {
      const t = ev.target;
      if (!(t instanceof HTMLElement)) return;
      const rec = { kind: 'rightclick', name: aqName(t), selector: aqSel(t) };
      try { __aqRecord(JSON.stringify(rec)); } catch (e) { /* binding absent */ }
    }, true);
    document.addEventListener('change', (ev) => {
      const t = ev.target;
      if (!(t instanceof HTMLElement)) return;
      let rec = null;
      if (t.tagName === 'SELECT') {
        const labels = Array.from(t.selectedOptions || [])
          .map((o) => (o.textContent || o.value || '').trim())
          .filter(Boolean);
        rec = { kind: 'select', name: aqName(t), value: labels.join(',') };
      } else if (t.tagName === 'INPUT' && (t.type === 'checkbox' || t.type === 'radio')) {
        rec = { kind: t.checked ? 'check' : 'uncheck', name: aqName(t), role: t.type };
      } else if (t.tagName === 'INPUT' && t.type === 'file' && t.files && t.files.length) {
        // File inputs only expose basenames — the recorded step replays
        // once the user drops the real files next to the scenario.
        const css = t.id ? '#' + t.id
          : t.name ? 'input[type="file"][name="' + t.name + '"]'
          : 'input[type="file"]';
        rec = {
          kind: 'upload',
          name: aqName(t),
          selector: css,
          files: Array.from(t.files).map((f) => f.name),
        };
      }
      if (rec) { try { __aqRecord(JSON.stringify(rec)); } catch (e) { /* binding absent */ } }
    }, true);
  })()`;

  // Clicks on these roles are dispatched but NOT recorded as 'click' — the
  // page's change listener emits the honest verb (check/uncheck) instead.
  const CHANGE_DRIVEN_ROLES = new Set(['checkbox', 'radio']);

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
    }, reconnectMs);
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
    heldClick = null;
    recentClick = null;
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
      // New document after a navigation: re-arm the viewport metrics (so click
      // coords map against the NEW page, not the previous one) and grab a fresh
      // frame, then tell the editor to refresh the element picker. Without the
      // metrics refresh, the first interactions on the new page use stale css
      // dimensions — which is how a navigating click made recording "stop".
      requestMetrics();
      requestFrame();
      broadcastEvent('loaded', {});
      return;
    }
    // Page-side record events (select / check / uncheck) raised by the
    // injected listener via the __aqRecord binding.
    if (msg.method === 'Runtime.bindingCalled' && msg.params && msg.params.name === '__aqRecord') {
      handlePageRecord(msg.params.payload);
      return;
    }
    // A click that triggers a download must record do/download, not
    // do/click — convert the held click if one is inside the window.
    if (msg.method === 'Page.downloadWillBegin' && msg.params) {
      onDownloadWillBegin(msg.params);
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
          // Auto-record needs the page's change events (select popups and
          // check/uncheck produce none of the input events this bridge sees).
          // Runtime.enable is required for bindingCalled to be delivered.
          send('Runtime.enable');
          send('Runtime.addBinding', { name: '__aqRecord' });
          send('Page.addScriptToEvaluateOnNewDocument', { source: RECORD_LISTENER_JS });
          send('Runtime.evaluate', { expression: RECORD_LISTENER_JS });
          // Arm download events + redirect downloads into a scratch dir —
          // only on the recording bridge: the replay-watch bridges must not
          // override the download handling the run's own do/download verbs
          // rely on.
          if (onRecord) {
            if (!dlDir) dlDir = makeDownloadDir();
            if (dlDir) {
              call('Page.setDownloadBehavior', {
                behavior: 'allowAndName',
                downloadPath: dlDir,
              }).catch(() => {});
            }
          }
          requestMetrics();
          requestFrame(); // instant first frame
          startPolling();
          if (page.url) setUrl(page.url);
          resolve();
        });
        sock.addEventListener('message', onMessage);
        sock.addEventListener('close', () => {
          if (ws === sock) {
            // The click physically dispatched even if the socket died —
            // commit it as a plain click rather than dropping it.
            flushHeldClick();
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
      // The preview commonly subscribes before the agent launches its first
      // browser. Keep retrying while that subscriber is present so the first
      // successful `agent-browser open` appears without a tab switch/reload.
      scheduleReconnect();
    }
  }

  function unsubscribe(res) {
    subscribers.delete(res);
    if (subscribers.size === 0) {
      if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
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
    // Any other recorded step lands chronologically AFTER a click that is
    // still inside the download window — flush it first so buffer order
    // matches user order.
    flushHeldClick();
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
    emitRecord('do', { intent: `fill ${info.name}`, verb: 'type', on: { role: 'textbox', name: info.name }, value: { from: 'literal', literal: info.value } });
  }

  // Translate a __aqRecord binding payload ({kind, name, value, role}) into a
  // recorded step. name '' → record-skip, same UX as an unnamed click.
  function handlePageRecord(payload) {
    let rec;
    try {
      rec = JSON.parse(payload);
    } catch {
      return;
    }
    if (!rec || typeof rec !== 'object') return;
    // rightclick falls back to its css selector — a nameless icon button or
    // canvas region is still recordable.
    if (!rec.name && !(rec.kind === 'rightclick' && rec.selector)) {
      broadcastEvent('record-skip', { reason: `${rec.kind} target has no accessible name` });
      return;
    }
    if (rec.kind === 'select') {
      emitRecord('do', {
        intent: `select ${rec.value} in ${rec.name}`,
        verb: 'select',
        on: { role: 'combobox', name: rec.name },
        value: { from: 'literal', literal: rec.value },
      });
    } else if (rec.kind === 'upload') {
      const files = Array.isArray(rec.files) && rec.files.length ? rec.files : [];
      if (!files.length) return;
      const on = rec.selector
        ? { raw: { kind: 'css', value: rec.selector }, reason: 'file input' }
        : { role: 'button', name: rec.name };
      emitRecord('do', {
        intent: `upload ${files.join(', ')} to ${rec.name} — place the file(s) next to the scenario`,
        verb: 'upload',
        on,
        value: { from: 'literal', literal: files.length === 1 ? files[0] : files },
      });
    } else if (rec.kind === 'rightclick') {
      if (!rec.selector) {
        broadcastEvent('record-skip', { reason: 'rightclick target has no css selector' });
        return;
      }
      emitRecord('do', {
        intent: `right-click ${rec.name || rec.selector}`,
        verb: 'rightclick',
        on: { raw: { kind: 'css', value: rec.selector }, reason: 'right-click target' },
      });
    } else if (rec.kind === 'check' || rec.kind === 'uncheck') {
      emitRecord('do', {
        intent: `${rec.kind} ${rec.name}`,
        verb: rec.kind,
        on: { role: rec.role, name: rec.name },
      });
    }
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
      if (CHANGE_DRIVEN_ROLES.has(el.role)) {
        // The injected change listener records check/uncheck — a bare click
        // here would double-record (and hide whether it set or cleared).
      } else if (el.name) {
        holdClick(el);
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

  function makeDownloadDir() {
    try {
      const fs = require('fs');
      const os = require('os');
      const path = require('path');
      return fs.mkdtempSync(path.join(os.tmpdir(), 'agent-qa-dl-'));
    } catch {
      return null;
    }
  }

  // A click that starts a download must record `do/download`, not
  // `do/click` — but downloadWillBegin only fires once the response head
  // arrives, so every recorded click waits in a short suppression window
  // first. Plain clicks surface in the buffer ~downloadHoldMs later.
  function holdClick(el) {
    flushHeldClick();
    if (!downloadHoldMs) {
      emitClick(el);
      return;
    }
    heldClick = {
      el,
      timer: setTimeout(() => {
        heldClick = null;
        emitClick(el);
      }, downloadHoldMs),
    };
    if (heldClick.timer.unref) heldClick.timer.unref();
  }

  function emitClick(el) {
    recentClick = { el, ts: Date.now() };
    emitRecord('do', {
      intent: `click ${el.name}`,
      verb: 'click',
      on: { role: el.role, name: el.name },
    });
  }

  function flushHeldClick() {
    if (!heldClick) return;
    clearTimeout(heldClick.timer);
    const el = heldClick.el;
    heldClick = null;
    emitClick(el);
  }

  function sanitizeFilename(name) {
    const base = String(name || '').split(/[\\/]/).pop() || '';
    const safe = base.replace(/[^\w.\- ]+/g, '_').trim();
    return safe || 'download';
  }

  function emitDownload(el, filename) {
    emitRecord('do', {
      intent: `download ${filename}`,
      verb: 'download',
      on: { role: el.role, name: el.name },
      value: { from: 'literal', literal: `downloads/${filename}` },
    });
  }

  function onDownloadWillBegin(p) {
    const filename = sanitizeFilename(p.suggestedFilename);
    // Inside the suppression window the held click IS the trigger.
    if (heldClick) {
      const held = heldClick;
      heldClick = null;
      clearTimeout(held.timer);
      emitDownload(held.el, filename);
      return;
    }
    // The click already committed (slow server before the response head) —
    // still record the download on the same locator and flag the earlier
    // click step as a duplicated trigger for the buffer UI to surface.
    if (recentClick && Date.now() - recentClick.ts < 3000) {
      emitDownload(recentClick.el, filename);
      broadcastEvent('record-skip', {
        reason: 'download started late — the click step just above duplicates the trigger; delete it before flush',
      });
      return;
    }
    broadcastEvent('record-skip', {
      reason: 'download detected with no recorded click — step skipped',
    });
  }

  // The same drag gesture `do/drag` emits, run directly on the resolved nodes.
  // CDP's dispatched mouse presses never initiate native HTML5 dnd, so the
  // bridge performs the DOM-level chain itself: pointerdown/mousedown on the
  // source, a shared-DataTransfer dragstart→enter/over→drop→dragend chain on
  // the endpoints, pointerup/mouseup on the target. Element-targeted (not
  // coordinate-targeted), so overlay interception can't swallow the drop and
  // the gesture replays identically.
  const DRAG_GESTURE_FN = `function (dst) {
    const src = this;
    const ctr = (el) => { const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; };
    const s = ctr(src), d = ctr(dst);
    const o = (x, y, up) => ({ bubbles: true, cancelable: true, composed: true, view: window, clientX: x, clientY: y, button: 0, buttons: up ? 0 : 1 });
    src.dispatchEvent(new PointerEvent('pointerdown', o(s.x, s.y)));
    src.dispatchEvent(new MouseEvent('mousedown', o(s.x, s.y)));
    try {
      const dt = new DataTransfer();
      src.dispatchEvent(new DragEvent('dragstart', Object.assign(o(s.x, s.y), { dataTransfer: dt })));
      document.dispatchEvent(new MouseEvent('mousemove', o(d.x, d.y)));
      dst.dispatchEvent(new PointerEvent('pointermove', o(d.x, d.y)));
      dst.dispatchEvent(new DragEvent('dragenter', Object.assign(o(d.x, d.y), { dataTransfer: dt })));
      dst.dispatchEvent(new DragEvent('dragover', Object.assign(o(d.x, d.y), { dataTransfer: dt })));
      dst.dispatchEvent(new DragEvent('drop', Object.assign(o(d.x, d.y), { dataTransfer: dt })));
      src.dispatchEvent(new DragEvent('dragend', Object.assign(o(d.x, d.y), { dataTransfer: dt })));
    } catch (e) {
      document.dispatchEvent(new MouseEvent('mousemove', o(d.x, d.y)));
      dst.dispatchEvent(new PointerEvent('pointermove', o(d.x, d.y)));
    }
    dst.dispatchEvent(new PointerEvent('pointerup', o(d.x, d.y, true)));
    dst.dispatchEvent(new MouseEvent('mouseup', o(d.x, d.y, true)));
    return true;
  }`;

  async function dispatchDragOn(srcBackendId, dstBackendId) {
    const s = await call('DOM.resolveNode', { backendNodeId: srcBackendId });
    const d = await call('DOM.resolveNode', { backendNodeId: dstBackendId });
    const srcObj = s && s.object && s.object.objectId;
    const dstObj = d && d.object && d.object.objectId;
    if (!srcObj || !dstObj) throw new Error('drag endpoint not resolvable');
    await call('Runtime.callFunctionOn', {
      objectId: srcObj,
      functionDeclaration: DRAG_GESTURE_FN,
      arguments: [{ objectId: dstObj }],
      returnByValue: true,
    });
  }

  async function backendNodeAt(nx, ny) {
    if (!css.width) return 0;
    const x = Math.round((Number(nx) || 0) * css.width);
    const y = Math.round((Number(ny) || 0) * css.height);
    const r = await call('DOM.getNodeForLocation', { x, y, includeUserAgentShadowDOM: false });
    return (r && r.backendNodeId) || 0;
  }

  async function dispatchDrag(nx0, ny0, nx1, ny1) {
    const s = await backendNodeAt(nx0, ny0);
    const d = await backendNodeAt(nx1, ny1);
    if (!s || !d) return;
    try {
      await dispatchDragOn(s, d);
    } catch {
      /* gesture failed — drop just doesn't happen */
    }
  }

  // Resolve both endpoints to role+name BEFORE dispatching the gesture — a
  // drop often reorders or rebuilds the list, after which a pick at the
  // original point would resolve the element that slid into place.
  // Then run the DOM-level drag gesture on the picked nodes and emit a
  // recordable `drag` step. Nameless endpoints can't be replayed — same
  // contract as click.
  async function recordAndDrag(nx0, ny0, nx1, ny1) {
    await finalizeFill();
    let src = null;
    let dst = null;
    try {
      src = await pick(nx0, ny0);
      dst = await pick(nx1, ny1);
    } catch {
      /* unpickable endpoint */
    }
    if (!src || !dst || !src.backendNodeId || !dst.backendNodeId) {
      broadcastEvent('record-skip', { reason: 'drag endpoint not resolvable' });
      return;
    }
    try {
      await dispatchDragOn(src.backendNodeId, dst.backendNodeId);
    } catch {
      /* gesture failed — still emit the step so the recorder can retry */
    }
    if (!src.name || !dst.name) {
      broadcastEvent('record-skip', {
        reason: `drag ${src.name ? 'target' : 'source'} has no accessible name`,
      });
      return;
    }
    emitRecord('do', {
      intent: `drag ${src.name} onto ${dst.name}`,
      verb: 'drag',
      on: { role: src.role, name: src.name },
      params: { to: { role: dst.role, name: dst.name } },
    });
    lastFill = null;
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
      case 'drag': {
        if (!css.width) return false;
        const nx0 = Number(evt.nx0) || 0;
        const ny0 = Number(evt.ny0) || 0;
        const nx1 = Number(evt.nx1) || 0;
        const ny1 = Number(evt.ny1) || 0;
        if (evt.record) {
          recordAndDrag(nx0, ny0, nx1, ny1);
        } else {
          dispatchDrag(nx0, ny0, nx1, ny1);
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
        // Never recorded: no replay verb models a raw wheel delta (scrollTo
        // scrolls an element into view — a different contract).
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
        if (evt.record) emitRecord('do', { intent: `open ${url}`, verb: 'goto', value: { from: 'literal', literal: url } });
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
        if (MODIFIER_KEYS.has(evt.key)) return true; // lone modifier = no-op

        const keyName = typeof evt.text === 'string' && evt.text.length === 1 ? evt.text : evt.key;
        const isChar = keyName.length === 1;
        // Printable keys chorded with Ctrl/Alt/Meta are shortcuts, not typing
        // (Shift stays on the typing path — Shift+a is just 'A'). Named keys
        // chord with any modifier (Shift+Tab, Control+Enter, …).
        const chorded = modsMask(evt.mods) !== 0 && (isChar ? evt.mods.ctrl || evt.mods.alt || evt.mods.meta : true);

        if (chorded) {
          const modifiers = modsMask(evt.mods);
          const codes = isChar ? charCodes(keyName) : (KEYMAP[keyName] || charCodes(keyName));
          const base = { key: keyName, modifiers, ...codes };
          send('Input.dispatchKeyEvent', { type: 'keyDown', ...base });
          send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
          if (evt.record) {
            const literal = chordName(keyName, evt.mods);
            finalizeFill()
              .then(() => emitRecord('do', { intent: `press ${literal}`, verb: 'press', value: { from: 'literal', literal } }))
              .catch(() => {});
          }
          return true;
        }

        if (isChar) {
          send('Input.dispatchKeyEvent', { type: 'char', text: keyName, key: keyName });
          if (evt.record) bumpTyping();
          return true;
        }
        const m = KEYMAP[keyName];
        if (m) {
          send('Input.dispatchKeyEvent', { type: 'keyDown', key: keyName, ...m });
          send('Input.dispatchKeyEvent', { type: 'keyUp', key: keyName, ...m });
          if (evt.record) {
            if (keyName === 'Backspace') {
              bumpTyping();
            } else if (keyName === 'Enter') {
              finalizeFill()
                .then(() => emitRecord('do', { intent: 'press Enter', verb: 'press', value: { from: 'literal', literal: 'Enter' } }))
                .catch(() => {});
            } else {
              // Tab/Escape/arrows/etc. — record an honest `press <key>` step
              // (a pending fill commits first so field content lands before
              // the focus change).
              finalizeFill()
                .then(() => emitRecord('do', { intent: `press ${evt.key}`, verb: 'press', value: { from: 'literal', literal: evt.key } }))
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
      // fetchRelatives so we get the ancestor chain too: DOM.getNodeForLocation
      // returns the DEEPEST node (often a text/span that resolves to "generic"),
      // but the clickable role lives on an ancestor <a>/<button>. Walk up to it.
      const ax = await call('Accessibility.getPartialAXTree', { backendNodeId, fetchRelatives: true });
      const nodes = (ax && ax.nodes) || [];
      const byId = new Map(nodes.map((n) => [n.nodeId, n]));
      const parentOf = new Map();
      for (const n of nodes) for (const c of n.childIds || []) parentOf.set(c, n.nodeId);
      const hit = nodes.find((n) => String(n.backendDOMNodeId) === String(backendNodeId));
      let node = hit || nodes.find((n) => n.name && n.name.value && n.role && n.role.value) || nodes[0];
      let cur = hit;
      for (let i = 0; cur && i < 12; i++) {
        const r = cur.role && cur.role.value;
        if (r && INTERACTIVE_ROLES.has(r)) {
          node = cur;
          break;
        }
        cur = byId.get(parentOf.get(cur.nodeId));
      }
      if (node) {
        role = (node.role && node.role.value) || '';
        name = (node.name && node.name.value) || '';
      }
      if (!name && backendNodeId) name = await textName(backendNodeId);
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
    return { role, name, x, y, box, interactive: INTERACTIVE_ROLES.has(role), backendNodeId };
  }

  // Content-derived name fallback for elements whose accessible name the AX
  // tree leaves empty (plain listitems, generic containers). Replay's name
  // ladder matches innerText/textContent alongside accName, so a text-derived
  // name records a locator that replays.
  async function textName(backendNodeId) {
    try {
      const r = await call('DOM.resolveNode', { backendNodeId });
      const oid = r && r.object && r.object.objectId;
      if (!oid) return '';
      const res = await call('Runtime.callFunctionOn', {
        objectId: oid,
        functionDeclaration:
          'function () { return ((this.innerText || this.textContent || "").trim()).slice(0, 80); }',
        returnByValue: true,
      });
      return (res && res.result && res.result.value) || '';
    } catch {
      return '';
    }
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
