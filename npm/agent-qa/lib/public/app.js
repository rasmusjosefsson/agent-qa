'use strict';
// agent-qa report view — vanilla JS client.
// Reads the read-only JSON API exposed by lib/report-server.js and renders
// a run viewer: scenarios + history on the left, the selected run's step
// list in the middle, and the selected step's Before/After + artifacts on
// the right. Live runs poll status.json/events.jsonl via the run endpoint.

const state = {
  root: '',
  scenarios: [],
  expanded: new Set(), // sids whose run history is open
  sel: { sid: null, runId: null, stepIdx: null, tab: 'step' },
  detail: null, // current run detail payload
  detailSig: null, // signature of rendered step detail (anti-flicker)
  scenarioDef: null, // recorded scenario.json being previewed (no run selected)
  runDef: null, // { sid, steps } — full step list of the selected run's scenario,
  //                used to pre-show not-yet-run steps during a live replay
  liveView: { es: null, sid: null }, // SSE screencast of a live replay's browser
  live: true,
  autoFollow: true, // follow an in-flight run until the user clicks a run
};

const $ = (sel) => document.querySelector(sel);

function api(pathname) {
  return fetch(pathname, { headers: { accept: 'application/json' } });
}

function artifactUrl(sid, runId, kind, stepId) {
  return `/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(
    runId,
  )}/artifact/${kind}/${encodeURIComponent(stepId)}`;
}

function fmtMs(ms) {
  if (typeof ms !== 'number') return '';
  return ms >= 1000 ? `${(ms / 1000).toFixed(1)}s` : `${ms}ms`;
}

function icon(status) {
  if (status === 'pass') return '✓';
  if (status === 'fail') return '✗';
  if (status === 'running') return '…';
  if (status === 'pending') return '○';
  return '·';
}

function el(tag, attrs = {}, ...kids) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'class') node.className = v;
    else if (k === 'text') node.textContent = v;
    else if (k.startsWith('on') && typeof v === 'function') node.addEventListener(k.slice(2), v);
    else if (v != null) node.setAttribute(k, v);
  }
  for (const kid of kids) {
    if (kid == null) continue;
    node.appendChild(typeof kid === 'string' ? document.createTextNode(kid) : kid);
  }
  return node;
}

// ---- scenarios sidebar ----

async function loadScenarios() {
  const res = await api('/api/scenarios');
  const data = await res.json();
  state.root = data.scenariosRoot;
  state.scenarios = data.scenarios || [];
  $('#rootLabel').textContent = state.root;
  renderSidebar();
  await maybeAutoFollow();
}

// While the user has not manually picked a run, follow whatever run is
// in flight (status.json state === "running", surfaced as activeRunId).
async function maybeAutoFollow() {
  if (!state.autoFollow || !state.live) return;
  const sc = state.scenarios.find((s) => s.activeRunId);
  if (!sc) return;
  if (state.sel.sid === sc.sid && state.sel.runId === sc.activeRunId) return;
  state.expanded.add(sc.sid);
  await loadRuns(sc.sid);
  await selectRun(sc.sid, sc.activeRunId, false);
}

function verdictBadge(run) {
  if (!run) return null;
  if (run.state === 'running') return el('span', { class: 'badge running', text: 'running' });
  if (run.exitCode === 0 || run.ok === true) return el('span', { class: 'badge pass', text: 'pass' });
  if (typeof run.exitCode === 'number' || run.ok === false)
    return el('span', { class: 'badge fail', text: 'fail' });
  return null;
}

function renderSidebar() {
  const list = $('#scenarioList');
  list.replaceChildren();
  if (state.scenarios.length === 0) {
    list.appendChild(el('li', { class: 'empty', text: 'No scenarios under the root yet.' }));
    return;
  }
  for (const sc of state.scenarios) {
    const li = el('li', { class: 'scenario' });
    const row = el(
      'div',
      { class: 'scenario-row', onclick: () => toggleScenario(sc.sid) },
      el(
        'div',
        { class: 'scenario-title' },
        el('span', { text: sc.scenarioId || sc.sid }),
        verdictBadge(sc.latestRun),
      ),
      el('div', { class: 'scenario-meta', text: sc.intent || sc.sid }),
    );
    li.appendChild(row);
    if (state.expanded.has(sc.sid)) li.appendChild(renderRuns(sc));
    list.appendChild(li);
  }
}

