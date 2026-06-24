'use strict';
// agent-qa authoring editor.
//
// A second tab of the localhost app. It builds a scenario.json by
// targeting the live page: pick an element from the ARIA tree, compose a
// deterministic step (role+name / unique-token target, structured
// assert), run it live for feedback, then record it. Every mutation goes
// through the Rust CLI via /api/edit/* — the editor never hand-edits the
// scenario tree, so `flush` output stays a replayable contract.

const $ = (id) => document.getElementById(id);

async function getJson(url) {
  const r = await fetch(url, { headers: { accept: 'application/json' } });
  const body = await r.json().catch(() => ({}));
  return { ok: r.ok, status: r.status, body };
}

async function postJson(url, payload) {
  const r = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload || {}),
  });
  const body = await r.json().catch(() => ({}));
  return { ok: r.ok, status: r.status, body };
}

// ---------- compose: form → trigger payload ----------

// Which fields each composer verb shows. Mirrors the record-step trigger
// shapes; the Rust side re-validates everything.
const VERB_FIELDS = {
  navigation: { value: 'URL', fields: ['value'] },
  click: { fields: ['role', 'name'] },
  type: { value: 'Value', name: 'Label', fields: ['name', 'value'] },
  press: { value: 'Key', fields: ['value'] },
  wait: { value: 'Milliseconds', fields: ['value'] },
  assertPresent: { fields: ['role', 'name', 'intent'], intentRequired: true },
  assertAbsent: { fields: ['role', 'name', 'intent'], intentRequired: true },
  assertUrl: { value: 'URL pattern', fields: ['value', 'intent'], intentRequired: true },
};

function updateFields() {
  const verb = $('verbSelect').value;
  const spec = VERB_FIELDS[verb] || {};
  const show = new Set(spec.fields || []);
  for (const f of ['role', 'name', 'value', 'intent']) {
    const el = $(`${f}Field`);
    if (el) el.classList.toggle('hidden', !show.has(f));
  }
  // Relabel the value/name inputs per verb for clarity.
  if (show.has('value')) $('valueField').querySelector('label').textContent = spec.value || 'Value';
  if (show.has('name')) {
    $('nameField').querySelector('label').textContent = spec.name || 'Name / label';
  }
}

// Build { kind, payload } for record-step / run-step, or { error }.
function composePayload() {
  const verb = $('verbSelect').value;
  const role = $('roleInput').value.trim();
  const name = $('nameInput').value.trim();
  const value = $('valueInput').value.trim();
  const intent = $('stepIntentInput').value.trim();
  const spec = VERB_FIELDS[verb] || {};
  if (spec.intentRequired && !intent) return { error: 'Intent is required for asserts.' };

  switch (verb) {
    case 'navigation':
      if (!value) return { error: 'URL is required.' };
      return { kind: 'navigation', payload: intent ? { route: value, intent } : { route: value } };
    case 'click':
      if (!role || !name) return { error: 'Role and name are required.' };
      return {
        kind: 'action',
        payload: { method: 'clickRole', args: [role, name], ...(intent ? { intent } : {}) },
      };
    case 'type':
      if (!name) return { error: 'Label is required.' };
      return {
        kind: 'action',
        payload: { method: 'fillByLabel', args: [name, value], ...(intent ? { intent } : {}) },
      };
    case 'press':
      if (!value) return { error: 'Key is required.' };
      return { kind: 'action', payload: { method: 'pressKey', args: [value], ...(intent ? { intent } : {}) } };
    case 'wait': {
      const ms = parseInt(value, 10);
      if (!Number.isInteger(ms) || ms < 0) return { error: 'Milliseconds must be a non-negative integer.' };
      return { kind: 'wait', payload: { condition: { kind: 'duration', ms }, ...(intent ? { intent } : {}) } };
    }
    case 'assertPresent':
      if (!role || !name) return { error: 'Role and name are required.' };
      return { kind: 'assert', payload: { kind: 'present', args: [role, name], intent } };
    case 'assertAbsent':
      if (!role || !name) return { error: 'Role and name are required.' };
      return { kind: 'assert', payload: { kind: 'absent', args: [role, name], intent } };
    case 'assertUrl':
      if (!value) return { error: 'URL pattern is required.' };
      return { kind: 'assert', payload: { kind: 'url', args: [value], intent } };
    default:
      return { error: `unknown verb ${verb}` };
  }
}

// ---------- buffer (recorded steps) ----------

