'use strict';
// agent-qa report viewer.
//
// A localhost-only, read-only web view over the per-run sidecar tree.
// It is a *consumer* of `events.jsonl` / `status.json` / `audit.json`
// (spec: docs/specs/scenario-sidecar-tree.md); it never drives a browser,
// never writes, never runs as a daemon. Hosted by the Node launcher so
// there is no new Rust crate.
//
// Discovery is by stable path, never by chasing a JSON field:
//   <root>/<sid>/scenario.json
//   <root>/<sid>/replays/latest.txt          → latest <runId>
//   <root>/<sid>/replays/<runId>/audit.json  → run metadata
//   <root>/<sid>/replays/<runId>/status.json → live cursor
//   <root>/<sid>/replays/<runId>/events.jsonl→ per-step stream
//   <root>/<sid>/replays/<runId>/<kind>/<stepId>.<ext> → per-step sidecars

const http = require('node:http');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const { createReadStream } = require('node:fs');
const { execFile } = require('node:child_process');
const { createLiveBridge } = require('./live-bridge.js');

const PUBLIC_DIR = path.join(__dirname, 'public');

// Editor trigger kinds, mirrors recorder_shape::TriggerKind. The
// Rust CLI re-validates, but we gate here too so the write surface can
// never shell an unexpected verb shape.
const EDIT_KINDS = ['navigation', 'action', 'wait', 'assert'];

// Per-step sidecar kinds that may be streamed by the artifact endpoint.
// Mirrors the fixed extensions in scenario-sidecar-tree.md § Path conventions.
const ARTIFACT_KINDS = {
  screenshots: { ext: '.png', type: 'image/png' },
  snapshots: { ext: '.txt', type: 'text/plain; charset=utf-8' },
  network: { ext: '.json', type: 'application/json; charset=utf-8' },
  probes: { ext: '.json', type: 'application/json; charset=utf-8' },
  perf: { ext: '.json', type: 'application/json; charset=utf-8' },
};

const STATIC_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
};

// Static files served by name (fixed allowlist; no user input reaches the
// filesystem path).
const STATIC_FILES = {
  '/': 'index.html',
  '/index.html': 'index.html',
  '/app.js': 'app.js',
  '/styles.css': 'styles.css',
  '/editor': 'editor.html',
  '/editor.html': 'editor.html',
  '/editor.js': 'editor.js',
};

// -------- path safety --------

// Mirror of Rust `is_safe_segment` (cli/src/sidecar.rs): non-empty, not
// `.`/`..`, and every char in `[A-Za-z0-9._-]`. Used to clamp every
// caller-supplied path segment so the file endpoint cannot escape the
// run directory.
function isSafeSegment(s) {
  return (
    typeof s === 'string' &&
    s.length > 0 &&
    s !== '.' &&
    s !== '..' &&
    /^[A-Za-z0-9._-]+$/.test(s)
  );
}

// -------- scenarios root resolution (mirror cli/src/paths.rs) --------