function renderRuns(sc) {
  const wrap = el('div', { class: 'runs' });
  const runs = sc._runs;
  if (!runs) {
    wrap.appendChild(el('div', { class: 'run-row muted', text: 'loading…' }));
    return wrap;
  }
  if (runs.length === 0) {
    wrap.appendChild(el('div', { class: 'run-row muted', text: 'No replays yet' }));
    return wrap;
  }
  // Newest first.
  for (const r of [...runs].reverse()) {
    const selected = state.sel.sid === sc.sid && state.sel.runId === r.runId;
    let badge;
    if (r.state === 'running') {
      badge = el('span', { class: 'badge running', text: 'running' });
    } else if (r.summary) {
      badge = el('span', {
        class: 'badge ' + (/PASS/.test(r.summary) ? 'pass' : /FAIL/.test(r.summary) ? 'fail' : ''),
        text: r.summary.replace(/^SUMMARY:\s*/, ''),
      });
    } else {
      badge = el('span', { class: 'badge running', text: 'in flight' });
    }
    wrap.appendChild(
      el(
        'div',
        {
          class: 'run-row' + (selected ? ' selected' : ''),
          onclick: () => selectRun(sc.sid, r.runId),
        },
        badge,
        el('span', { class: 'run-id', text: r.runId }),
      ),
    );
  }
  return wrap;
}

async function toggleScenario(sid) {
  if (state.expanded.has(sid)) {
    state.expanded.delete(sid);
    renderSidebar();
    return;
  }
  state.expanded.add(sid);
  renderSidebar();
  await Promise.all([loadRuns(sid), selectScenario(sid)]);
}

// Show a recorded scenario's steps in the center pane, even with zero runs.
async function selectScenario(sid) {
  state.autoFollow = false;
  closeReplayLiveView();
  state.sel = { sid, runId: null, stepIdx: null, tab: 'step' };
  state.detail = null;
  state.detailSig = null;
  try {
    const res = await api(`/api/scenarios/${encodeURIComponent(sid)}/scenario`);
    state.scenarioDef = res.ok ? (await res.json()).scenario : null;
  } catch {
    state.scenarioDef = null;
  }
  renderScenarioDef();
}

// Verb → badge category/label, mirroring the editor's step colours.
function verbCat(verb) {
  const v = String(verb || '');
  if (/^(goto|navigate)/.test(v)) return 'nav';
  if (/^click/.test(v)) return 'click';
  if (/^(fill|type)/.test(v)) return 'fill';
  if (/^press/.test(v)) return 'press';
  if (/^assert/.test(v)) return 'assert';
  if (/^wait/.test(v)) return 'wait';
  return 'action';
}
function verbBadge(verb) {
  const map = { goto: 'GO TO', navigate: 'GO TO', click: 'CLICK', fill: 'FILL', type: 'FILL', press: 'PRESS' };
  return map[verb] || String(verb || 'STEP').toUpperCase();
}
// Fallback human text when a step has no authored intent.
function stepText(st) {
  if (st.on && (st.on.role || st.on.name)) return `${st.on.role || ''} “${st.on.name || ''}”`.trim();
  if (st.value && st.value.literal != null) return String(st.value.literal);
  return st.verb || st.id || '';
}

function renderScenarioDef() {
  const def = state.scenarioDef;
  const head = $('#runHeader');
  const ol = $('#stepList');
  const pane = $('#detailPane');
  if (!def) {
    head.replaceChildren(el('div', { class: 'empty', text: 'No scenario.json for this scenario.' }));
    ol.replaceChildren();
    return;
  }
  const steps = def.steps || [];
  head.replaceChildren(
    el(
      'div',
      { class: 'def-head' },
      el('h2', {}, el('span', { text: def.intent || def.id || 'Scenario' })),
      el(
        'button',
        { class: 'btn replay-btn', onclick: (e) => replayScenario(state.sel.sid, e.currentTarget) },
        '▶ Replay',
      ),
    ),
    el('div', {
      class: 'sub',
      text: `${steps.length} step${steps.length === 1 ? '' : 's'} · recorded · not yet replayed`,
    }),
  );
  ol.replaceChildren();
  steps.forEach((st, i) => {
    ol.appendChild(
      el(
        'li',
        { class: 'step def-step' },
        el('span', { class: 'idx', text: String(i) }),
        el('span', { class: `vbadge v-${verbCat(st.verb)}`, text: verbBadge(st.verb) }),
        el('span', { class: 'label' }, el('span', { text: st.intent || stepText(st) })),
      ),
    );
  });
  if (steps.length === 0) {
    ol.appendChild(el('li', { class: 'empty', text: 'This scenario has no steps.' }));
  }
  pane.replaceChildren(
    el('div', {
      class: 'empty',
      text: 'Recorded scenario definition. Press ▶ Replay to run it and watch the steps here.',
    }),
  );
}

