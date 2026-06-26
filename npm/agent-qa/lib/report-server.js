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
const os = require('node:os');
const crypto = require('node:crypto');
const path = require('node:path');
const { createReadStream } = require('node:fs');
const { execFile } = require('node:child_process');
const { pathToFileURL } = require('node:url');
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
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.svg': 'image/svg+xml',
  '.json': 'application/json; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.ico': 'image/x-icon',
};

// Static files served by name (fixed allowlist; no user input reaches the
// filesystem path).
const STATIC_FILES = {
  '/': 'index.html',
  '/index.html': 'index.html',
  '/editor': 'editor.html',
  '/editor.html': 'editor.html',
  '/chat': 'chat.html',
  '/chat.html': 'chat.html',
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

// The React app (`web/`) builds into lib/public/ (the canonical UI) and is
// served at /, /editor, /chat (see STATIC_FILES). Its hashed assets and fonts
// live under lib/public/assets/ and are served here with the same path-safety
// clamp as serveStatic.
async function serveAssets(res, p) {
  const parts = p.slice(1).split('/').filter(Boolean); // ['assets', '<hashed>']
  if (!parts.length || !parts.every(isSafeSegment)) return notFound(res, 'not found');
  return serveStatic(res, parts.join('/'));
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

// List live agent-browser session names by reading the per-session unix
// sockets in agent-browser's state dir (AGENT_BROWSER_HOME, default
// ~/.agent-browser). Cheap — no Chrome launch. Filters out sessions whose
// daemon pid is dead (stale socket) and any unsafe names. Sorted, with
// "default" first so it anchors the picker.
function listBrowserSessions() {
  const home = process.env.AGENT_BROWSER_HOME || path.join(os.homedir(), '.agent-browser');
  let names = [];
  try {
    names = fs
      .readdirSync(home)
      .filter((f) => f.endsWith('.sock'))
      .map((f) => f.slice(0, -'.sock'.length))
      .filter((n) => n && isSafeSegment(n));
  } catch {
    return [];
  }
  const alive = names.filter((n) => {
    let pid;
    try {
      pid = parseInt(fs.readFileSync(path.join(home, `${n}.pid`), 'utf8').trim(), 10);
    } catch {
      return true; // socket present, no pid file — assume live
    }
    if (!Number.isInteger(pid) || pid <= 0) return true;
    try {
      process.kill(pid, 0); // throws ESRCH if dead
      return true;
    } catch (e) {
      return !!(e && e.code === 'EPERM'); // alive but not ours → keep; ESRCH → drop
    }
  });
  return alive.sort((a, b) => (a === 'default' ? -1 : b === 'default' ? 1 : a.localeCompare(b)));
}

// Bind a dedicated agent-browser session to the chat conversation so the
// agent's browsing AND recording land in one browser the live pane mirrors
// ("chat + browser = session"). The Rust CLI honors AGENT_BROWSER_SESSION for
// both bare `agent-browser` and `agent-qa start`, so exposing the name on
// process.env is enough — the in-process chat agent's bash subprocesses inherit
// it. New chat rotates to a fresh name (a clean browser); the old daemon is
// closed for cleanup. Editor/replay strip this var so they keep their own
// sessions.
function makeChatBrowserBinding() {
  const mint = () => `chat-${crypto.randomBytes(4).toString('hex')}`;
  let name = mint();
  return {
    get name() {
      return name;
    },
    rotate() {
      name = mint();
      return name;
    },
  };
}

// Best-effort close of an agent-browser session daemon (cleanup on New chat so
// stale chat browsers don't accumulate). Detached + unref'd; never throws.
function closeBrowserSession(bin, session) {
  if (!session) return;
  try {
    const { spawn } = require('node:child_process');
    const child = spawn(bin || 'agent-browser', ['close', '--session', session], {
      stdio: 'ignore',
      detached: true,
    });
    child.on('error', () => {});
    child.unref();
  } catch {
    /* best-effort */
  }
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

// -------- chat — in-app agent (pi SDK) --------
//
// The Chat tab gives the browser the *same power as a terminal pi session*
// (bash/edit/write), localhost-only — the same trust boundary as the user's
// own shell, but more powerful than the read-only Runs viewer. The pi SDK is
// pure ESM and a heavy optional dep, so it is lazy-loaded (dynamic import of
// the ESM `chat-agent.mjs`) only on first chat use; if it can't be resolved
// the Chat tab degrades to "unavailable", everything else keeps working.

async function buildRealHub(config) {
  const url = pathToFileURL(path.join(__dirname, 'chat-agent.mjs')).href;
  const mod = await import(url);
  return mod.createChatBackend(config);
}

// Metadata for one chat (safe to serialize to the frontend).
function chatMeta(e) {
  return { id: e.id, title: e.title, createdAt: e.createdAt, session: e.browser.name };
}

// Read THIS chat's in-progress (or most recent) recording straight off disk:
// the per-chat record scratch (`scenario.env` + `scenario.steps.jsonl`) plus a
// `flushed` flag derived from whether the scenario dir has a `scenario.json`
// yet. No CLI spawn — pure file reads — so the chat UI can poll it cheaply
// while the agent drives the browser.
// Resolve which scenario id a chat's recording panel should show. While a
// recording is active, scenario.env carries the live SID. On finish the CLI
// truncates scenario.env but leaves scenario.last pointing at the saved SID, so
// we fall back to that — the panel keeps showing the finished recording.
async function resolveChatRecordingSid(dir) {
  let env = {};
  try {
    env = parseEnvFile(await fsp.readFile(path.join(dir, 'scenario.env'), 'utf8'));
  } catch {
    env = {};
  }
  if (env.SID) return { sid: env.SID, active: true, env };
  let last = '';
  try {
    last = (await fsp.readFile(path.join(dir, 'scenario.last'), 'utf8')).trim();
  } catch {
    last = '';
  }
  if (last) return { sid: last, active: false, env: {} };
  return { sid: null, active: false, env: {} };
}

// Map a finalized scenario.json step onto the live RecordingStep shape the UI
// renders (kind + payload + intent), so saved recordings look like live ones.
function normalizeFinalizedStep(step, i) {
  const verb = (step && step.verb) || '';
  const literal = step && step.value && step.value.literal != null ? step.value.literal : undefined;
  let kind;
  let payload;
  if (verb === 'goto' || verb === 'navigate') {
    kind = 'navigation';
    payload = { route: literal != null ? String(literal) : step.intent || '' };
  } else if (step.kind === 'check' || verb === 'assert' || verb === 'expect') {
    kind = 'assert';
    payload = { kind: 'check', intent: step.intent || '' };
  } else if (verb === 'wait' || verb === 'waitFor') {
    kind = 'wait';
    payload = { condition: { kind: verb }, intent: step.intent || '' };
  } else {
    kind = 'action';
    payload = {
      method: verb || step.kind || 'step',
      args: literal != null ? [literal] : [],
      intent: step.intent || '',
    };
  }
  return {
    stepIndex: i,
    stepId: step.id || `s${i}`,
    kind,
    payload,
    intent: step.intent || null,
    recordedAt: step.recordedAt || null,
  };
}

async function chatRecordingState(entry, scenariosRoot) {
  const out = {
    recording: false,
    sid: null,
    intent: null,
    session: null,
    startedAt: null,
    baseline: null,
    flushed: false,
    steps: [],
  };
  const dir = entry.recordDir();
  if (!dir) return out;
  const { sid, active, env } = await resolveChatRecordingSid(dir);
  if (!sid) return out;
  out.sid = sid;

  if (active) {
    out.intent = env.INTENT || null;
    out.session = env.SESSION || null;
    out.startedAt = env.STARTED || null;
    out.baseline = env.BASELINE || null;
    try {
      const txt = await fsp.readFile(path.join(dir, 'scenario.steps.jsonl'), 'utf8');
      out.steps = txt
        .split('\n')
        .map((l) => l.trim())
        .filter(Boolean)
        .map((l) => {
          try {
            return JSON.parse(l);
          } catch {
            return null;
          }
        })
        .filter(Boolean);
    } catch {
      /* no steps recorded yet */
    }
    if (scenariosRoot && isSafeSegment(sid)) {
      try {
        await fsp.access(path.join(scenariosRoot, sid, 'scenario.json'));
        out.flushed = true;
      } catch {
        /* not flushed → still being recorded */
      }
    }
    out.recording = !out.flushed;
    return out;
  }

  // Finished recording: hydrate from the saved scenario.json.
  out.flushed = true;
  out.recording = false;
  out.session = (entry.browser && entry.browser.name) || null;
  if (scenariosRoot && isSafeSegment(sid)) {
    try {
      const j = JSON.parse(
        await fsp.readFile(path.join(scenariosRoot, sid, 'scenario.json'), 'utf8')
      );
      out.intent = j.intent || null;
      out.steps = Array.isArray(j.steps) ? j.steps.map((s, i) => normalizeFinalizedStep(s, i)) : [];
    } catch {
      out.sid = null; // saved scenario vanished → nothing to show
    }
  }
  return out;
}

// Serve a per-step recording artifact (the keyframe screenshot or ARIA
// snapshot) for THIS chat's current recording, resolved via the record env's
// SID. Path-clamped to the scenario's recording dir.
async function serveRecordingArtifact(res, entry, scenariosRoot, stepId, kind) {
  const dir = entry.recordDir();
  if (!dir || !scenariosRoot) return notFound(res, 'no recording');
  if (!isSafeSegment(stepId)) return badRequest(res, 'unsafe step id');
  const { sid } = await resolveChatRecordingSid(dir);
  if (!sid || !isSafeSegment(sid)) return notFound(res, 'no recording');
  const spec =
    kind === 'snapshot'
      ? { sub: 'snapshots', ext: '.txt', type: 'text/plain; charset=utf-8' }
      : { sub: 'screenshots', ext: '.png', type: 'image/png' };
  const base = path.resolve(scenariosRoot, sid, 'recording', spec.sub);
  const full = path.resolve(base, `${stepId}${spec.ext}`);
  if (full !== base && !full.startsWith(base + path.sep)) return badRequest(res, 'path escape');
  let stat;
  try {
    stat = await fsp.stat(full);
  } catch {
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

// Owns the open chats. Each chat is an independent conversation with its own
// lazily-created hub AND its own agent-browser session (injected into the
// agent's bash via bashEnv), so chats run concurrently without sharing a
// browser. `deps.chat` may be:
//   - { hub }        : a ready hub shared by all chats (tests)
//   - { createHub }  : async factory returning a hub (tests / custom wiring)
//   - { config }     : pi SDK config → build a real per-chat hub via chat-agent.mjs
//   - undefined      : chat unavailable
function createChatManager(deps) {
  const chat = deps && deps.chat;
  const recordRoot = deps && deps.recordRoot;
  const chats = new Map();

  function makeEntry() {
    const id = crypto.randomBytes(4).toString('hex');
    const browser = makeChatBrowserBinding();
    let hubPromise = null;
    let resolvedHub = null;
    const entry = {
      id,
      browser,
      createdAt: Date.now(),
      title: 'New chat',
      // Per-chat record scratch dir so concurrent recordings don't collide and
      // the chat's pane can detect its own active recording.
      recordDir: () => (recordRoot ? path.join(recordRoot, browser.name) : null),
      getHub() {
        if (!chat) return Promise.resolve(null);
        if (chat.hub) {
          resolvedHub = chat.hub;
          return Promise.resolve(chat.hub);
        }
        if (resolvedHub) return Promise.resolve(resolvedHub);
        if (!hubPromise) {
          let factory;
          if (typeof chat.createHub === 'function') {
            factory = Promise.resolve().then(() => chat.createHub());
          } else if (chat.config) {
            // Inject this chat's browser session + record dir into every bash
            // spawn so the agent's browsing AND recording land in this chat's
            // own browser. Read lazily so New-chat rotation is picked up.
            const cfg = {
              ...chat.config,
              bashEnv: () => {
                const env = { AGENT_BROWSER_SESSION: browser.name };
                const dir = entry.recordDir();
                if (dir) env.AGENT_QA_RECORD_DIR = dir;
                return env;
              },
            };
            factory = buildRealHub(cfg);
          } else {
            return Promise.resolve(null);
          }
          hubPromise = factory
            .then((hub) => {
              resolvedHub = hub;
              return hub;
            })
            .catch((err) => {
              hubPromise = null;
              throw err;
            });
        }
        return hubPromise;
      },
      dispose() {
        if (resolvedHub && typeof resolvedHub.dispose === 'function') {
          try {
            resolvedHub.dispose();
          } catch {
            /* ignore */
          }
        }
        resolvedHub = null;
        hubPromise = null;
      },
    };
    return entry;
  }

  function create() {
    const entry = makeEntry();
    chats.set(entry.id, entry);
    // Pre-warm the hub so the first /state fetch doesn't pay the full pi SDK
    // init latency — makes "New chat" feel responsive. Best-effort; the real
    // /state await reuses this same in-flight promise (no double build).
    Promise.resolve()
      .then(() => entry.getHub())
      .catch(() => {});
    return entry;
  }

  // The primary chat backs the legacy flat routes; created on demand so there
  // is always one chat for single-chat clients and tests.
  function primary() {
    const first = chats.values().next().value;
    return first || create();
  }

  function dispose() {
    // Close each chat's agent-browser daemon too (mirrors remove()). Without
    // this, quitting the server (SIGINT/SIGTERM) disposed the agents but left
    // every chat-<id> browser daemon running — they survived and piled up in
    // the session picker on the next run.
    for (const e of chats.values()) {
      e.dispose();
      closeBrowserSession(deps && deps.agentBrowserBin, e.browser.name);
    }
    chats.clear();
  }

  return {
    create,
    primary,
    get: (id) => chats.get(id) || null,
    list: () => Array.from(chats.values()).map(chatMeta),
    remove(id) {
      const e = chats.get(id);
      if (!e) return false;
      e.dispose();
      closeBrowserSession(deps && deps.agentBrowserBin, e.browser.name);
      chats.delete(id);
      return true;
    },
    dispose,
    available: () => !!chat,
  };
}

async function handleChat(req, res, manager, deps, seg, scenariosRoot) {
  // seg: path segments after ['api','chat']
  const route = seg.join('/');

  // ----- global routes (not tied to a specific chat) -----

  // Enumerate the agent-browser sessions the user can switch the live pane to.
  // We read the per-session unix sockets in agent-browser's state dir (cheap;
  // no Chrome launch like `doctor`), then keep only sessions whose daemon pid
  // is still alive so stale sockets don't linger in the picker.
  if (route === 'sessions' && req.method === 'GET') {
    return sendJson(res, 200, { sessions: listBrowserSessions() });
  }

  // List the open chats / create a new one. Each chat owns its own browser
  // session + conversation; the frontend addresses a specific chat via the
  // /api/chat/c/<id>/* routes.
  if (route === 'list' && req.method === 'GET') {
    return sendJson(res, 200, { chats: manager.list() });
  }
  if (route === 'create' && req.method === 'POST') {
    return sendJson(res, 200, chatMeta(manager.create()));
  }

  // Live browser pane: a read-only CDP screencast of the agent-browser session
  // the chat agent drives (default session "default"; override with ?session).
  // Reuses the same per-session screencast bridge as the replay viewer, so the
  // user can watch the agent operate a browser right next to the conversation.
  // Requires the launcher-resolved Rust CLI (deps.liveForSession).
  if (route === 'browser-stream' && req.method === 'GET') {
    if (!deps || typeof deps.liveForSession !== 'function') {
      return sendJson(res, 503, { error: 'live browser unavailable: agent-qa CLI not resolved' });
    }
    const url = new URL(req.url, 'http://127.0.0.1');
    const session = url.searchParams.get('session') || 'default';
    if (!isSafeSegment(session)) return badRequest(res, 'unsafe session');
    const bridge = deps.liveForSession(session);
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

  // Drive the agent's browser from the pane's URL bar: navigate / back /
  // forward / reload, dispatched over the same read-only screencast bridge's
  // CDP link. The bridge must already be connected (the pane is streaming).
  if (route === 'browser-navigate' && req.method === 'POST') {
    if (!deps || typeof deps.liveForSession !== 'function') {
      return sendJson(res, 503, { error: 'live browser unavailable: agent-qa CLI not resolved' });
    }
    const body = await readJsonBody(req);
    const session = typeof body.session === 'string' && body.session ? body.session : 'default';
    if (!isSafeSegment(session)) return badRequest(res, 'unsafe session');
    const action = body.action || 'navigate';
    let evt;
    if (action === 'navigate') {
      let url = String(body.url || '').trim();
      if (!url) return badRequest(res, 'url is required');
      if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(url) && !url.startsWith('about:')) {
        url = 'https://' + url; // bare host → https
      }
      evt = { type: 'navigate', url };
    } else if (action === 'reload') {
      evt = { type: 'reload' };
    } else if (action === 'back') {
      evt = { type: 'back' };
    } else if (action === 'forward') {
      evt = { type: 'forward' };
    } else {
      return badRequest(res, 'unknown action');
    }
    const ok = deps.liveForSession(session).input(evt);
    if (!ok) {
      return sendJson(res, 409, {
        error: 'no live browser session — ask the agent to open a page first',
      });
    }
    return sendJson(res, 200, { ok: true });
  }

  // ----- resolve the target chat -----
  // Legacy flat routes (/api/chat/state …) operate on the primary chat (one
  // server, one chat — single-chat clients + tests). Explicit
  // /api/chat/c/<id>/<sub> routes address a specific chat.
  let entry;
  let sub;
  if (seg[0] === 'c') {
    if (!seg[1] || !isSafeSegment(seg[1])) return badRequest(res, 'bad chat id');
    entry = manager.get(seg[1]);
    if (!entry) return notFound(res, 'unknown chat');
    sub = seg.slice(2).join('/');
  } else {
    entry = manager.primary();
    sub = route;
  }

  // Remove a chat: dispose its hub + close its browser daemon.
  if (sub === 'delete' && (req.method === 'POST' || req.method === 'DELETE')) {
    manager.remove(entry.id);
    return sendJson(res, 200, { ok: true });
  }

  // The agent-browser session to mirror in THIS chat's live pane. Priority: an
  // active recorder session for this chat (skill rule: "one session for the
  // whole recording"), else the chat's bound browser session. `recording` lets
  // the pane badge the difference. Cheap file read — no CLI spawn.
  if (sub === 'active-session' && req.method === 'GET') {
    let recorder = null;
    try {
      const dir = entry.recordDir();
      if (dir) {
        const env = parseEnvFile(await fsp.readFile(path.join(dir, 'scenario.env'), 'utf8'));
        if (env.SESSION) recorder = env.SESSION;
      }
    } catch {
      /* no active recording */
    }
    const mine = entry.browser.name;
    return sendJson(res, 200, {
      session: recorder || mine,
      recording: !!recorder && recorder === mine,
    });
  }

  // Live view of THIS chat's recording: the steps recorded so far + their
  // per-step screenshot/snapshot artifacts. Cheap file reads — pollable.
  if (sub === 'recording' && req.method === 'GET') {
    return sendJson(res, 200, await chatRecordingState(entry, scenariosRoot));
  }
  {
    const m = /^recording\/step\/([^/]+)\/(screenshot|snapshot)$/.exec(sub);
    if (m && req.method === 'GET') {
      return serveRecordingArtifact(res, entry, scenariosRoot, decodeURIComponent(m[1]), m[2]);
    }
  }

  let hub = null;
  try {
    hub = await entry.getHub();
  } catch (err) {
    const reason = String((err && err.message) || err);
    if (sub === 'state' && req.method === 'GET') {
      return sendJson(res, 200, { available: false, reason });
    }
    return sendJson(res, 503, { error: `chat unavailable: ${reason}` });
  }

  if (!hub) {
    if (sub === 'state' && req.method === 'GET') {
      return sendJson(res, 200, { available: false, reason: 'pi SDK not configured' });
    }
    return sendJson(res, 503, { error: 'chat unavailable: pi SDK not configured' });
  }

  if (sub === 'state' && req.method === 'GET') {
    return sendJson(res, 200, { available: true, ...(await hub.getState()) });
  }

  // Live agent event stream (SSE). Survives reconnects; the client first GETs
  // /state to rehydrate history, then opens this for live deltas.
  if (sub === 'stream' && req.method === 'GET') {
    res.writeHead(200, {
      'content-type': 'text/event-stream',
      'cache-control': 'no-cache',
      connection: 'keep-alive',
      'x-accel-buffering': 'no',
    });
    hub.subscribe(res);
    req.on('close', () => hub.unsubscribe(res));
    return undefined;
  }

  if (req.method !== 'POST') {
    return sendJson(res, 405, { error: 'method not allowed' });
  }
  const body = await readJsonBody(req);

  switch (sub) {
    case 'prompt': {
      const text = typeof body.text === 'string' ? body.text : '';
      if (!text.trim()) return badRequest(res, 'text is required');
      const opts = {};
      if (body.streamingBehavior === 'steer' || body.streamingBehavior === 'followUp') {
        opts.streamingBehavior = body.streamingBehavior;
      }
      // Fire-and-forget: the answer streams over SSE. Errors are broadcast to
      // subscribers by the hub, so swallow the rejection here.
      Promise.resolve(hub.prompt(text, opts)).catch(() => {});
      return sendJson(res, 202, { ok: true });
    }
    case 'abort': {
      await hub.abort();
      return sendJson(res, 200, { ok: true });
    }
    case 'new': {
      await hub.newSession();
      // Rotate this chat's bound browser to a fresh session and close the old
      // daemon (clean browser for the reset conversation).
      const old = entry.browser.name;
      entry.browser.rotate();
      closeBrowserSession(deps && deps.agentBrowserBin, old);
      return sendJson(res, 200, { ok: true });
    }
    case 'model': {
      try {
        const state = await hub.setModel({ provider: body.provider, id: body.id || body.model });
        return sendJson(res, 200, { ok: true, ...state });
      } catch (err) {
        return sendJson(res, 400, { error: String((err && err.message) || err) });
      }
    }
    case 'thinking': {
      try {
        const state = await hub.setThinkingLevel(String(body.level || ''));
        return sendJson(res, 200, { ok: true, ...state });
      } catch (err) {
        return sendJson(res, 400, { error: String((err && err.message) || err) });
      }
    }
    default:
      return notFound(res, 'unknown chat endpoint');
  }
}

// -------- router --------

function createRequestHandler(root, deps, chat) {
  const chatManager = chat || createChatManager(deps);
  return async function handle(req, res) {
    try {
      const url = new URL(req.url, 'http://127.0.0.1');
      const p = url.pathname;

      // Editor write surface. Only mounted when a CLI runner is
      // provided (i.e. launched from the Node launcher with a resolved
      // Rust binary). Handles its own methods (GET + POST).
      const segAll = p.split('/').filter(Boolean);

      // In-app chat surface (pi SDK). Handles its own methods (GET + POST).
      if (segAll[0] === 'api' && segAll[1] === 'chat') {
        return await handleChat(req, res, chatManager, deps, segAll.slice(2), root);
      }

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

      // React app hashed assets + fonts (built from web/ → lib/public/assets).
      if (p.startsWith('/assets/')) {
        return serveAssets(res, p);
      }

      if (p === '/api/root') {
        return sendJson(res, 200, {
          scenariosRoot: root,
          editor: !!(deps && deps.runCli),
          chat: chatManager.available(),
          liveBrowser: !!(deps && typeof deps.liveForSession === 'function'),
        });
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
  const chatManager = createChatManager(deps);
  const server = http.createServer(createRequestHandler(root, deps, chatManager));
  server._live = deps && deps.live; // for clean shutdown
  server._chat = chatManager; // for clean shutdown
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
  // The in-process chat agent inherits process.env (its bash subprocesses do
  // too). Pin the same scenarios + record roots the viewer/editor use so
  // chat-recorded scenarios show up in the Runs tab and the live pane can
  // follow the chat's recorder session. Editor/replay set these explicitly in
  // their own childEnv, so this only steers the chat agent.
  process.env.AGENT_QA_SCENARIOS_DIR = root;
  process.env.AGENT_QA_RECORD_DIR = recordRoot;
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
    // Editor/replay drive their own sessions (default / replay-<sid>); never
    // let an ambient or chat-bound AGENT_BROWSER_SESSION steer their CLI calls.
    delete childEnv.AGENT_BROWSER_SESSION;
    deps = {
      recordRoot,
      agentBrowserBin: opts.agentBrowserBin || process.env.AGENT_BROWSER_BIN || null,
      runCli: makeCliRunner({ bin, env: childEnv, cwd: opts.cwd || process.cwd() }),
      replay: makeReplaySpawner({ bin, env: childEnv, cwd: opts.cwd || process.cwd() }),
    };
  }

  // In-app chat: independent of the Rust binary. The pi SDK is lazy-loaded
  // on first chat use; if it can't be resolved the Chat tab reports
  // "unavailable" and the rest of the app keeps working. Opt out with
  // AGENT_QA_NO_CHAT=1.
  if (opts.chat !== false && process.env.AGENT_QA_NO_CHAT !== '1') {
    deps = deps || {};
    // Each chat owns its own agent-browser session, injected into that chat's
    // bash spawns via bashEnv by the chat manager (per-chat, so chats run
    // concurrently without sharing a browser). The live pane mirrors it.
    deps.chat = {
      config: {
        cwd: opts.cwd || process.cwd(),
        sdkPath: opts.piSdkPath || process.env.AGENT_QA_PI_SDK || undefined,
        agentDir: opts.agentDir || undefined,
        tools: Array.isArray(opts.chatTools) ? opts.chatTools : undefined,
        logger: (m) => console.error(`  [chat] ${m}`),
      },
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
    if (deps && deps.chat) {
      console.error(`  chat:           ${url}chat`);
    }
    console.error('  (localhost-only; Ctrl-C to stop)');
    if (opts.open !== false) openBrowser(url);
  });

  const shutdown = () => {
    if (server._chat) {
      try {
        server._chat.dispose();
      } catch {
        /* ignore */
      }
    }
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
  createChatManager,
  createServer,
  createRequestHandler,
  start,
  ARTIFACT_KINDS,
  EDIT_KINDS,
};