// Minimal `[paths].scenarios_root` reader. Not a full TOML parser — the
// env var and the `--root` flag both override this, so a line scanner for
// the one key we care about keeps the launcher dependency-free.
function scenariosRootFromToml(tomlPath) {
  let text;
  try {
    text = fs.readFileSync(tomlPath, 'utf8');
  } catch {
    return null;
  }
  let inPaths = false;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.replace(/#.*$/, '').trim();
    if (!line) continue;
    const section = line.match(/^\[(.+)\]$/);
    if (section) {
      inPaths = section[1].trim() === 'paths';
      continue;
    }
    if (!inPaths) continue;
    const m = line.match(/^scenarios_root\s*=\s*(.+)$/);
    if (m) {
      const v = m[1].trim().replace(/^["']|["']$/g, '');
      if (v) return v;
    }
  }
  return null;
}

function resolveRelative(value, base) {
  return path.isAbsolute(value) ? value : path.resolve(base, value);
}

// Priority (first hit wins), matching cli/src/paths.rs::scenarios_root:
//   1. explicit `--root` (cwd-relative)
//   2. AGENT_QA_SCENARIOS_DIR env (cwd-relative)
//   3. agent-qa.toml [paths].scenarios_root (relative to the toml dir)
//   4. default <cwd>/tmp/agent-qa-scenarios
function resolveScenariosRoot(opts = {}) {
  const cwd = opts.cwd || process.cwd();
  const env = opts.env || process.env;
  if (opts.root) return resolveRelative(opts.root, cwd);
  const fromEnv = env.AGENT_QA_SCENARIOS_DIR;
  if (fromEnv) return resolveRelative(fromEnv, cwd);
  let dir = cwd;
  for (;;) {
    for (const name of ['agent-qa.toml', '.agent-qa.toml']) {
      const candidate = path.join(dir, name);
      if (fs.existsSync(candidate)) {
        const v = scenariosRootFromToml(candidate);
        if (v) return resolveRelative(v, dir);
      }
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return path.join(cwd, 'tmp', 'agent-qa-scenarios');
}

// Mirror of cli/src/paths.rs::record_root — the ephemeral recording
// scratch dir holding scenario.env + scenario.steps.jsonl during an
// authoring session. Override with AGENT_QA_RECORD_DIR, else
// <cwd>/tmp/agent-qa-record.
function resolveRecordRoot(opts = {}) {
  const cwd = opts.cwd || process.cwd();
  const env = opts.env || process.env;
  if (env.AGENT_QA_RECORD_DIR) return resolveRelative(env.AGENT_QA_RECORD_DIR, cwd);
  return path.join(cwd, 'tmp', 'agent-qa-record');
}

// Parse the shell-ish `KEY=value` recording env file `start` writes.
// INTENT is single-quoted with start.rs's escaping; unwrap it.
function parseEnvFile(text) {
  const out = {};
  if (typeof text !== 'string') return out;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    const eq = line.indexOf('=');
    if (eq <= 0) continue;
    const key = line.slice(0, eq).trim();
    let val = line.slice(eq + 1).trim();
    if (val.startsWith("'") && val.endsWith("'") && val.length >= 2) {
      val = val.slice(1, -1).replace(/'\\''/g, "'");
    }
    out[key] = val;
  }
  return out;
}

// -------- small fs readers (absence is meaningful, never an error) --------

async function readJson(p) {
  try {
    return JSON.parse(await fsp.readFile(p, 'utf8'));
  } catch {
    return null;
  }
}

async function readText(p) {
  try {
    return await fsp.readFile(p, 'utf8');
  } catch {
    return null;
  }
}

// Parse an append-only events.jsonl into an array of rows. Tolerates a
// trailing partial line (the file may be mid-write) and skips garbage.
async function readEvents(p) {
  const text = await readText(p);
  if (text == null) return [];
  const rows = [];
  for (const line of text.split('\n')) {
    const t = line.trim();
    if (!t) continue;
    try {
      rows.push(JSON.parse(t));
    } catch {
      // Partial / truncated line — ignore, do not fail the request.
    }
  }
  return rows;
}

async function latestRunId(scenarioDir) {
  const txt = await readText(path.join(scenarioDir, 'replays', 'latest.txt'));
  return txt == null ? null : txt.trim() || null;
}

// -------- run enumeration --------

async function listRuns(scenarioDir) {
  const replaysDir = path.join(scenarioDir, 'replays');
  let entries;
  try {
    entries = await fsp.readdir(replaysDir, { withFileTypes: true });
  } catch {
    return [];
  }
  const runs = [];
  for (const ent of entries) {
    if (!ent.isDirectory()) continue;
    const runId = ent.name;
    if (!isSafeSegment(runId)) continue;
    const [audit, status] = await Promise.all([
      readJson(path.join(replaysDir, runId, 'audit.json')),
      readJson(path.join(replaysDir, runId, 'status.json')),
    ]);
    runs.push({
      runId,
      startedAt: audit?.startedAt ?? null,
      finishedAt: audit?.finishedAt ?? null,
      summary: audit?.summary ?? null,
      exitCode: typeof audit?.exitCode === 'number' ? audit.exitCode : null,
      profile: audit?.profile ?? null,
      tag: audit?.tag ?? null,
      // Live cursor — lets the UI render a "running" badge before audit.json
      // gains its final summary (audit.json is written empty-ish at start).
      state: status?.state ?? null,
      ok: status?.ok ?? null,
    });
  }
  // Sort by runId — the timestamp prefix makes this chronological. We do
  // NOT use this to pick "latest"; that is what replays/latest.txt is for.
  runs.sort((a, b) => (a.runId < b.runId ? -1 : a.runId > b.runId ? 1 : 0));
  return runs;
}

// Find an in-flight run by content (status.json state === "running"), NOT
// by timestamp-sorting the directory. `latest.txt` is only written when a
// run finishes (cli/src/runner.rs updates it after the step loop), so it
// cannot point at an active run; status.json is the live discriminator.
// If several are running (parallel runs), break the tie by the greatest
// runId — its timestamp prefix is the newest. This tie-break is scoped to
// *live* selection and never overrides the canonical `latest.txt` pointer.
async function findActiveRunId(scenarioDir) {
  const replaysDir = path.join(scenarioDir, 'replays');
  let entries;
  try {
    entries = await fsp.readdir(replaysDir, { withFileTypes: true });
  } catch {
    return null;
  }
  let active = null;
  for (const ent of entries) {
    if (!ent.isDirectory() || !isSafeSegment(ent.name)) continue;
    const status = await readJson(path.join(replaysDir, ent.name, 'status.json'));
    if (status?.state === 'running' && (active == null || ent.name > active)) {
      active = ent.name;
    }
  }
  return active;
}

async function scenarioSummary(root, sid) {
  const dir = path.join(root, sid);
  const scenario = await readJson(path.join(dir, 'scenario.json'));
  const latest = await latestRunId(dir);
  const activeRunId = await findActiveRunId(dir);
  // Prefer the in-flight run for the "current" view; fall back to the
  // canonical completed pointer from latest.txt.
  const currentRunId = activeRunId || latest;
  let latestRun = null;
  if (currentRunId && isSafeSegment(currentRunId)) {
    const runDir = path.join(dir, 'replays', currentRunId);
    const [audit, status] = await Promise.all([
      readJson(path.join(runDir, 'audit.json')),
      readJson(path.join(runDir, 'status.json')),
    ]);
    latestRun = {
      runId: currentRunId,
      summary: audit?.summary ?? null,
      exitCode: typeof audit?.exitCode === 'number' ? audit.exitCode : null,
      startedAt: audit?.startedAt ?? null,
      finishedAt: audit?.finishedAt ?? null,
      state: status?.state ?? null,
      currentIdx: typeof status?.currentIdx === 'number' ? status.currentIdx : null,
      total: typeof status?.total === 'number' ? status.total : null,
      ok: status?.ok ?? null,
    };
  }
  return {
    sid,
    dir,
    scenarioId: scenario?.id ?? null,
    hasScenario: !!scenario,
    intent: scenario?.intent ?? null,
    steps: Array.isArray(scenario?.steps) ? scenario.steps.length : null,
    latestRunId: latest,
    activeRunId,
    latestRun,
  };
}

async function listScenarios(root) {
  let entries;
  try {
    entries = await fsp.readdir(root, { withFileTypes: true });
  } catch {
    return [];
  }
  const sids = entries
    .filter((e) => e.isDirectory() && isSafeSegment(e.name))
    .map((e) => e.name)
    .sort();
  const out = [];
  for (const sid of sids) {
    out.push(await scenarioSummary(root, sid));
  }
  // Only surface real scenarios: a dir is shown once it has recorded steps
  // (a flushed scenario.json with steps) or it already has replay runs.
  // Un-flushed/empty recording buffers, start skeletons, and cancelled
  // sessions (0 steps, no runs) are hidden so the list isn't cluttered.
  return out.filter((s) => (s.steps && s.steps > 0) || s.latestRunId);
}

async function runDetail(root, sid, runId) {
  const runDir = path.join(root, sid, 'replays', runId);
  const [audit, status, events, latest] = await Promise.all([
    readJson(path.join(runDir, 'audit.json')),
    readJson(path.join(runDir, 'status.json')),
    readEvents(path.join(runDir, 'events.jsonl')),
    latestRunId(path.join(root, sid)),
  ]);
  return {
    sid,
    runId,
    isLatest: latest === runId,
    audit,
    status,
    events,
  };
}

// -------- http helpers --------

function sendJson(res, code, body) {
  const buf = Buffer.from(JSON.stringify(body));
  res.writeHead(code, {
    'content-type': 'application/json; charset=utf-8',
    'content-length': buf.length,
    'cache-control': 'no-store',
  });
  res.end(buf);
}

function notFound(res, msg) {
  sendJson(res, 404, { error: msg || 'not found' });
}

function badRequest(res, msg) {
  sendJson(res, 400, { error: msg || 'bad request' });
}

async function serveStatic(res, relName) {
  // relName is from a fixed allowlist (no user input), but resolve+clamp
  // anyway to keep the static handler honest.
  const full = path.join(PUBLIC_DIR, relName);
  if (!full.startsWith(PUBLIC_DIR + path.sep) && full !== PUBLIC_DIR) {
    return notFound(res, 'not found');
  }
  let stat;
  try {
    stat = await fsp.stat(full);
  } catch {
    return notFound(res, 'not found');
  }
  if (!stat.isFile()) return notFound(res, 'not found');
  const type = STATIC_TYPES[path.extname(full)] || 'application/octet-stream';
  res.writeHead(200, { 'content-type': type, 'content-length': stat.size });
  createReadStream(full).pipe(res);
}

async function serveArtifact(res, root, sid, runId, kind, stepId) {
  if (![sid, runId, stepId].every(isSafeSegment)) {
    return badRequest(res, 'unsafe path segment');
  }
  const spec = ARTIFACT_KINDS[kind];
  if (!spec) return badRequest(res, `unknown artifact kind ${kind}`);

  const runDir = path.resolve(root, sid, 'replays', runId);
  const full = path.resolve(runDir, kind, stepId + spec.ext);
  // Clamp to the run directory — defence in depth on top of isSafeSegment.
  if (full !== runDir && !full.startsWith(runDir + path.sep)) {
    return badRequest(res, 'path escapes run dir');
  }
  let stat;
  try {
    stat = await fsp.stat(full);
  } catch {
    // Absence is meaningful (capture skipped) — say so, do not 500.
    return notFound(res, 'not captured');
  }
  if (!stat.isFile()) return notFound(res, 'not captured');
  res.writeHead(200, {
    'content-type': spec.type,
    'content-length': stat.size,
    'cache-control': 'no-store',
  });
  createReadStream(full).pipe(res);
}

// -------- editor — write surface via the Rust CLI --------
//
// The authoring editor needs a *write* path (start a recording session,
// record/run/reorder steps, flush a scenario.json). All writes go through
// the existing Rust plumbing — never to a network
// service and never by the Node server hand-editing the scenario tree.
// This server only shells the Rust CLI (localhost-only) and relays the
// structured result. Args are passed as an array to execFile (no shell),
// so a scenario intent / payload can never inject a command.

// Build a function that runs the agent-qa Rust CLI with a fixed binary +
// child env. Resolves to { code, stdout, stderr, spawnError }. A missing
// binary surfaces as spawnError (→ 500) rather than a fake exit code.
function makeCliRunner({ bin, env, cwd }) {
  return function runCli(args) {
    return new Promise((resolve) => {
      execFile(
        bin,
        args,
        { env, cwd, maxBuffer: 32 * 1024 * 1024 },
        (err, stdout, stderr) => {
          if (err && typeof err.code === 'string') {
            // Spawn failure (ENOENT, EACCES, …) — not a process exit.
            resolve({ code: null, stdout: '', stderr: '', spawnError: err });
            return;
          }
          const code = err && typeof err.code === 'number' ? err.code : 0;
          resolve({ code, stdout: stdout || '', stderr: stderr || '', spawnError: null });
        },
      );
    });
  };
}

// agent-browser session name a UI-triggered replay drives. Deterministic
// per sid so the live screencast can attach to exactly that browser while
// the replay runs.
function replaySessionFor(sid) {
  return `replay-${sid}`;
}

// Build a getCdpUrl() that resolves a session's CDP WebSocket endpoint via
// the Rust `cdp-url` verb. session=null → the recorder's env session.
function cdpUrlResolver(runCli, session) {
  return async () => {
    const args = ['cdp-url', ...(session ? ['--session', session] : []), '--json'];
    const r = await runCli(args);
    if (r.spawnError) throw new Error(String(r.spawnError.message || r.spawnError));
    if (r.code !== 0) throw new Error((r.stderr || 'cdp-url failed').trim());
    const j = lastJsonLine(r.stdout) || {};
    if (!j.url) throw new Error('cdp-url returned no url (no live session?)');
    return j.url;
  };
}

// Spawn a replay of a recorded scenario as a detached child. Replay is
// long-running (it drives a browser through every step), so we do NOT wait
// for it to finish — the viewer's live poll auto-follows the new run via
// status.json. Resolves quickly to { ok, pid } | { ok:false, error }.
function makeReplaySpawner({ bin, env, cwd }) {
  const { spawn } = require('node:child_process');
  return function replay(sid, session) {
    return new Promise((resolve) => {
      const args = ['replay', sid, ...(session ? ['--session', session] : [])];
      const child = spawn(bin, args, { env, cwd, stdio: 'ignore', detached: true });
      let done = false;
      child.once('spawn', () => {
        if (done) return;
        done = true;
        child.unref();
        resolve({ ok: true, pid: child.pid });
      });
      child.once('error', (err) => {
        if (done) return;
        done = true;
        resolve({ ok: false, error: String((err && err.message) || err) });
      });
    });
  };
}

// Parse the last JSON object emitted on stdout (our CLI verbs print one
// JSON line). Returns null if none parses.
function lastJsonLine(stdout) {
  const lines = String(stdout || '')
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean);
  for (let i = lines.length - 1; i >= 0; i--) {
    try {
      return JSON.parse(lines[i]);
    } catch {
      // keep scanning upward
    }
  }
  return null;
}

// Read a size-limited JSON request body.
function readJsonBody(req, limit = 512 * 1024) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    req.on('data', (c) => {
      size += c.length;
      if (size > limit) {
        reject(new Error('request body too large'));
        req.destroy();
        return;
      }
      chunks.push(c);
    });
    req.on('end', () => {
      const text = Buffer.concat(chunks).toString('utf8');
      if (!text.trim()) return resolve({});
      try {
        resolve(JSON.parse(text));
      } catch {
        reject(new Error('invalid JSON body'));
      }
    });
    req.on('error', reject);
  });
}