// Map a buffer row to a human, scannable label: a category (drives the badge
// colour), a short action title, and a full detail line (target/value). Modeled
// on Chrome DevTools Recorder + Playwright codegen so the author can read what
// each step does at a glance instead of a raw verb name.
function rowLabel(row) {
  const p = row.payload || {};
  switch (row.kind) {
    case 'navigation':
      return { cls: 'nav', title: 'Go to', detail: p.route || '' };
    case 'action': {
      const m = p.method || 'action';
      const a = p.args || [];
      switch (m) {
        case 'clickRole':
          return { cls: 'click', title: 'Click', detail: `${a[0]} “${a[1]}”` };
        case 'clickByText':
        case 'clickByLabel':
          return { cls: 'click', title: 'Click', detail: `“${a[0]}”` };
        case 'clickSelector':
          return { cls: 'click', title: 'Click', detail: a[0] || '' };
        case 'fillByLabel':
        case 'fillBySelector':
          return { cls: 'fill', title: 'Fill', detail: `${a[0]} → ${maskValue(a[0], a[1])}` };
        case 'pressKey':
          return { cls: 'press', title: 'Press', detail: a[0] || '' };
        default:
          return { cls: 'action', title: m, detail: a.join('  ·  ') };
      }
    }
    case 'wait': {
      const c = p.condition || {};
      return { cls: 'wait', title: 'Wait', detail: c.ms != null ? `${c.ms} ms` : c.kind || '' };
    }
    case 'assert':
      return { cls: 'assert', title: `Assert ${p.kind || ''}`.trim(), detail: (p.args || []).join('  ·  ') };
    default:
      return { cls: 'action', title: row.kind || '?', detail: '' };
  }
}

// Hide secret-looking field values in the steps list (the value is still
// recorded faithfully in the buffer/scenario — this only masks the display).
function maskValue(label, value) {
  const v = String(value == null ? '' : value);
  if (/pass|secret|cvv|card|token|otp|\bpin\b/i.test(String(label || ''))) {
    return '•'.repeat(Math.min(10, Math.max(4, v.length)));
  }
  return v;
}

async function refreshBuffer() {
  const { body } = await getJson('/api/edit/buffer');
  const rows = Array.isArray(body.rows) ? body.rows : [];
  // Session header.
  const active = !!body.sid;
  $('sessionActive').classList.toggle('hidden', !active);
  $('startBtn').classList.toggle('hidden', active);
  $('startFields').classList.toggle('hidden', active);
  if (active) {
    $('activeSid').textContent = body.sid;
    $('sessionMeta').textContent = body.intent ? `“${body.intent}”` : '';
    if (body.intent && !$('intentInput').value) $('intentInput').value = body.intent;
  } else {
    $('sessionMeta').textContent = '';
  }

  const list = $('bufferList');
  list.innerHTML = '';
  $('bufferEmpty').classList.toggle('hidden', rows.length > 0);
  rows.forEach((row, i) => {
    const { cls, title, detail } = rowLabel(row);
    const li = document.createElement('li');
    li.className = 'buffer-step';
    li.innerHTML = `
      <span class="idx">${i}</span>
      <span class="step-main">
        <span class="step-head"><span class="vbadge v-${cls}">${escapeHtml(title)}</span></span>
        <span class="btext" title="${escapeHtml(detail)}">${escapeHtml(detail)}</span>
      </span>
      <span class="row-actions">
        <button class="icon-btn" data-act="up" ${i === 0 ? 'disabled' : ''} title="Move up">↑</button>
        <button class="icon-btn" data-act="down" ${i === rows.length - 1 ? 'disabled' : ''} title="Move down">↓</button>
        <button class="icon-btn danger" data-act="del" title="Delete">✕</button>
      </span>`;
    li.querySelector('[data-act="up"]').onclick = () => moveRow(i, i - 1);
    li.querySelector('[data-act="down"]').onclick = () => moveRow(i, i + 1);
    li.querySelector('[data-act="del"]').onclick = () => deleteRow(i);
    list.appendChild(li);
  });
}

async function moveRow(from, to) {
  if (to < 0) return;
  await postJson('/api/edit/move', { from, to });
  refreshBuffer();
}

async function deleteRow(index) {
  await postJson('/api/edit/delete', { index });
  refreshBuffer();
}

// ---------- session lifecycle ----------