// Kick off a replay from the UI, then auto-follow the new run as it streams.
async function replayScenario(sid, btn) {
  if (!sid) return;
  const sc = state.scenarios.find((s) => s.sid === sid);
  const before = new Set((sc && sc._runs ? sc._runs : []).map((r) => r.runId));
  if (btn) {
    btn.disabled = true;
    btn.textContent = 'Replaying…';
  }
  let started = false;
  try {
    const res = await fetch(`/api/scenarios/${encodeURIComponent(sid)}/replay`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: '{}',
    });
    started = res.ok;
    if (!res.ok) {
      const j = await res.json().catch(() => ({}));
      alert('Replay did not start: ' + (j.error || res.status));
    }
  } catch (e) {
    alert('Replay did not start: ' + e.message);
  }
  const resetBtn = () => {
    if (btn) {
      btn.disabled = false;
      btn.textContent = '▶ Replay';
    }
  };
  if (!started) return resetBtn();
  // The run dir is minted by the replay child; poll until a new runId shows
  // up, then select it (live or finished). pollTick keeps it fresh after.
  state.autoFollow = true;
  state.expanded.add(sid);
  const deadline = Date.now() + 30000;
  const tick = async () => {
    await loadRuns(sid);
    const scc = state.scenarios.find((s) => s.sid === sid);
    const fresh = ((scc && scc._runs) || []).filter((r) => !before.has(r.runId));
    if (fresh.length) {
      fresh.sort((a, b) => (a.runId < b.runId ? 1 : -1));
      resetBtn();
      await selectRun(sid, fresh[0].runId, false);
      return;
    }
    if (Date.now() < deadline) setTimeout(tick, 700);
    else resetBtn();
  };
  setTimeout(tick, 500);
}

// ---- live replay screencast (watch the browser while it replays) ----

// Stream the replay session's CDP screencast into the detail pane. Frames
// arrive as SSE `message` events carrying { data: <base64 jpeg> } — the same
// shape the editor's live pane consumes. Read-only: no input is forwarded.
function ensureReplayLiveView(sid) {
  if (state.liveView.es && state.liveView.sid === sid) return; // already streaming
  closeReplayLiveView();
  const img = el('img', {
    class: 'live-shot',
    alt: 'live replay browser',
    title: 'Click to enlarge',
    onclick: () => {
      if (img.src) openLightbox(img.src, 'Live replay browser');
    },
  });
  $('#detailPane').replaceChildren(
    el(
      'div',
      { class: 'live-wrap' },
      el('div', { class: 'live-bar' }, el('span', { class: 'live-dot', text: '● live browser' })),
      img,
    ),
  );
  const es = new EventSource(`/api/scenarios/${encodeURIComponent(sid)}/replay-stream`);
  state.liveView = { es, sid };
  es.onmessage = (ev) => {
    try {
      const f = JSON.parse(ev.data);
      if (f.data) img.src = 'data:image/jpeg;base64,' + f.data;
    } catch {
      /* keep-alive comment */
    }
  };
}

function closeReplayLiveView() {
  if (state.liveView.es) {
    try {
      state.liveView.es.close();
    } catch {
      /* noop */
    }
  }
  state.liveView = { es: null, sid: null };
}

async function loadRuns(sid) {
  const res = await api(`/api/scenarios/${encodeURIComponent(sid)}/runs`);
  const data = await res.json();
  const sc = state.scenarios.find((s) => s.sid === sid);
  if (sc) sc._runs = data.replays || [];
  renderSidebar();
}

// ---- run + steps ----

async function selectRun(sid, runId, manual = true) {
  if (manual) state.autoFollow = false;
  state.scenarioDef = null; // a run takes over the center pane
  state.sel = { sid, runId, stepIdx: null, tab: 'step' };
  state.detailSig = null;
  await refreshRun();
}