// Relay a CLI result to the client: spawn failure → 500; non-zero exit →
// 422 with stderr; success → 200 with parsed JSON (if any) merged in.
function sendCliResult(res, r, extra = {}) {
  if (r.spawnError) {
    return sendJson(res, 500, {
      error: `agent-qa CLI failed to launch: ${r.spawnError.message || r.spawnError}`,
    });
  }
  const parsed = lastJsonLine(r.stdout);
  if (r.code !== 0) {
    return sendJson(res, 422, {
      ok: false,
      code: r.code,
      error: (r.stderr || r.stdout || 'command failed').trim(),
      ...(parsed ? { result: parsed } : {}),
      ...extra,
    });
  }
  return sendJson(res, 200, {
    ok: true,
    code: 0,
    stdout: r.stdout,
    stderr: r.stderr,
    ...(parsed ? { result: parsed } : {}),
    ...extra,
  });
}

async function readBuffer(deps) {
  const r = await deps.runCli(['buffer', 'list', '--json']);
  const parsed = lastJsonLine(r.stdout);
  const rows = parsed && Array.isArray(parsed.rows) ? parsed.rows : [];
  // Read the session env (sid / intent) directly — it is recording scratch,
  // not the scenario contract.
  let env = {};
  try {
    env = parseEnvFile(await fsp.readFile(path.join(deps.recordRoot, 'scenario.env'), 'utf8'));
  } catch {
    env = {};
  }
  return {
    sid: env.SID || null,
    intent: env.INTENT || null,
    session: env.SESSION || null,
    baseline: env.BASELINE || null,
    rows,
    spawnError: r.spawnError ? String(r.spawnError.message || r.spawnError) : null,
  };
}