async function startSession() {
  const intent = $('intentInput').value.trim();
  if (!intent) return flash('Enter an intent first.', true);
  const url = $('openUrlInput').value.trim() || 'about:blank';
  const { ok, body } = await postJson('/api/edit/start', { intent, url });
  if (!ok) return flash(body.error || 'start failed', true);
  flash(`started ${body.sid || ''}`);
  // Bake the entry navigation in as step 0 so a pick-recorded click has a
  // page to land on at replay time (start --open is authoring setup, not a
  // recorded step on its own).
  if (url && url !== 'about:blank') {
    $('addressBar').value = url;
    await postJson('/api/edit/record', { kind: 'navigation', payload: { route: url } });
  }
  await refreshBuffer();
  connectLive();
  await snapshot();
}

async function flushScenario() {
  const { ok, body } = await postJson('/api/edit/flush', {});
  if (!ok) return flash(body.error || 'flush failed', true);
  flash(`flushed → ${body.scenarioFile || body.sid || 'scenario.json'}`);
  await refreshBuffer();
}

// Discard the in-progress recording after an explicit confirm. Empties the
// buffer and drops the session so the editor returns to the start screen.
async function cancelScenario() {
  const n = bufferCount();
  const ok = await confirmDialog({
    title: 'Discard this recording?',
    body:
      n > 0
        ? `${n} recorded step${n === 1 ? '' : 's'} will be thrown away. This can’t be undone.`
        : 'This recording session will be discarded.',
    confirm: 'Discard',
    danger: true,
  });
  if (!ok) return;
  const r = await postJson('/api/edit/cancel', {});
  if (!r.ok) return flash(r.body.error || 'cancel failed', true);
  flash('recording discarded');
  await refreshBuffer();
}

function bufferCount() {
  return document.querySelectorAll('#bufferList .buffer-step').length;
}

// Minimal promise-based confirm modal. Resolves true on confirm, false on
// cancel / overlay click / Escape.
function confirmDialog({ title, body, confirm = 'OK', cancel = 'Cancel', danger = false }) {
  return new Promise((resolve) => {
    const overlay = $('confirmModal');
    $('confirmTitle').textContent = title || '';
    $('confirmBody').textContent = body || '';
    const okBtn = $('confirmOkBtn');
    const cancelBtn = $('confirmCancelBtn');
    okBtn.textContent = confirm;
    cancelBtn.textContent = cancel;
    okBtn.classList.toggle('danger', !!danger);
    overlay.classList.remove('hidden');
    okBtn.focus();

    const done = (val) => {
      overlay.classList.add('hidden');
      okBtn.onclick = cancelBtn.onclick = overlay.onclick = null;
      document.removeEventListener('keydown', onKey);
      resolve(val);
    };
    const onKey = (e) => {
      if (e.key === 'Escape') done(false);
      else if (e.key === 'Enter') done(true);
    };
    okBtn.onclick = () => done(true);
    cancelBtn.onclick = () => done(false);
    overlay.onclick = (e) => {
      if (e.target === overlay) done(false);
    };
    document.addEventListener('keydown', onKey);
  });
}

// ---------- element picker (ARIA tree) ----------

let ariaNodes = [];
let snapTimer = null;

// Re-snapshot the picker shortly after the page settles (navigation/run-step),
// debounced so a burst of redirects only snapshots once.
function scheduleSnapshot() {
  clearTimeout(snapTimer);
  snapTimer = setTimeout(() => snapshot(), 700);
}

async function snapshot() {
  const interactive = $('interactiveOnly').checked ? '1' : '0';
  const { ok, body } = await getJson(`/api/edit/snapshot?interactive=${interactive}`);
  if (!ok) {
    $('pickerEmpty').textContent = body.error || 'snapshot failed (is a page open?)';
    $('pickerEmpty').classList.remove('hidden');
    ariaNodes = [];
    $('ariaTree').innerHTML = '';
    return;
  }
  ariaNodes = (body.result && body.result.nodes) || [];
  renderTree();
}

function renderTree() {
  const filter = $('pickerFilter').value.trim().toLowerCase();
  const tree = $('ariaTree');
  tree.innerHTML = '';
  const shown = ariaNodes.filter((n) => {
    if (!filter) return true;
    return (n.role || '').toLowerCase().includes(filter) || (n.name || '').toLowerCase().includes(filter);
  });
  $('pickerEmpty').classList.toggle('hidden', shown.length > 0);
  for (const n of shown) {
    const li = document.createElement('li');
    li.className = 'aria-node' + (n.pickable ? ' pickable' : '');
    li.style.paddingLeft = `${8 + Math.min(n.depth, 12) * 12}px`;
    const nm = n.name ? ` <span class="aname">“${escapeHtml(n.name)}”</span>` : '';
    li.innerHTML = `<span class="arole">${escapeHtml(n.role || '')}</span>${nm}`;
    if (n.pickable) li.onclick = () => pickNode(n);
    tree.appendChild(li);
  }
}