// Collapse running+terminal rows into one entry per idx (terminal wins).
function collapseEvents(events) {
  const byIdx = new Map();
  for (const e of events) {
    if (typeof e.idx !== 'number') continue;
    byIdx.set(e.idx, { ...(byIdx.get(e.idx) || {}), ...e });
  }
  return [...byIdx.values()].sort((a, b) => a.idx - b.idx);
}

async function refreshRun() {
  const { sid, runId } = state.sel;
  if (!sid || !runId) return;
  // Fetch the scenario definition once per sid so renderSteps can pre-show
  // not-yet-run steps in a pending state during a live replay.
  if (!state.runDef || state.runDef.sid !== sid) {
    state.runDef = { sid, steps: [] };
    try {
      const dres = await api(`/api/scenarios/${encodeURIComponent(sid)}/scenario`);
      if (dres.ok) state.runDef = { sid, steps: (await dres.json()).scenario.steps || [] };
    } catch {
      /* best-effort: fall back to events-only rendering */
    }
  }
  const res = await api(`/api/scenarios/${encodeURIComponent(sid)}/runs/${encodeURIComponent(runId)}`);
  if (!res.ok) return;
  state.detail = await res.json();
  renderRunHeader();
  renderSteps();
  // While the run is live and no step is pinned, show the browser live;
  // otherwise fall back to the static step detail.
  if (isRunLive(state.detail) && state.sel.stepIdx == null) {
    ensureReplayLiveView(sid);
  } else {
    closeReplayLiveView();
    renderDetail();
  }
}

function isRunLive(detail) {
  return detail && detail.status && detail.status.state === 'running';
}

function renderRunHeader() {
  const head = $('#runHeader');
  const d = state.detail;
  if (!d) {
    head.replaceChildren(el('div', { class: 'empty', text: 'Select a run from the left.' }));
    return;
  }
  const a = d.audit || {};
  const s = d.status || {};
  const live = isRunLive(d);
  const summary = a.summary || (live ? `running ${s.currentIdx || 0}/${s.total || '?'}` : 'in flight');
  const cls = /PASS/.test(summary) ? 'pass' : /FAIL/.test(summary) ? 'fail' : 'running';
  head.replaceChildren(
    el(
      'h2',
      {},
      el('span', { class: 'badge ' + cls, text: summary.replace(/^SUMMARY:\s*/, '') }),
      live ? el('span', { class: 'live-dot', text: '  ● live' }) : null,
    ),
    el('div', { class: 'sub', text: d.runId }),
  );
}

function renderSteps() {
  const ol = $('#stepList');
  const d = state.detail;
  ol.replaceChildren();
  if (!d) return;
  const events = collapseEvents(d.events || []);
  const currentIdx = d.status && typeof d.status.currentIdx === 'number' ? d.status.currentIdx : -1;
  const liveCurrent = isRunLive(d) ? currentIdx : -1;

  // Merge the full scenario step list (if known) with the run events so the
  // not-yet-run steps appear up front in a disabled "pending" state instead of
  // popping in only when their turn comes.
  const defSteps = state.runDef && state.runDef.sid === state.sel.sid ? state.runDef.steps : null;
  let rows = events;
  if (defSteps && defSteps.length) {
    const byId = new Map(events.map((e) => [e.id, e]));
    rows = defSteps.map((ds, i) => {
      const ev = byId.get(ds.id);
      if (ev) return ev; // executed / running — use the real event
      return {
        idx: i + 1,
        id: ds.id,
        intent: ds.intent || stepText(ds),
        kind: ds.verb,
        status: 'pending',
        total: defSteps.length,
        pending: true,
      };
    });
  }

  for (const st of rows) {
    const selected = state.sel.stepIdx === st.idx;
    const isCurrent = st.idx === liveCurrent;
    const pending = st.status === 'pending';
    ol.appendChild(
      el(
        'li',
        {
          class:
            'step' +
            (selected ? ' selected' : '') +
            (isCurrent ? ' current' : '') +
            (pending ? ' pending' : ''),
          onclick: pending ? null : () => selectStep(st.idx),
        },
        el('span', { class: 'ico ' + st.status, text: icon(st.status) }),
        el('span', { class: 'idx', text: `${st.idx}/${st.total || rows.length}` }),
        el(
          'span',
          { class: 'label' },
          el('span', { text: st.intent || st.id }),
          ' ',
          el('span', { class: 'kind', text: st.kind ? `(${st.kind})` : '' }),
        ),
        el('span', { class: 'ms', text: pending ? '' : fmtMs(st.ms) }),
      ),
    );
  }
  if (rows.length === 0) {
    ol.appendChild(el('li', { class: 'empty', text: 'No steps recorded for this run.' }));
  }
}