async function handleEdit(req, res, deps, seg) {
  // seg: path segments after ['api','edit']
  const route = seg.join('/');
  const method = req.method;

  if (route === 'buffer' && method === 'GET') {
    return sendJson(res, 200, await readBuffer(deps));
  }
  if (route === 'snapshot' && method === 'GET') {
    const url = new URL(req.url, 'http://127.0.0.1');
    const args = ['aria-snapshot'];
    if (url.searchParams.get('interactive') === '1') args.push('--interactive');
    const r = await deps.runCli(args);
    return sendCliResult(res, r);
  }

  // Live-browser screencast (SSE). Frames are base64 JPEGs pushed as they
  // are captured; the client renders them and posts input back to /input.
  if (route === 'stream' && method === 'GET') {
    if (!deps.live) return sendJson(res, 503, { error: 'live browser unavailable' });
    res.writeHead(200, {
      'content-type': 'text/event-stream',
      'cache-control': 'no-cache',
      connection: 'keep-alive',
      'x-accel-buffering': 'no',
    });
    res.write(': connected\n\n');
    deps.live.subscribe(res);
    req.on('close', () => deps.live.unsubscribe(res));
    return undefined;
  }

  if (method !== 'POST') {
    return sendJson(res, 405, { error: 'method not allowed' });
  }
  const body = await readJsonBody(req);

  switch (route) {
    case 'start': {
      const intent = String(body.intent || '').trim();
      if (!intent) return badRequest(res, 'intent is required');
      const args = ['start', intent];
      if (body.url) args.push('--open', String(body.url));
      if (body.profile) args.push('--profile', String(body.profile));
      else if (body.keepSession) args.push('--keep-session');
      const r = await deps.runCli(args);
      // Surface the freshly-minted sid for the UI.
      const m = /started sid=(\S+)/.exec(r.stdout || '');
      return sendCliResult(res, r, m ? { sid: m[1] } : {});
    }
    case 'record':
    case 'run-step': {
      const kind = String(body.kind || '');
      if (!EDIT_KINDS.includes(kind)) {
        return badRequest(res, `kind must be one of ${EDIT_KINDS.join(', ')}`);
      }
      if (body.payload == null || typeof body.payload !== 'object') {
        return badRequest(res, 'payload (object) is required');
      }
      const verb = route === 'record' ? 'record-step' : 'run-step';
      const r = await deps.runCli([verb, kind, JSON.stringify(body.payload)]);
      return sendCliResult(res, r);
    }
    case 'delete': {
      const index = Number(body.index);
      if (!Number.isInteger(index) || index < 0) {
        return badRequest(res, 'index (non-negative integer) is required');
      }
      const r = await deps.runCli(['buffer', 'delete', String(index)]);
      return sendCliResult(res, r);
    }
    case 'move': {
      const from = Number(body.from);
      const to = Number(body.to);
      if (![from, to].every((n) => Number.isInteger(n) && n >= 0)) {
        return badRequest(res, 'from and to (non-negative integers) are required');
      }
      const r = await deps.runCli(['buffer', 'move', String(from), String(to)]);
      return sendCliResult(res, r);
    }
    case 'clear': {
      const r = await deps.runCli(['buffer', 'clear']);
      return sendCliResult(res, r);
    }
    case 'cancel': {
      // Discard the in-progress recording entirely: empty the buffer (via the
      // Rust CLI) and drop the recording-session scratch (scenario.env) so the
      // editor returns to the start screen. The now-empty scenario dir is left
      // on disk but stays hidden from the Scenarios list (0 steps, no runs).
      const r = await deps.runCli(['buffer', 'clear']);
      try {
        await fsp.unlink(path.join(deps.recordRoot, 'scenario.env'));
      } catch {
        /* already gone */
      }
      return sendCliResult(res, r, { cancelled: true });
    }
    case 'input': {
      if (!deps.live) return sendJson(res, 503, { error: 'live browser unavailable' });
      const ok = deps.live.input(body || {});
      return sendJson(res, ok ? 200 : 409, { ok });
    }
    case 'pick': {
      if (!deps.live || typeof deps.live.pick !== 'function') {
        return sendJson(res, 503, { error: 'live browser unavailable' });
      }
      const nx = Number(body.nx);
      const ny = Number(body.ny);
      if (!(nx >= 0 && nx <= 1 && ny >= 0 && ny <= 1)) {
        return badRequest(res, 'nx and ny (0..1) are required');
      }
      try {
        const el = await deps.live.pick(nx, ny);
        return sendJson(res, 200, { ok: true, element: el });
      } catch (e) {
        return sendJson(res, 409, { ok: false, error: String((e && e.message) || e) });
      }
    }
    case 'flush': {
      const r = await deps.runCli(['flush']);
      const sid = /flushed sid=(\S+)/.exec(r.stdout || '');
      const wrote = /wrote\s+(.+)$/m.exec(r.stdout || '');
      return sendCliResult(res, r, {
        ...(sid ? { sid: sid[1] } : {}),
        ...(wrote ? { scenarioFile: wrote[1].trim() } : {}),
      });
    }
    default:
      return notFound(res, 'unknown editor endpoint');
  }
}