function pickNode(n) {
  $('roleInput').value = n.role || '';
  $('nameInput').value = n.name || '';
  // Default to a click step when picking, unless the user is mid-assert.
  const verb = $('verbSelect').value;
  if (!['click', 'type', 'assertPresent', 'assertAbsent'].includes(verb)) {
    $('verbSelect').value = 'click';
    updateFields();
  }
  $('pickedHint').textContent = `picked ${n.role} “${n.name}”${n.ref ? ` (ref ${n.ref})` : ''}`;
}

// ---------- compose actions ----------

async function runStep() {
  const c = composePayload();
  if (c.error) return showRunResult({ ok: false, error: c.error });
  const { ok, body } = await postJson('/api/edit/run-step', { kind: c.kind, payload: c.payload });
  const report = body.result || {};
  showRunResult({
    ok: ok && report.ok,
    error: report.error || (!ok ? body.error : null),
    report,
  });
  // A live step may have changed the page; refresh the picker.
  scheduleSnapshot();
}

async function recordStep() {
  const c = composePayload();
  if (c.error) return showRunResult({ ok: false, error: c.error });
  const { ok, body } = await postJson('/api/edit/record', { kind: c.kind, payload: c.payload });
  if (!ok) return showRunResult({ ok: false, error: body.error || 'record failed' });
  showRunResult({ ok: true, recorded: true });
  $('pickedHint').textContent = '';
  await refreshBuffer();
}

function showRunResult(r) {
  const el = $('runResult');
  el.classList.remove('hidden');
  if (r.recorded) {
    el.className = 'run-result pass';
    el.textContent = '✓ recorded';
    return;
  }
  el.className = 'run-result ' + (r.ok ? 'pass' : 'fail');
  el.textContent = r.ok ? '✓ step passed' : `✗ ${r.error || 'step failed'}`;
}

// ---------- live browser (CDP screencast over SSE) ----------

let liveES = null;
const liveImg = new Image();
let liveConnected = false;

function setLiveStatus(text, cls) {
  const el = $('liveStatus');
  if (!el) return;
  el.textContent = text;
  el.className = 'live-status' + (cls ? ' ' + cls : '');
}

function drawFrame(b64) {
  const cv = $('liveCanvas');
  const ctx = cv.getContext('2d');
  liveImg.onload = () => {
    // Match the canvas backing store to the frame's pixel size once, so the
    // image is crisp; CSS scales it to fit the stage.
    if (cv.width !== liveImg.naturalWidth || cv.height !== liveImg.naturalHeight) {
      cv.width = liveImg.naturalWidth;
      cv.height = liveImg.naturalHeight;
    }
    ctx.drawImage(liveImg, 0, 0, cv.width, cv.height);
  };
  liveImg.src = 'data:image/jpeg;base64,' + b64;
}

function connectLive() {
  if (liveES) {
    liveES.close();
    liveES = null;
  }
  setLiveStatus('connecting…', 'busy');
  $('liveHint').classList.add('hidden');
  const es = new EventSource('/api/edit/stream');
  liveES = es;
  es.onmessage = (ev) => {
    try {
      const f = JSON.parse(ev.data);
      if (f.data) {
        liveConnected = true;
        setLiveStatus('live', 'ok');
        drawFrame(f.data);
      }
    } catch {
      /* ignore keep-alive comments */
    }
  };
  es.addEventListener('bridge-error', (ev) => {
    let msg = 'no live session';
    try {
      msg = JSON.parse(ev.data).error || msg;
    } catch {
      /* ignore */
    }
    setLiveStatus(msg, 'err');
    $('liveHint').textContent = msg + ' — start a session first.';
    $('liveHint').classList.remove('hidden');
  });
  es.addEventListener('url', (ev) => {
    try {
      const { url } = JSON.parse(ev.data);
      const ab = $('addressBar');
      if (url && document.activeElement !== ab) ab.value = url;
      // Page changed under us — refresh the element picker to match.
      scheduleSnapshot();
    } catch {
      /* ignore */
    }
  });
  es.addEventListener('recordable', (ev) => {
    // Fallback path (no server-side recorder): the client persists the step.
    try {
      const { kind, payload } = JSON.parse(ev.data);
      if (kind && payload) {
        enqueueRecord(kind, payload);
        flash('recorded ' + recordLabel(payload));
      }
    } catch {
      /* ignore */
    }
  });
  es.addEventListener('buffer-changed', (ev) => {
    // Server already recorded the step once; just refresh + toast.
    try {
      const { payload } = JSON.parse(ev.data);
      if (payload) flash('recorded ' + recordLabel(payload));
    } catch {
      /* ignore */
    }
    refreshBuffer();
  });
  es.addEventListener('record-skip', (ev) => {
    try {
      flash(JSON.parse(ev.data).reason || 'not recorded', true);
    } catch {
      /* ignore */
    }
  });
  es.addEventListener('loaded', () => scheduleSnapshot());
  es.onerror = () => {
    if (!liveConnected) setLiveStatus('disconnected', 'err');
  };
}