function selectStep(idx) {
  closeReplayLiveView(); // clicking a step swaps the live view for its detail
  state.sel.stepIdx = idx;
  state.detailSig = null;
  renderSteps();
  renderDetail();
}

function selectTab(tab) {
  state.sel.tab = tab;
  state.detailSig = null;
  renderDetail();
}

function stepByIdx(steps, idx) {
  return steps.find((s) => s.idx === idx) || null;
}

function renderDetail() {
  const pane = $('#detailPane');
  const d = state.detail;
  if (!d || state.sel.stepIdx == null) {
    pane.replaceChildren(el('div', { class: 'empty', text: 'Select a step to see details.' }));
    return;
  }
  const steps = collapseEvents(d.events || []);
  const step = stepByIdx(steps, state.sel.stepIdx);
  if (!step) {
    pane.replaceChildren(el('div', { class: 'empty', text: 'Step not found.' }));
    return;
  }
  // Anti-flicker: skip re-render if nothing about this view changed.
  const sig = JSON.stringify([
    state.sel.sid,
    state.sel.runId,
    step.idx,
    step.status,
    step.ms,
    step.error,
    step.screenshot,
    state.sel.tab,
  ]);
  if (sig === state.detailSig) return;
  state.detailSig = sig;

  const prev = stepByIdx(steps, step.idx - 1);
  const { bar, body } = renderTabs(step);
  pane.replaceChildren(
    el(
      'div',
      { class: 'detail-head' },
      el(
        'h3',
        {},
        el('span', { class: 'ico ' + step.status, text: icon(step.status) + ' ' }),
        step.intent || step.id,
      ),
      el('div', { class: 'kind', text: `${step.kind || ''} · step ${step.idx}/${step.total || ''}` }),
    ),
    renderShots(step, prev), // fixed-height, so the tab bar below stays put
    bar, // tab bar sits at a constant position under the (fixed) shots
    el(
      'div',
      { class: 'detail-scroll' },
      step.error ? el('div', { class: 'error-box', text: step.error }) : null,
      body,
    ),
  );
}

function renderShots(step, prev) {
  const wrap = el('div', { class: 'shots' });
  wrap.appendChild(shotCol('Before', prev));
  wrap.appendChild(shotCol('After', step));
  return wrap;
}

function shotCol(title, step) {
  const col = el('div', { class: 'shot' }, el('h4', { text: title }));
  const { sid, runId } = state.sel;
  if (step && step.screenshot) {
    const url = artifactUrl(sid, runId, 'screenshots', step.id);
    const caption = `${title} · ${step.intent || step.id}`;
    col.appendChild(
      el(
        'a',
        {
          href: url, // ⌘/middle-click still opens the original; plain click = overlay
          onclick: (e) => {
            if (e.metaKey || e.ctrlKey || e.shiftKey || e.button === 1) return;
            e.preventDefault();
            openLightbox(url, caption);
          },
        },
        el('img', { src: url, alt: title, loading: 'lazy' }),
      ),
    );
  } else {
    col.appendChild(el('div', { class: 'none', text: 'not captured' }));
  }
  return col;
}