// -------- router --------

function createRequestHandler(root, deps) {
  return async function handle(req, res) {
    try {
      const url = new URL(req.url, 'http://127.0.0.1');
      const p = url.pathname;

      // Editor write surface. Only mounted when a CLI runner is
      // provided (i.e. launched from the Node launcher with a resolved
      // Rust binary). Handles its own methods (GET + POST).
      const segAll = p.split('/').filter(Boolean);
      if (segAll[0] === 'api' && segAll[1] === 'edit') {
        if (!deps || !deps.runCli) {
          return sendJson(res, 503, { error: 'editor unavailable: agent-qa CLI not resolved' });
        }
        return await handleEdit(req, res, deps, segAll.slice(2));
      }

      // Trigger a replay of a recorded scenario (POST). Spawns the Rust CLI
      // `replay <sid>` detached; the viewer's live poll then auto-follows the
      // new run. Requires the launcher-resolved CLI (deps.replay).
      if (
        req.method === 'POST' &&
        segAll[0] === 'api' &&
        segAll[1] === 'scenarios' &&
        segAll[3] === 'replay' &&
        segAll.length === 4
      ) {
        if (!deps || typeof deps.replay !== 'function') {
          return sendJson(res, 503, { error: 'replay unavailable: agent-qa CLI not resolved' });
        }
        const sid = decodeURIComponent(segAll[2]);
        if (!isSafeSegment(sid)) return badRequest(res, 'unsafe sid');
        const scn = await readJson(path.join(root, sid, 'scenario.json'));
        if (!scn) return notFound(res, 'no scenario.json to replay');
        const out = await deps.replay(sid, replaySessionFor(sid));
        if (!out.ok) return sendJson(res, 500, { error: out.error || 'replay failed to start' });
        return sendJson(res, 202, { ok: true, sid, started: true });
      }

      if (req.method !== 'GET' && req.method !== 'HEAD') {
        return sendJson(res, 405, { error: 'method not allowed' });
      }

      if (Object.prototype.hasOwnProperty.call(STATIC_FILES, p)) {
        return serveStatic(res, STATIC_FILES[p]);
      }

      if (p === '/api/root') {
        return sendJson(res, 200, { scenariosRoot: root, editor: !!(deps && deps.runCli) });
      }
      if (p === '/api/scenarios') {
        return sendJson(res, 200, {
          scenariosRoot: root,
          scenarios: await listScenarios(root),
        });
      }

      // /api/scenarios/:sid/runs[/:runId[/artifact/:kind/:stepId]]
      const seg = segAll; // e.g. ['api','scenarios',sid,'runs',...]
      if (seg[0] === 'api' && seg[1] === 'scenarios' && seg.length >= 4) {
        const sid = decodeURIComponent(seg[2]);
        if (!isSafeSegment(sid)) return badRequest(res, 'unsafe sid');
        // /api/scenarios/:sid/scenario → the recorded scenario.json definition
        if (seg[3] === 'scenario' && seg.length === 4) {
          const scn = await readJson(path.join(root, sid, 'scenario.json'));
          if (!scn) return notFound(res, 'no scenario.json for this sid');
          return sendJson(res, 200, { sid, scenario: scn });
        }
        // /api/scenarios/:sid/replay-stream → live screencast (SSE) of the
        // replay's browser, so the viewer can watch the run as it happens.
        if (seg[3] === 'replay-stream' && seg.length === 4) {
          if (!deps || typeof deps.liveForSession !== 'function') {
            return sendJson(res, 503, { error: 'live replay unavailable: agent-qa CLI not resolved' });
          }
          const bridge = deps.liveForSession(replaySessionFor(sid));
          res.writeHead(200, {
            'content-type': 'text/event-stream',
            'cache-control': 'no-cache',
            connection: 'keep-alive',
            'x-accel-buffering': 'no',
          });
          res.write(': connected\n\n');
          bridge.subscribe(res);
          req.on('close', () => bridge.unsubscribe(res));
          return undefined;
        }
        if (seg[3] === 'runs') {
          if (seg.length === 4) {
            return sendJson(res, 200, { sid, replays: await listRuns(path.join(root, sid)) });
          }
          const runId = decodeURIComponent(seg[4]);
          if (!isSafeSegment(runId)) return badRequest(res, 'unsafe runId');
          if (seg.length === 5) {
            return sendJson(res, 200, await runDetail(root, sid, runId));
          }
          if (seg[5] === 'artifact' && seg.length === 8) {
            const kind = decodeURIComponent(seg[6]);
            const stepId = decodeURIComponent(seg[7]);
            return serveArtifact(res, root, sid, runId, kind, stepId);
          }
        }
      }

      return notFound(res, 'not found');
    } catch (err) {
      sendJson(res, 500, { error: String((err && err.message) || err) });
    }
  };
}