function liveNormCoords(ev) {
  const cv = $('liveCanvas');
  const r = cv.getBoundingClientRect();
  if (!r.width || !r.height) return null;
  return {
    nx: Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)),
    ny: Math.min(1, Math.max(0, (ev.clientY - r.top) / r.height)),
  };
}

function sendInput(evt) {
  fetch('/api/edit/input', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(evt),
  }).catch(() => {});
}

function wireLiveCanvas() {
  const cv = $('liveCanvas');
  if (!cv) return;
  cv.tabIndex = 0; // focusable so it can receive keystrokes
  cv.addEventListener('mousedown', (ev) => {
    const c = liveNormCoords(ev);
    if (!c) return;
    ev.preventDefault();
    const mode = $('clickMode').value;
    if (mode === 'pick') {
      pickAt(c);
    } else if (mode === 'record') {
      sendInput({ type: 'click', ...c, record: true }); // forward + auto-record
      cv.focus();
    } else {
      sendInput({ type: 'click', ...c });
      cv.focus();
    }
  });
  cv.addEventListener('wheel', (ev) => {
    if ($('clickMode').value === 'pick') return;
    const c = liveNormCoords(ev);
    if (c) sendInput({ type: 'scroll', ...c, dx: ev.deltaX, dy: ev.deltaY });
    ev.preventDefault();
  }, { passive: false });
  cv.addEventListener('keydown', (ev) => {
    const mode = $('clickMode').value;
    if (mode === 'pick') return;
    const record = mode === 'record';
    if (ev.key.length === 1) sendInput({ type: 'key', text: ev.key, record });
    else sendInput({ type: 'key', key: ev.key, record });
    if (['Enter', 'Backspace', 'Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(ev.key)) {
      ev.preventDefault();
    }
  });
  // Hover highlight: in pick/record mode, show a box + role/name label on the
  // element under the cursor (devtools-inspector style) so you can see what
  // you're about to select.
  cv.addEventListener('mousemove', onHoverMove);
  cv.addEventListener('mouseleave', hideHover);
  $('clickMode').addEventListener('change', () => {
    if ($('clickMode').value === 'interact') hideHover();
  });
}

let hoverThrottle = null;
let hoverLatest = null;
function onHoverMove(ev) {
  const mode = $('clickMode').value;
  if (mode !== 'pick' && mode !== 'record') return hideHover();
  const c = liveNormCoords(ev);
  if (!c) return;
  hoverLatest = c;
  if (hoverThrottle) return;
  hoverThrottle = setTimeout(() => {
    hoverThrottle = null;
    if (hoverLatest) inspectHover(hoverLatest);
  }, 90);
}

async function inspectHover(c) {
  const { ok, body } = await postJson('/api/edit/pick', { nx: c.nx, ny: c.ny });
  if (!ok || !body.element || $('clickMode').value === 'interact') return hideHover();
  showHover(body.element);
}

function hideHover() {
  $('liveHover').classList.add('hidden');
}

function showHover(el) {
  const box = el && el.box;
  const ov = $('liveHover');
  if (!box) return hideHover();
  const cv = $('liveCanvas');
  const r = cv.getBoundingClientRect();
  const s = $('liveStage').getBoundingClientRect();
  const offX = r.left - s.left + $('liveStage').scrollLeft;
  const offY = r.top - s.top + $('liveStage').scrollTop;
  ov.style.left = offX + box.nx * r.width + 'px';
  ov.style.top = offY + box.ny * r.height + 'px';
  ov.style.width = Math.max(2, box.nw * r.width) + 'px';
  ov.style.height = Math.max(2, box.nh * r.height) + 'px';
  const recordable = el.interactive !== false;
  ov.classList.toggle('noninteractive', !recordable);
  const tail = !recordable && $('clickMode').value === 'record' ? '  (not recordable)' : '';
  $('liveHoverLabel').textContent = (el.role || '?') + (el.name ? ' · ' + el.name : '') + tail;
  ov.classList.remove('hidden');
}

// Pick mode: clicking the live page hit-tests the element and loads its
// accessible role+name into the composer as a click step, ready to Record.
async function pickAt(c) {
  setLiveStatus('picking…', 'busy');
  const { ok, body } = await postJson('/api/edit/pick', { nx: c.nx, ny: c.ny });
  setLiveStatus('live', 'ok');
  if (!ok || !body.element) {
    flash(body.error || 'nothing pickable there', true);
    return;
  }
  const { role, name } = body.element;
  if (!role && !name) {
    flash('element has no accessible role/name', true);
    return;
  }
  $('verbSelect').value = 'click';
  updateFields();
  $('roleInput').value = role || '';
  $('nameInput').value = name || '';
  $('pickedHint').textContent = `picked ${role || '?'} “${name || ''}” — Record step to keep it`;
  flash(`picked ${role} “${name}”`);
}

function wireBrowserBar() {
  $('navBack').onclick = () => sendInput({ type: 'back' });
  $('navForward').onclick = () => sendInput({ type: 'forward' });
  $('navReload').onclick = () => (liveConnected ? sendInput({ type: 'reload' }) : connectLive());
  $('addressBar').addEventListener('keydown', (ev) => {
    if (ev.key !== 'Enter') return;
    const url = $('addressBar').value.trim();
    if (!url) return;
    if (!liveConnected) connectLive();
    // In auto-record mode, navigating the address bar records a goto step.
    sendInput({ type: 'navigate', url, record: $('clickMode').value === 'record' });
  });
}

// Human label for an auto-recorded step payload.
function recordLabel(payload) {
  const m = payload.method;
  if (m === 'clickRole') return `click ${payload.args[0]} “${payload.args[1]}”`;
  if (m === 'fillByLabel') return `fill “${payload.args[0]}”`;
  if (m === 'pressKey') return `press ${payload.args[0]}`;
  if (payload.route) return `goto ${payload.route}`;
  return 'step';
}

// Recordable events from the bridge (auto-record) are committed through the
// Rust CLI one at a time so step order is preserved.
let recordQueue = Promise.resolve();
function enqueueRecord(kind, payload) {
  recordQueue = recordQueue
    .then(() => postJson('/api/edit/record', { kind, payload }))
    .then(() => refreshBuffer())
    .catch(() => {});
}

// ---------- misc ----------

function escapeHtml(s) {
  return String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  })[c]);
}