// shadcn Dialog-style image viewer: blurred backdrop + centered card,
// dismissed by backdrop click, the ✕ button, or Escape.
let lightboxKeyHandler = null;
function openLightbox(url, caption) {
  closeLightbox();
  const card = el(
    'div',
    { class: 'lightbox-card', onclick: (e) => e.stopPropagation() },
    el(
      'div',
      { class: 'lightbox-bar' },
      el('span', { class: 'lightbox-cap', text: caption || '', title: caption || '' }),
      el(
        'div',
        { class: 'lightbox-actions' },
        el('a', { class: 'lightbox-open', href: url, target: '_blank', rel: 'noopener', text: 'Open original ↗' }),
        el('button', { class: 'lightbox-close', title: 'Close (Esc)', 'aria-label': 'Close', onclick: closeLightbox }, '✕'),
      ),
    ),
    el('img', { class: 'lightbox-img', src: url, alt: caption || '' }),
  );
  const overlay = el('div', { class: 'lightbox-overlay', onclick: closeLightbox }, card);
  overlay.id = 'lightbox';
  document.body.appendChild(overlay);
  lightboxKeyHandler = (e) => {
    if (e.key === 'Escape') closeLightbox();
  };
  document.addEventListener('keydown', lightboxKeyHandler);
}
function closeLightbox() {
  const ex = document.getElementById('lightbox');
  if (ex) ex.remove();
  if (lightboxKeyHandler) {
    document.removeEventListener('keydown', lightboxKeyHandler);
    lightboxKeyHandler = null;
  }
}

function renderTabs(step) {
  const tabs = ['step', 'context', 'network', 'html', 'console'];
  const labels = { step: 'Step', context: 'Context', network: 'Network', html: 'HTML', console: 'Console' };
  const bar = el('div', { class: 'tabs' });
  for (const t of tabs) {
    bar.appendChild(
      el('div', { class: 'tab' + (state.sel.tab === t ? ' active' : ''), onclick: () => selectTab(t) }, labels[t]),
    );
  }
  const body = el('div', { class: 'tab-body', id: 'tabBody' });
  fillTab(body, step, state.sel.tab);
  return { bar, body };
}

async function fillTab(body, step, tab) {
  const { sid, runId } = state.sel;
  if (tab === 'step') {
    const dl = el('dl', { class: 'kv' });
    const rows = [
      ['id', step.id],
      ['kind', step.kind],
      ['status', step.status],
      ['duration', fmtMs(step.ms)],
      ['snapshot', step.snapshot || '(none)'],
      ['screenshot', step.screenshot || '(none)'],
    ];
    for (const [k, v] of rows) {
      dl.appendChild(el('dt', { text: k }));
      dl.appendChild(el('dd', { text: v == null || v === '' ? '—' : String(v) }));
    }
    body.replaceChildren(dl);
    return;
  }
  if (tab === 'console') {
    body.replaceChildren(el('div', { class: 'not-captured', text: 'Console output is not captured for this step.' }));
    return;
  }
  // context → snapshots/<id>.txt ; network → network/<id>.json ; html → probes/<id>.json
  const kind = tab === 'context' ? 'snapshots' : tab === 'network' ? 'network' : 'probes';
  body.replaceChildren(el('div', { class: 'not-captured', text: 'Loading…' }));
  try {
    const res = await fetch(artifactUrl(sid, runId, kind, step.id));
    if (!res.ok) {
      body.replaceChildren(el('div', { class: 'not-captured', text: 'Not captured for this step.' }));
      return;
    }
    let text = await res.text();
    if (kind !== 'snapshots') {
      try {
        text = JSON.stringify(JSON.parse(text), null, 2);
      } catch {
        /* leave raw */
      }
    }
    body.replaceChildren(el('pre', { class: 'dump', text }));
  } catch {
    body.replaceChildren(el('div', { class: 'not-captured', text: 'Not captured for this step.' }));
  }
}

// ---- live polling ----

async function pollTick() {
  if (!state.live) return;
  // Refresh the selected run if it is live.
  if (state.detail && isRunLive(state.detail)) {
    await refreshRun();
  } else if (state.sel.sid && state.sel.runId) {
    // Run finished or status unknown — one more refresh, cheap.
    await refreshRun();
  }
  // Keep the scenarios list (and any open run histories) fresh.
  await loadScenarios();
  for (const sid of state.expanded) await loadRuns(sid);
}

function setupLive() {
  $('#liveToggle').addEventListener('change', (e) => {
    state.live = e.target.checked;
  });
  setInterval(() => {
    pollTick().catch(() => {});
  }, 1500);
}

// ---- boot ----

$('#refreshBtn').addEventListener('click', () => {
  loadScenarios().catch(() => {});
  if (state.sel.sid && state.sel.runId) refreshRun().catch(() => {});
});

loadScenarios().catch((e) => {
  $('#scenarioList').replaceChildren(el('li', { class: 'empty', text: 'Failed to load: ' + e.message }));
});
setupLive();