function createServer(root, deps) {
  // When a CLI runner is present (real editor) but no live bridge was
  // injected, build one that resolves the session's CDP url through the
  // same Rust CLI path the rest of the editor uses.
  if (deps && deps.runCli && !deps.live) {
    const runCli = deps.runCli;
    const sessionBridges = new Map(); // session -> read-only screencast bridge
    deps = {
      ...deps,
      live: createLiveBridge({
        getCdpUrl: cdpUrlResolver(runCli, null),
        // Auto-record commits the step ONCE here, server-side, through the same
        // Rust CLI as manual recording. The bridge then broadcasts
        // 'buffer-changed' so every open editor tab refreshes its Steps list.
        // (Previously each tab recorded the broadcast itself, which
        // double-recorded whenever more than one tab was connected.)
        onRecord: async (kind, payload) => {
          const r = await runCli(['record-step', kind, JSON.stringify(payload)]);
          if (r.spawnError) throw new Error(String(r.spawnError.message || r.spawnError));
          if (r.code !== 0) throw new Error((r.stderr || 'record-step failed').trim());
        },
        logger: (m) => console.error(`  [live] ${m}`),
      }),
      // Lazily-created, cached screencast bridges for replay sessions, so the
      // Runs viewer can watch a replay's browser live. Read-only (no input,
      // no onRecord). Each bridge tears its CDP link down on last unsubscribe.
      liveForSession: (session) => {
        let b = sessionBridges.get(session);
        if (!b) {
          b = createLiveBridge({
            getCdpUrl: cdpUrlResolver(runCli, session),
            logger: (m) => console.error(`  [replay-live ${session}] ${m}`),
          });
          sessionBridges.set(session, b);
        }
        return b;
      },
    };
  }
  const server = http.createServer(createRequestHandler(root, deps));
  server._live = deps && deps.live; // for clean shutdown
  return server;
}