let flashTimer = null;
function flash(msg, isError) {
  const meta = $('sessionMeta');
  meta.textContent = msg;
  meta.style.color = isError ? 'var(--fail)' : 'var(--pass)';
  clearTimeout(flashTimer);
  flashTimer = setTimeout(() => {
    meta.style.color = '';
    refreshBuffer();
  }, 2500);
}

async function init() {
  const { body } = await getJson('/api/root');
  $('rootLabel').textContent = body.scenariosRoot || '';
  if (!body.editor) {
    $('unavailableBanner').classList.remove('hidden');
    document.querySelectorAll('button, input, select').forEach((el) => (el.disabled = true));
    return;
  }
  $('verbSelect').onchange = updateFields;
  $('startBtn').onclick = startSession;
  $('flushBtn').onclick = flushScenario;
  $('cancelBtn').onclick = cancelScenario;
  $('snapshotBtn').onclick = snapshot;
  $('interactiveOnly').onchange = snapshot;
  $('pickerFilter').oninput = renderTree;
  $('refreshBufferBtn').onclick = refreshBuffer;
  $('runStepBtn').onclick = runStep;
  $('recordStepBtn').onclick = recordStep;
  wireLiveCanvas();
  wireBrowserBar();
  updateFields();
  await refreshBuffer();
  // If a session is already live (page reload), start streaming + snapshot.
  const buf = await getJson('/api/edit/buffer');
  if (buf.body && buf.body.sid) {
    connectLive();
    snapshot();
  }
}

init();