// -------- browser open --------

function openBrowser(targetUrl) {
  const { spawn } = require('node:child_process');
  const plat = process.platform;
  const cmd = plat === 'darwin' ? 'open' : plat === 'win32' ? 'cmd' : 'xdg-open';
  const args = plat === 'win32' ? ['/c', 'start', '', targetUrl] : [targetUrl];
  try {
    const child = spawn(cmd, args, { stdio: 'ignore', detached: true });
    child.on('error', () => {});
    child.unref();
  } catch {
    /* best-effort; URL is printed regardless */
  }
}

// -------- entrypoint --------

function start(opts = {}) {
  const root = resolveScenariosRoot(opts);
  const recordRoot = resolveRecordRoot(opts);
  const host = '127.0.0.1';
  const port = Number.isInteger(opts.port) ? opts.port : 7878;

  // Editor write surface: only mounted when the launcher resolved a Rust
  // binary. Children inherit a child env that pins the scenarios + record
  // roots so `start`/`flush`/`buffer` write exactly where the viewer reads,
  // and forwards the resolved agent-browser binary.
  let deps;
  const bin = opts.agentQaBin || process.env.AGENT_QA_BINARY_PATH || null;
  if (bin) {
    const childEnv = {
      ...process.env,
      AGENT_QA_SCENARIOS_DIR: root,
      AGENT_QA_RECORD_DIR: recordRoot,
    };
    if (opts.agentBrowserBin) childEnv.AGENT_BROWSER_BIN = opts.agentBrowserBin;
    deps = {
      recordRoot,
      runCli: makeCliRunner({ bin, env: childEnv, cwd: opts.cwd || process.cwd() }),
      replay: makeReplaySpawner({ bin, env: childEnv, cwd: opts.cwd || process.cwd() }),
    };
  }

  const server = createServer(root, deps);

  server.on('error', (err) => {
    if (err && err.code === 'EADDRINUSE') {
      console.error(`agent-qa report view: port ${port} is already in use on ${host}.`);
      console.error('Pass a different port: agent-qa report view --port <N>');
    } else {
      console.error(`agent-qa report view: ${err.message || err}`);
    }
    process.exit(1);
  });

  server.listen(port, host, () => {
    const url = `http://${host}:${port}/`;
    console.error(`agent-qa report view — read-only viewer + editor`);
    console.error(`  scenarios root: ${root}`);
    console.error(`  serving:        ${url}`);
    if (deps) {
      console.error(`  editor:         ${url}editor`);
    } else {
      console.error('  editor:         (unavailable — agent-qa binary not resolved)');
    }
    console.error('  (localhost-only; Ctrl-C to stop)');
    if (opts.open !== false) openBrowser(url);
  });

  const shutdown = () => {
    if (server._live) {
      try {
        server._live.stop();
      } catch {
        /* ignore */
      }
    }
    server.close(() => process.exit(0));
    // Force-exit if connections linger.
    setTimeout(() => process.exit(0), 500).unref();
  };
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  return server;
}

module.exports = {
  isSafeSegment,
  resolveScenariosRoot,
  resolveRecordRoot,
  scenariosRootFromToml,
  parseEnvFile,
  listScenarios,
  listRuns,
  findActiveRunId,
  runDetail,
  scenarioSummary,
  makeCliRunner,
  lastJsonLine,
  createLiveBridge,
  createServer,
  createRequestHandler,
  start,
  ARTIFACT_KINDS,
  EDIT_KINDS,
};
