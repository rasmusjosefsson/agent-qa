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

// Direct scenario/2 draft kinds accepted by the Rust recorder.
const EDIT_KINDS = ['do', 'check'];

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
  '/cases': 'cases.html',
  '/cases.html': 'cases.html',
  '/sets': 'sets.html',
  '/sets.html': 'sets.html',
  '/plans': 'plans.html',
  '/plans.html': 'plans.html',
  '/personas': 'personas.html',
  '/personas.html': 'personas.html',
  '/environments': 'environments.html',
  '/environments.html': 'environments.html',
  '/knowledge': 'knowledge.html',
  '/knowledge.html': 'knowledge.html',
  '/plugins': 'plugins.html',
  '/plugins.html': 'plugins.html',
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

// Mirror of cli/src/paths.rs::record_root. The directory holds one typed
// recorder-state.json file during an authoring session.
function resolveRecordRoot(opts = {}) {
  const cwd = opts.cwd || process.cwd();
  const env = opts.env || process.env;
  if (env.AGENT_QA_RECORD_DIR) return resolveRelative(env.AGENT_QA_RECORD_DIR, cwd);
  return path.join(cwd, 'tmp', 'agent-qa-record');
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

// -------- test cases (author-side artifact) --------
//
// A "test case" is a human-authored, plain-English test (numbered steps +
// expected result + [TOKEN] test data) — the thing a QA engineer writes before
// it is a machine-replayable scenario. The chat agent turns a case into a
// recorded `scenario.json`; the case stores the resulting sid in `scenarioSid`
// (1→1). Cases are pure metadata (no browser), so this is a plain Node JSON
// surface — no Rust CLI required, unlike the editor's record/replay writes.
//
// Stored under `<root>/_cases/<id>/case.json`. The `_cases` dir passes
// isSafeSegment but never surfaces in Runs: listScenarios drops any dir with 0
// steps and no runs, and `_cases` has no scenario.json of its own.
const CASES_DIRNAME = '_cases';
const casesDir = (root) => path.join(root, CASES_DIRNAME);
const caseFile = (root, id) => path.join(casesDir(root), id, 'case.json');

// Normalize an upsert body into a sealed case/1 record, preserving fields the
// caller omits (so a metadata save can't clobber a sid the agent just linked —
// `??` treats an explicit null as "keep existing" too).
function normalizeCase(id, body, existing) {
  const now = Date.now();
  const arr = (v, fb) => (Array.isArray(v) ? v : fb);
  return {
    schema: 'case/1',
    id,
    title: String(body.title ?? existing?.title ?? id),
    startUrl: String(body.startUrl ?? existing?.startUrl ?? ''),
    preconditions: String(body.preconditions ?? existing?.preconditions ?? ''),
    steps: arr(body.steps, existing?.steps ?? []).map((s) => String(s)),
    expected: String(body.expected ?? existing?.expected ?? ''),
    inputs:
      body.inputs && typeof body.inputs === 'object' ? body.inputs : existing?.inputs ?? {},
    tags: arr(body.tags, existing?.tags ?? []).map((t) => String(t)),
    scenarioSid: body.scenarioSid ?? existing?.scenarioSid ?? null,
    source: String(body.source ?? existing?.source ?? 'manual'),
    sourceRef: body.sourceRef ?? existing?.sourceRef ?? null,
    // Links to external test-management items (provider-agnostic), so an import
    // adapter can round-trip a case to its origin without the core knowing any
    // one provider's schema. `[{ provider, key, url }]`.
    externalRefs: arr(body.externalRefs, existing?.externalRefs ?? []).map((r) => ({
      provider: String((r && r.provider) ?? ''),
      key: String((r && r.key) ?? ''),
      url: r && r.url != null ? String(r.url) : null,
    })),
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
}

// Join a case to its linked scenario summary (last-run status/time) when set.
async function caseWithScenario(root, rec) {
  let scenario = null;
  if (rec.scenarioSid && isSafeSegment(rec.scenarioSid)) {
    scenario = await scenarioSummary(root, rec.scenarioSid);
  }
  return { ...rec, scenario };
}

async function listCases(root) {
  let entries;
  try {
    entries = await fsp.readdir(casesDir(root), { withFileTypes: true });
  } catch {
    return [];
  }
  const ids = entries
    .filter((e) => e.isDirectory() && isSafeSegment(e.name))
    .map((e) => e.name)
    .sort();
  const out = [];
  for (const id of ids) {
    const rec = await readJson(caseFile(root, id));
    if (rec) out.push(await caseWithScenario(root, rec));
  }
  return out;
}

// /api/cases[/:id[/delete|link]] — GET list/one, POST upsert/delete/link.
async function handleCases(req, res, root, seg) {
  const method = req.method;

  if (seg.length === 0) {
    if (method === 'GET') return sendJson(res, 200, { cases: await listCases(root) });
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  const id = decodeURIComponent(seg[0]);
  if (!isSafeSegment(id)) return badRequest(res, 'unsafe case id');
  const dir = path.join(casesDir(root), id);
  const file = caseFile(root, id);

  if (seg.length === 1) {
    if (method === 'GET') {
      const rec = await readJson(file);
      if (!rec) return notFound(res, 'no such case');
      return sendJson(res, 200, { case: await caseWithScenario(root, rec) });
    }
    if (method === 'POST') {
      let body;
      try {
        body = await readJsonBody(req);
      } catch (e) {
        return badRequest(res, String((e && e.message) || e));
      }
      const existing = await readJson(file);
      const rec = normalizeCase(id, body, existing);
      await fsp.mkdir(dir, { recursive: true });
      await fsp.writeFile(file, JSON.stringify(rec, null, 2) + '\n');
      return sendJson(res, 200, { ok: true, case: rec });
    }
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  if (seg.length === 2 && method === 'POST') {
    if (seg[1] === 'delete') {
      try {
        await fsp.rm(dir, { recursive: true, force: true });
      } catch (e) {
        return sendJson(res, 500, { error: 'delete failed: ' + ((e && e.message) || e) });
      }
      return sendJson(res, 200, { ok: true, id, deleted: true });
    }
    if (seg[1] === 'link') {
      // Read-merge-write only `scenarioSid` so a concurrent metadata save
      // can't be lost; this is the path the agent uses post-record.
      let body;
      try {
        body = await readJsonBody(req);
      } catch (e) {
        return badRequest(res, String((e && e.message) || e));
      }
      const existing = await readJson(file);
      if (!existing) return notFound(res, 'no such case');
      existing.scenarioSid = body.scenarioSid ? String(body.scenarioSid) : null;
      existing.updatedAt = Date.now();
      await fsp.writeFile(file, JSON.stringify(existing, null, 2) + '\n');
      return sendJson(res, 200, { ok: true, case: await caseWithScenario(root, existing) });
    }
  }

  return notFound(res, 'not found');
}

// -------- test sets (curated collections of cases) --------
//
// A "test set" is a reusable, cross-cutting grouping of cases — the "organize"
// primitive (a case can belong to many sets). Membership resolves one of two
// ways: an explicit `caseIds` list ('manual'), or a `tagQuery` matched against
// case tags, any-of ('tag'). Pure JSON metadata, mirroring _cases; no browser.
//
// Stored under `<root>/_sets/<id>/set.json`. Like `_cases`, the `_sets` dir
// never surfaces in Runs (no scenario.json, 0 steps).
const SETS_DIRNAME = '_sets';
const setsDir = (root) => path.join(root, SETS_DIRNAME);
const setFile = (root, id) => path.join(setsDir(root), id, 'set.json');

function normalizeSet(id, body, existing) {
  const now = Date.now();
  const arr = (v, fb) => (Array.isArray(v) ? v : fb);
  const mode =
    body.mode === 'tag' || body.mode === 'manual' ? body.mode : existing?.mode ?? 'manual';
  return {
    schema: 'set/1',
    id,
    name: String(body.name ?? existing?.name ?? id),
    description: String(body.description ?? existing?.description ?? ''),
    mode,
    caseIds: arr(body.caseIds, existing?.caseIds ?? []).map((s) => String(s)),
    tagQuery: arr(body.tagQuery, existing?.tagQuery ?? []).map((t) => String(t)),
    source: String(body.source ?? existing?.source ?? 'manual'),
    sourceRef: body.sourceRef ?? existing?.sourceRef ?? null,
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
}

// Resolve a set's members against the full case list. Manual sets keep their
// stored order (filtered to ids that still exist); tag sets match any case
// carrying ≥1 of the set's tags, in case-list order.
function resolveSetCaseIds(set, allCases) {
  if (set.mode === 'tag') {
    const want = new Set(set.tagQuery);
    if (want.size === 0) return [];
    return allCases.filter((c) => (c.tags || []).some((t) => want.has(t))).map((c) => c.id);
  }
  const byId = new Set(allCases.map((c) => c.id));
  return set.caseIds.filter((cid) => byId.has(cid));
}

async function listSets(root) {
  let entries;
  try {
    entries = await fsp.readdir(setsDir(root), { withFileTypes: true });
  } catch {
    return [];
  }
  const ids = entries
    .filter((e) => e.isDirectory() && isSafeSegment(e.name))
    .map((e) => e.name)
    .sort();
  const allCases = await listCases(root);
  const out = [];
  for (const id of ids) {
    const rec = await readJson(setFile(root, id));
    if (rec) out.push({ ...rec, caseCount: resolveSetCaseIds(rec, allCases).length });
  }
  return out;
}

// /api/sets[/:id[/delete|cases]] — GET list/one/resolved-cases, POST upsert/delete.
async function handleSets(req, res, root, seg) {
  const method = req.method;

  if (seg.length === 0) {
    if (method === 'GET') return sendJson(res, 200, { sets: await listSets(root) });
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  const id = decodeURIComponent(seg[0]);
  if (!isSafeSegment(id)) return badRequest(res, 'unsafe set id');
  const dir = path.join(setsDir(root), id);
  const file = setFile(root, id);

  if (seg.length === 1) {
    if (method === 'GET') {
      const rec = await readJson(file);
      if (!rec) return notFound(res, 'no such set');
      const allCases = await listCases(root);
      return sendJson(res, 200, {
        set: { ...rec, caseCount: resolveSetCaseIds(rec, allCases).length },
      });
    }
    if (method === 'POST') {
      let body;
      try {
        body = await readJsonBody(req);
      } catch (e) {
        return badRequest(res, String((e && e.message) || e));
      }
      const existing = await readJson(file);
      const rec = normalizeSet(id, body, existing);
      await fsp.mkdir(dir, { recursive: true });
      await fsp.writeFile(file, JSON.stringify(rec, null, 2) + '\n');
      return sendJson(res, 200, { ok: true, set: rec });
    }
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  if (seg.length === 2 && seg[1] === 'cases' && method === 'GET') {
    const rec = await readJson(file);
    if (!rec) return notFound(res, 'no such set');
    const allCases = await listCases(root);
    const ids = new Set(resolveSetCaseIds(rec, allCases));
    return sendJson(res, 200, { cases: allCases.filter((c) => ids.has(c.id)) });
  }

  if (seg.length === 2 && seg[1] === 'delete' && method === 'POST') {
    try {
      await fsp.rm(dir, { recursive: true, force: true });
    } catch (e) {
      return sendJson(res, 500, { error: 'delete failed: ' + ((e && e.message) || e) });
    }
    return sendJson(res, 200, { ok: true, id, deleted: true });
  }

  return notFound(res, 'not found');
}

// -------- test plans (runnable + trackable scope of cases) --------
//
// A "test plan" is the "execute + track" primitive: a named scope made of sets
// and/or individual cases. Running a plan replays every member case's linked
// scenario; the dashboard rolls up each case's latest run. Membership is the
// deduped union of the cases resolved from `scope.setIds` plus `scope.caseIds`.
//
// Stored under `<root>/_plans/<id>/plan.json`. Hidden from Runs like _cases.
const PLANS_DIRNAME = '_plans';
const plansDir = (root) => path.join(root, PLANS_DIRNAME);
const planFile = (root, id) => path.join(plansDir(root), id, 'plan.json');

function normalizePlan(id, body, existing) {
  const now = Date.now();
  const arr = (v, fb) => (Array.isArray(v) ? v : fb);
  const scope = body.scope && typeof body.scope === 'object' ? body.scope : {};
  const existingScope = existing?.scope ?? {};
  return {
    schema: 'plan/1',
    id,
    name: String(body.name ?? existing?.name ?? id),
    description: String(body.description ?? existing?.description ?? ''),
    scope: {
      setIds: arr(scope.setIds, existingScope.setIds ?? []).map((s) => String(s)),
      caseIds: arr(scope.caseIds, existingScope.caseIds ?? []).map((s) => String(s)),
    },
    source: String(body.source ?? existing?.source ?? 'manual'),
    sourceRef: body.sourceRef ?? existing?.sourceRef ?? null,
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
}

// Deduped union of cases from the plan's sets (resolved live) then its explicit
// caseIds, in that order. Ids that no longer exist are dropped.
function resolvePlanCaseIds(plan, allSets, allCases) {
  const order = [];
  const seen = new Set();
  const add = (cid) => {
    if (!seen.has(cid)) {
      seen.add(cid);
      order.push(cid);
    }
  };
  const setById = new Map(allSets.map((s) => [s.id, s]));
  for (const sid of plan.scope.setIds) {
    const s = setById.get(sid);
    if (s) for (const cid of resolveSetCaseIds(s, allCases)) add(cid);
  }
  const byId = new Set(allCases.map((c) => c.id));
  for (const cid of plan.scope.caseIds) if (byId.has(cid)) add(cid);
  return order;
}

async function listPlans(root) {
  let entries;
  try {
    entries = await fsp.readdir(plansDir(root), { withFileTypes: true });
  } catch {
    return [];
  }
  const ids = entries
    .filter((e) => e.isDirectory() && isSafeSegment(e.name))
    .map((e) => e.name)
    .sort();
  const [allCases, allSets] = [await listCases(root), await listSets(root)];
  const out = [];
  for (const id of ids) {
    const rec = await readJson(planFile(root, id));
    if (rec) out.push({ ...rec, caseCount: resolvePlanCaseIds(rec, allSets, allCases).length });
  }
  return out;
}

// /api/plans[/:id[/delete|cases|run]] — GET list/one/resolved-cases, POST
// upsert/delete/run. The `run` action replays each member scenario and needs
// the launcher-resolved CLI (deps.replay); everything else is pure JSON.
async function handlePlans(req, res, root, seg, deps) {
  const method = req.method;

  if (seg.length === 0) {
    if (method === 'GET') return sendJson(res, 200, { plans: await listPlans(root) });
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  const id = decodeURIComponent(seg[0]);
  if (!isSafeSegment(id)) return badRequest(res, 'unsafe plan id');
  const dir = path.join(plansDir(root), id);
  const file = planFile(root, id);

  if (seg.length === 1) {
    if (method === 'GET') {
      const rec = await readJson(file);
      if (!rec) return notFound(res, 'no such plan');
      const [allCases, allSets] = [await listCases(root), await listSets(root)];
      return sendJson(res, 200, {
        plan: { ...rec, caseCount: resolvePlanCaseIds(rec, allSets, allCases).length },
      });
    }
    if (method === 'POST') {
      let body;
      try {
        body = await readJsonBody(req);
      } catch (e) {
        return badRequest(res, String((e && e.message) || e));
      }
      const existing = await readJson(file);
      const rec = normalizePlan(id, body, existing);
      await fsp.mkdir(dir, { recursive: true });
      await fsp.writeFile(file, JSON.stringify(rec, null, 2) + '\n');
      return sendJson(res, 200, { ok: true, plan: rec });
    }
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  if (seg.length === 2 && seg[1] === 'cases' && method === 'GET') {
    const rec = await readJson(file);
    if (!rec) return notFound(res, 'no such plan');
    const [allCases, allSets] = [await listCases(root), await listSets(root)];
    const ids = new Set(resolvePlanCaseIds(rec, allSets, allCases));
    return sendJson(res, 200, { cases: allCases.filter((c) => ids.has(c.id)) });
  }

  if (seg.length === 2 && seg[1] === 'run' && method === 'POST') {
    if (!deps || typeof deps.replay !== 'function') {
      return sendJson(res, 503, { error: 'run unavailable: agent-qa CLI not resolved' });
    }
    const rec = await readJson(file);
    if (!rec) return notFound(res, 'no such plan');
    // Optional persona/environment for this run: { profile, params }.
    let runOpts = {};
    try {
      runOpts = runOptsFromBody(await readJsonBody(req));
    } catch {
      /* no/!json body → defaults */
    }
    // Resolve the run's persona credentials once for the whole batch (an
    // unresolved vault ref fails the run up front rather than per case).
    const auth = await resolveRunAuthEnv(root, deps, runOpts);
    if (auth.error) return sendJson(res, 200, { ok: false, error: auth.error, started: [], skipped: [] });
    runOpts.env = auth.env;
    if (auth.profile) runOpts.profile = auth.profile;
    const [allCases, allSets] = [await listCases(root), await listSets(root)];
    const memberIds = resolvePlanCaseIds(rec, allSets, allCases);
    const byId = new Map(allCases.map((c) => [c.id, c]));
    const started = [];
    const skipped = [];
    for (const cid of memberIds) {
      const c = byId.get(cid);
      const sid = c && c.scenarioSid;
      if (!sid || !isSafeSegment(sid)) {
        skipped.push({ caseId: cid, reason: 'no recorded scenario' });
        continue;
      }
      const scn = await readJson(path.join(root, sid, 'scenario.json'));
      if (!scn) {
        skipped.push({ caseId: cid, reason: 'scenario missing' });
        continue;
      }
      const session = sessionForReplay(sid, runOpts.profile);
      let out;
      try {
        out = await launchReplay(deps, sid, session, runOpts);
      } catch (e) {
        skipped.push({ caseId: cid, reason: String((e && e.message) || e) });
        continue;
      }
      if (out.ok) started.push({ caseId: cid, sid });
      else skipped.push({ caseId: cid, reason: out.error || 'replay failed to start' });
    }
    return sendJson(res, 202, { ok: true, started, skipped });
  }

  if (seg.length === 2 && seg[1] === 'delete' && method === 'POST') {
    try {
      await fsp.rm(dir, { recursive: true, force: true });
    } catch (e) {
      return sendJson(res, 500, { error: 'delete failed: ' + ((e && e.message) || e) });
    }
    return sendJson(res, 200, { ok: true, id, deleted: true });
  }

  return notFound(res, 'not found');
}

// -------- run options (persona + environment) --------
//
// A replay/plan-run can carry a persona (forwarded as `--profile`) and
// environment values (each `--param k=v`). The UI resolves the chosen persona
// + environment into `{ profile, params }` and posts that; the server just
// passes it through to the replay spawner.
function runOptsFromBody(body) {
  // Headless is explicit rather than implicit: every workbench replay sends a
  // mode flag to the CLI, so an inherited AGENT_BROWSER_HEADED value cannot
  // make the default visible. Only a JSON boolean true opts into headed mode.
  const o = { headed: !!(body && body.headed === true) };
  if (body && body.profile) o.profile = String(body.profile);
  // A persona/environment named by id lets the server resolve the persona's
  // credentials at run time (see resolveRunAuthEnv) — the profile string alone
  // can't, since creds live only under the persona record.
  if (body && body.personaId) o.personaId = String(body.personaId);
  if (body && body.environmentId) o.environmentId = String(body.environmentId);
  if (body && body.params && typeof body.params === 'object') {
    o.params = {};
    for (const [k, v] of Object.entries(body.params)) if (k) o.params[String(k)] = String(v);
  }
  return o;
}

// Prepare authentication for a persona-scoped replay/plan-run. Mirrors the
// chat replay + connect flow so an auth-walled scenario replays from the Plans
// and Runs tabs without a prior Connect click:
//   - resolve the persona's credentials (env-var → literal | `vault:` ref),
//   - surface the environment's connection config as AGENT_QA_ENV_* vars,
//   - self-bootstrap by registering the profile against the environment's auth
//     plugin (idempotent `profile-add --adapter`), so replay's own `useProfile`
//     op finds the adapter binding and runs `profile-bootstrap` — with these
//     credentials in env — against the fresh replay session (which triggers a
//     full `auth login`; see cli/src/env_ops.rs).
// Returns { env, profile } to hand to the replay spawner, or { error } when a
// vault ref can't be resolved. With no persona it returns just the plugin env
// (the pre-existing no-auth path), so unauthenticated runs are unchanged.
async function resolveRunAuthEnv(root, deps, opts) {
  const base = pluginsEnv(await readPluginPaths(root));
  const personaId = opts && opts.personaId ? String(opts.personaId) : '';
  if (!personaId) return { env: base };
  if (!isSafeSegment(personaId)) return { error: 'unsafe persona id' };

  const persona = await loadPersonaById(root, personaId);
  if (!persona) return { error: `no such persona: ${personaId}` };
  const profile = String(persona.profile || '').trim();
  if (!profile) return { error: 'persona has no profile to authenticate' };

  // The environment (optional) names the auth plugin + connection config. When
  // the caller didn't name one, fall back to the default/sole environment so the
  // plugin still gets the env's shared config (e.g. an OAuth client id) — else
  // bootstrap fails with "auth-failed: <cred> unset".
  let env = null;
  const envId = opts && opts.environmentId ? String(opts.environmentId) : '';
  if (envId && isSafeSegment(envId)) {
    env = await loadEnvironmentById(root, envId);
  }
  if (!env) env = await pickDefaultEnvironment(root);
  const auth = (env && env.auth) || {};

  // Environment's shared creds (base) + persona's identity creds (override).
  const entries = {
    ...(auth.creds || {}),
    ...((persona.credentials && persona.credentials.entries) || {}),
  };
  const { env: resolvedEnv, unresolved } = await resolveVaultRefs(entries);
  if (unresolved.length) {
    return {
      error: `could not resolve vault refs: ${unresolved.join(', ')}. Run \`vault login\` and set VAULT_ADDR.`,
    };
  }

  const extraEnv = { ...base };
  if (env && env.baseUrl) extraEnv.AGENT_QA_ENV_BASE_URL = String(env.baseUrl);
  if (auth.loginUrl) extraEnv.AGENT_QA_ENV_LOGIN_URL = String(auth.loginUrl);
  for (const [k, v] of Object.entries(auth.config || {})) {
    extraEnv[`AGENT_QA_ENV_${k.toUpperCase().replace(/[^A-Z0-9]+/g, '_')}`] = String(v);
  }
  Object.assign(extraEnv, resolvedEnv);

  // Register the profile against the env's plugin (idempotent — a no-op if the
  // persona was already Connected). Bootstrap itself happens inside replay.
  if (deps && typeof deps.runCli === 'function') {
    const addArgs = ['profile-add', profile];
    if (auth.plugin) addArgs.push('--adapter', String(auth.plugin));
    await deps.runCli(addArgs, extraEnv);
  }

  return { env: extraEnv, profile };
}

// -------- personas + environments (run-config records) --------
//
// Flat JSON records under <root>/_personas and <root>/_environments. A persona
// names a login identity (its `profile` is the value passed to `--profile`); an
// environment names a target (its `baseUrl` + `params` become `--param`s). Both
// carry a vendor-neutral connection block so a downstream auth plugin can sign
// in: the environment names the plugin + login entry; the persona names the
// env-var prefix holding its secrets (secrets never live here).
function strMap(src, fallback) {
  const out = {};
  const obj = src && typeof src === 'object' ? src : fallback && typeof fallback === 'object' ? fallback : {};
  for (const [k, v] of Object.entries(obj)) if (k) out[String(k)] = String(v);
  return out;
}

function normalizePersona(id, body, existing) {
  const now = Date.now();
  const cred = body.credentials || existing?.credentials || {};
  return {
    schema: 'persona/1',
    id,
    name: String(body.name ?? existing?.name ?? id),
    profile: String(body.profile ?? existing?.profile ?? id),
    // The login the agent picks when the user names no persona (and falls back
    // to the sole persona when nothing is flagged).
    default: typeof body.default === 'boolean' ? body.default : !!existing?.default,
    // Credentials this login hands the auth plugin: env-var name → value. Each
    // value may be a literal OR a `vault:<path>:<key>` reference resolved at
    // run time. Stored locally; vault refs hold no secret, just a pointer.
    credentials: {
      entries: strMap(cred.entries, existing?.credentials?.entries),
    },
    description: String(body.description ?? existing?.description ?? ''),
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
}

// Read a Vault token: $VAULT_TOKEN, else ~/.vault-token (e.g. after
// `vault login`). Returns null when neither is present.
async function readVaultToken() {
  if (process.env.VAULT_TOKEN && process.env.VAULT_TOKEN.trim()) return process.env.VAULT_TOKEN.trim();
  try {
    const t = await fsp.readFile(path.join(os.homedir(), '.vault-token'), 'utf8');
    return t.trim() || null;
  } catch {
    return null;
  }
}

// Resolve any `vault:<path>:<key>` values in a {name:value} map via the generic
// HashiCorp Vault KV HTTP API (GET <VAULT_ADDR>/v1/<path>, X-Vault-Token).
// Handles KV-v2 (data.data) and KV-v1 (data). Non-vault values pass through;
// values that can't be resolved are returned in `unresolved` (left literal).
async function resolveVaultRefs(map) {
  const out = {};
  const unresolved = [];
  const needsVault = Object.values(map).some((v) => typeof v === 'string' && v.startsWith('vault:'));
  const token = needsVault ? await readVaultToken() : null;
  const endpoint = needsVault ? String(process.env.VAULT_ADDR || '').replace(/\/$/, '') : '';
  for (const [name, value] of Object.entries(map)) {
    if (typeof value !== 'string' || !value.startsWith('vault:')) {
      out[name] = value;
      continue;
    }
    const parts = value.slice('vault:'.length).split(':');
    if (!endpoint || !token || parts.length !== 2 || !parts[0] || !parts[1]) {
      out[name] = value;
      unresolved.push(name);
      continue;
    }
    const [vpath, key] = parts;
    try {
      const r = await fetch(`${endpoint}/v1/${vpath}`, { headers: { 'X-Vault-Token': token } });
      if (!r.ok) {
        out[name] = value;
        unresolved.push(name);
        continue;
      }
      const j = await r.json();
      const v2 = j && j.data && j.data.data;
      const resolved = v2 && typeof v2[key] === 'string' ? v2[key] : j && j.data ? j.data[key] : undefined;
      if (typeof resolved === 'string') out[name] = resolved;
      else {
        out[name] = value;
        unresolved.push(name);
      }
    } catch {
      out[name] = value;
      unresolved.push(name);
    }
  }
  return { env: out, unresolved };
}

function normalizeEnvironment(id, body, existing) {
  const now = Date.now();
  const params = strMap(body.params, existing?.params);
  const auth = body.auth || existing?.auth || {};
  return {
    schema: 'environment/1',
    id,
    name: String(body.name ?? existing?.name ?? id),
    baseUrl: String(body.baseUrl ?? existing?.baseUrl ?? ''),
    params,
    // Connection: which auth plugin signs in here + free-form config it reads.
    // The plugin binary is supplied downstream (agent-qa.toml [plugins]); the
    // workbench only references it by name and never embeds vendor logic.
    auth: {
      plugin: String(auth.plugin ?? ''),
      loginUrl: String(auth.loginUrl ?? ''),
      config: strMap(auth.config, existing?.auth?.config),
      // Shared/app-level credentials for this environment (env-var name → value
      // | `vault:` ref), injected as bare env vars and MERGED UNDER a persona's
      // own creds at connect/run time. Put what every identity shares here (e.g.
      // the OAuth client id); keep per-identity email/password on the persona.
      creds: strMap(auth.creds, existing?.auth?.creds),
    },
    description: String(body.description ?? existing?.description ?? ''),
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
}

// Generic flat-record CRUD shared by personas + environments: GET list/one,
// POST upsert/delete, stored at <root>/<dirname>/<id>/<key>.json.
// Read `extra-dirs` for a table ([personas] / [environments]) from the global
// ~/.agent-qa/agent-qa.toml — the dirs installed extension packages register
// their read-only records under (see lib/packages.js). Best-effort parse.
function packageRecordDirs(tableName) {
  // Same config home the installer writes to (lib/packages.js): AGENT_QA_HOME
  // override, else ~/.agent-qa.
  const home = process.env.AGENT_QA_HOME || path.join(os.homedir(), '.agent-qa');
  const tomlPath = path.join(home, 'agent-qa.toml');
  let text;
  try {
    text = fs.readFileSync(tomlPath, 'utf8');
  } catch {
    return [];
  }
  const block = [];
  let inTable = false;
  for (const line of text.split('\n')) {
    const m = /^\s*\[([^\]]+)\]\s*$/.exec(line);
    if (m) {
      inTable = m[1].trim() === tableName;
      continue;
    }
    if (inTable) block.push(line);
  }
  const dirs = [];
  for (const q of block.join('\n').matchAll(/"([^"]+)"/g)) {
    const p = q[1];
    dirs.push(p.startsWith('~') ? path.join(os.homedir(), p.slice(1)) : p);
  }
  return dirs;
}

// Friendly package label from a registered dir path (…/packages/<scheme>/<name>/…).
function pkgLabelFromDir(dir) {
  const m = /\/packages\/[^/]+\/([^/]+)/.exec(dir);
  return m ? m[1] : path.basename(path.dirname(dir));
}

// Read-only records shipped by installed packages: flat `<id>.json` files in
// each registered dir. Tagged source=<pkg> + readOnly so the UI shows them
// distinctly and blocks edits (clone to customize). Local records shadow these.
async function readPackageRecords(tableName) {
  const out = [];
  for (const dir of packageRecordDirs(tableName)) {
    let files = [];
    try {
      files = await fsp.readdir(dir);
    } catch {
      continue;
    }
    const label = pkgLabelFromDir(dir);
    for (const f of files.sort()) {
      if (!f.endsWith('.json')) continue;
      const rec = await readJson(path.join(dir, f));
      if (!rec) continue;
      const id = String(rec.id || f.slice(0, -'.json'.length));
      out.push({ ...rec, id, source: label, readOnly: true });
    }
  }
  return out;
}

// Resolve a persona/environment by id: a local record first, else a read-only
// package-provided one. Used by connect + replay so personas shipped by an
// installed extension package are usable directly (not merely listable).
async function loadPersonaById(root, id) {
  return (
    (await readJson(path.join(root, '_personas', id, 'persona.json'))) ||
    (await readPackageRecords('personas')).find((p) => p.id === id) ||
    null
  );
}
async function loadEnvironmentById(root, id) {
  return (
    (await readJson(path.join(root, '_environments', id, 'environment.json'))) ||
    (await readPackageRecords('environments')).find((e) => e.id === id) ||
    null
  );
}

// All environments (local records shadow package-provided ones by id).
async function listAllEnvironments(root) {
  const out = [];
  const seen = new Set();
  let entries = [];
  try {
    entries = await fsp.readdir(path.join(root, '_environments'), { withFileTypes: true });
  } catch {
    /* none yet */
  }
  for (const e of entries) {
    if (!e.isDirectory()) continue;
    const rec = await readJson(path.join(root, '_environments', e.name, 'environment.json'));
    if (rec) {
      const id = String(rec.id || e.name);
      out.push({ ...rec, id });
      seen.add(id);
    }
  }
  for (const rec of await readPackageRecords('environments')) {
    if (!seen.has(rec.id)) out.push(rec);
  }
  return out;
}

// Pick the environment to use when the caller didn't name one: the one flagged
// `default`, else the sole environment, else null. Lets connect/replay inject an
// environment's shared config (e.g. an OAuth client id) even when the caller —
// or the chat agent — forgot to pass an environmentId. Ambiguous (2+, none
// default) → null, so we never silently pick the wrong target.
async function pickDefaultEnvironment(root) {
  const all = await listAllEnvironments(root);
  return all.find((e) => e.default) || (all.length === 1 ? all[0] : null);
}

// All personas (local records shadow package-provided ones by id).
async function listAllPersonas(root) {
  const out = [];
  const seen = new Set();
  let entries = [];
  try {
    entries = await fsp.readdir(path.join(root, '_personas'), { withFileTypes: true });
  } catch {
    /* none yet */
  }
  for (const e of entries) {
    if (!e.isDirectory()) continue;
    const rec = await readJson(path.join(root, '_personas', e.name, 'persona.json'));
    if (rec) {
      const id = String(rec.id || e.name);
      out.push({ ...rec, id });
      seen.add(id);
    }
  }
  for (const rec of await readPackageRecords('personas')) {
    if (!seen.has(rec.id)) out.push(rec);
  }
  return out;
}

// The persona to auto-sign-in a new chat with: the one flagged `default`, else
// the sole persona, else null (ambiguous → don't guess).
async function pickDefaultPersona(root) {
  const all = await listAllPersonas(root);
  return all.find((p) => p.default) || (all.length === 1 ? all[0] : null);
}

async function handleSimpleRecords(req, res, root, seg, cfg) {
  const { dirname, key, plural, normalize } = cfg;
  const method = req.method;
  const baseDir = path.join(root, dirname);
  const fileOf = (id) => path.join(baseDir, id, `${key}.json`);

  if (seg.length === 0) {
    if (method === 'GET') {
      let entries = [];
      try {
        entries = await fsp.readdir(baseDir, { withFileTypes: true });
      } catch {
        /* none yet */
      }
      const ids = entries
        .filter((e) => e.isDirectory() && isSafeSegment(e.name))
        .map((e) => e.name)
        .sort();
      const localIds = new Set(ids);
      const items = [];
      for (const id of ids) {
        const rec = await readJson(fileOf(id));
        if (rec) items.push({ ...rec, readOnly: false });
      }
      // Merge package-provided records (read-only); local ids win on collision.
      for (const p of await readPackageRecords(plural)) {
        if (!localIds.has(p.id)) items.push(p);
      }
      return sendJson(res, 200, { [plural]: items });
    }
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  const id = decodeURIComponent(seg[0]);
  if (!isSafeSegment(id)) return badRequest(res, `unsafe ${key} id`);
  const dir = path.join(baseDir, id);
  const file = fileOf(id);
  const localRec = await readJson(file);
  // A package record only "wins" this id when there's no local override.
  const pkgRec = localRec ? null : (await readPackageRecords(plural)).find((p) => p.id === id) || null;

  if (seg.length === 1) {
    if (method === 'GET') {
      const rec = localRec ? { ...localRec, readOnly: false } : pkgRec;
      if (!rec) return notFound(res, `no such ${key}`);
      return sendJson(res, 200, { [key]: rec });
    }
    if (method === 'POST') {
      if (pkgRec) {
        return sendJson(res, 409, {
          error: `${key} "${id}" is provided by package "${pkgRec.source}" (read-only) — clone it under a new id to customize`,
        });
      }
      let body;
      try {
        body = await readJsonBody(req);
      } catch (e) {
        return badRequest(res, String((e && e.message) || e));
      }
      const rec = normalize(id, body, localRec);
      await fsp.mkdir(dir, { recursive: true });
      await fsp.writeFile(file, JSON.stringify(rec, null, 2) + '\n');
      return sendJson(res, 200, { ok: true, [key]: rec });
    }
    return sendJson(res, 405, { error: 'method not allowed' });
  }

  if (seg.length === 2 && seg[1] === 'delete' && method === 'POST') {
    if (pkgRec) {
      return sendJson(res, 409, {
        error: `${key} "${id}" is provided by a package (read-only) — cannot delete`,
      });
    }
    try {
      await fsp.rm(dir, { recursive: true, force: true });
    } catch (e) {
      return sendJson(res, 500, { error: 'delete failed: ' + ((e && e.message) || e) });
    }
    return sendJson(res, 200, { ok: true, id, deleted: true });
  }

  return notFound(res, 'not found');
}

// -------- workbench plugin registry --------
//
// A UI-managed list of auth-plugin binary paths, stored at <root>/_config/
// plugins.json. The workbench injects it as AGENT_QA_PLUGINS for every CLI call
// it makes (discovery, connect, replay), so plugins can be registered entirely
// from the UI — no hand-edited agent-qa.toml required.
const pluginsConfigFile = (root) => path.join(root, '_config', 'plugins.json');

async function readPluginPaths(root) {
  const rec = await readJson(pluginsConfigFile(root));
  return Array.isArray(rec && rec.paths)
    ? rec.paths.filter((p) => typeof p === 'string' && p.trim())
    : [];
}

async function writePluginPaths(root, paths) {
  const clean = [
    ...new Set((Array.isArray(paths) ? paths : []).map((p) => String(p).trim()).filter(Boolean)),
  ];
  await fsp.mkdir(path.join(root, '_config'), { recursive: true });
  await fsp.writeFile(
    pluginsConfigFile(root),
    JSON.stringify({ schema: 'plugins/1', paths: clean }, null, 2) + '\n'
  );
  return clean;
}

// Colon-separated AGENT_QA_PLUGINS env for the registered paths (the CLI's
// env-var registration mechanism).
function pluginsEnv(paths) {
  return paths && paths.length ? { AGENT_QA_PLUGINS: paths.join(':') } : {};
}

// POST /api/personas/:id/connect { environmentId } — sign a persona in for an
// environment via the downstream auth plugin: profile-add (register the profile
// against the env's plugin) → profile-bootstrap (run the plugin's auth) →
// profile-status (report). The env's connection config is passed to the plugin
// as AGENT_QA_ENV_* vars; secrets stay in the user's own env (never here).
async function handleConnect(req, res, root, personaId, deps, opts = {}) {
  if (!isSafeSegment(personaId)) return badRequest(res, 'unsafe persona id');
  if (!deps || typeof deps.runCli !== 'function') {
    return sendJson(res, 503, { error: 'connect unavailable: agent-qa CLI not resolved' });
  }
  const persona = await loadPersonaById(root, personaId);
  if (!persona) return notFound(res, 'no such persona');
  const profile = String(persona.profile || '').trim();
  if (!profile) return badRequest(res, 'persona has no profile to bootstrap');

  // Optional target agent-browser session to authenticate (e.g. a chat's own
  // session, so the chat agent operates an already-signed-in page). Omitted →
  // the plugin's per-profile default session (`<profile>-session`).
  const sessionOverride = typeof opts.session === 'string' && opts.session ? opts.session : null;

  // The caller may have pre-parsed the request body (the chat connect route
  // reads it to extract personaId); reuse it so the stream isn't consumed twice.
  let body = opts.body;
  if (body === undefined) {
    try {
      body = await readJsonBody(req);
    } catch {
      body = {};
    }
  }
  const headed = !!(body && body.headed === true);
  let env = null;
  const envId = body.environmentId ? String(body.environmentId) : '';
  if (envId && isSafeSegment(envId)) {
    env = await loadEnvironmentById(root, envId);
  }
  // No environment named → fall back to the default/sole one, so the plugin
  // still gets the environment's shared config (e.g. an OAuth client id). This
  // is why a persona-only connect used to fail with "auth-failed: <cred> unset".
  if (!env) env = await pickDefaultEnvironment(root);
  const auth = (env && env.auth) || {};
  // An auth plugin can come from three places: the workbench's own registry
  // (UI-imported → AGENT_QA_PLUGINS), an environment's auth.plugin adapter
  // preference, or the CLI's native discovery (agent-qa.toml [plugins] /
  // AGENT_QA_PLUGINS / $PATH). Only block when none of them can serve `auth` —
  // a workbench with the plugin in agent-qa.toml must still be able to connect.
  const pluginPaths = await readPluginPaths(root);
  if (pluginPaths.length === 0 && !auth.plugin) {
    const probe = await deps.runCli(['plugins', 'path', 'auth'], {});
    const discoverable = !!(probe && probe.code === 0 && String(probe.stdout || '').trim());
    if (!discoverable) {
      return badRequest(
        res,
        'no auth plugin available — install an extension that provides one (Extensions → Install from npm / git), or add one to agent-qa.toml',
      );
    }
  }

  // Surface the environment's connection config to the plugin via env vars,
  // plus the UI-registered plugin paths so the plugin is discoverable.
  const extraEnv = { ...pluginsEnv(pluginPaths) };
  // Run profile-add / profile-bootstrap in the SAME record dir the chat agent
  // records under, so the scenario's `useProfile` op resolves the profile on
  // replay (otherwise it lands in the shared root the agent never looks in).
  if (opts.recordDir) extraEnv.AGENT_QA_RECORD_DIR = String(opts.recordDir);
  if (env && env.baseUrl) extraEnv.AGENT_QA_ENV_BASE_URL = String(env.baseUrl);
  if (auth.loginUrl) extraEnv.AGENT_QA_ENV_LOGIN_URL = String(auth.loginUrl);
  for (const [k, v] of Object.entries(auth.config || {})) {
    extraEnv[`AGENT_QA_ENV_${k.toUpperCase().replace(/[^A-Z0-9]+/g, '_')}`] = String(v);
  }

  const log = [];

  // Credentials for the plugin (env-var → value | `vault:` ref, resolved here).
  // The environment's shared creds (e.g. the OAuth client id) are the base; the
  // persona's identity creds (email/password) merge on top and win on any key
  // collision. Values may be literals or vault refs (token from `vault login` /
  // VAULT_TOKEN, endpoint from VAULT_ADDR).
  const entries = {
    ...(auth.creds || {}),
    ...((persona.credentials && persona.credentials.entries) || {}),
  };
  const { env: resolvedEnv, unresolved } = await resolveVaultRefs(entries);
  if (unresolved.length) {
    log.push({
      step: 'vault',
      code: 1,
      stdout: '',
      stderr: `could not resolve vault refs: ${unresolved.join(', ')}. Run \`vault login\` and set VAULT_ADDR.`,
      spawnError: null,
    });
    return sendJson(res, 200, { ok: false, authenticated: false, profile, log });
  }
  Object.assign(extraEnv, resolvedEnv);

  const step = async (label, args) => {
    const r = await deps.runCli(args, extraEnv);
    log.push({
      step: label,
      code: r.code,
      stdout: (r.stdout || '').slice(0, 4000),
      stderr: (r.stderr || '').slice(0, 4000),
      spawnError: r.spawnError ? String((r.spawnError && r.spawnError.message) || r.spawnError) : null,
    });
    return r;
  };

  // Register the profile (idempotent); the plugin is discovered from the
  // registry. The plugin reads credentials from the injected env directly.
  const addArgs = ['profile-add', profile];
  if (auth.plugin) addArgs.push('--adapter', String(auth.plugin));
  await step('profile-add', addArgs);
  const browserSession = sessionOverride || `${profile}-session`;
  let releaseBrowser;
  try {
    releaseBrowser = await prepareBrowserSession(deps, browserSession, headed);
  } catch (e) {
    log.push({
      step: 'session-recycle',
      code: 1,
      stdout: '',
      stderr: String((e && e.message) || e),
      spawnError: null,
    });
    return sendJson(res, 200, {
      ok: false,
      authenticated: false,
      profile,
      session: sessionOverride,
      headed,
      log,
    });
  }
  try {
    const bootArgs = ['profile-bootstrap', profile];
    if (sessionOverride) bootArgs.push('--session', sessionOverride);
    bootArgs.push(headed ? '--headed' : '--headless');
    const boot = await step('profile-bootstrap', bootArgs);
    const statArgs = ['profile-status', profile];
    if (sessionOverride) statArgs.push('--session', sessionOverride);
    const stat = await step('profile-status', statArgs);
    const authenticated = /authenticated/i.test(stat.stdout || '');
    if (authenticated) {
      // Remember the profile on the chat entry (so the agent's bash gets
      // AGENT_QA_PROFILE → a later `start` records a useProfile baseline) and
      // bind it into any recording already in progress (so a `start` that ran
      // BEFORE connect still replays under this profile, not fresh).
      if (opts.entry) {
        opts.entry.connectedProfile = profile;
        opts.entry.connectedPersonaId = personaId;
        opts.entry.connectedEnvironmentId = (env && env.id) || null;
      }
      if (opts.recordDir) await bindRecordingProfile(deps.runCli, profile);
    }
    return sendJson(res, 200, {
      ok: boot.code === 0,
      authenticated,
      profile,
      session: sessionOverride,
      headed,
      log,
    });
  } finally {
    releaseBrowser();
  }
}

// Add the connected profile to the active recorder through the Rust CLI.
// The CLI owns the typed state file and rejects malformed setup operations.
async function bindRecordingProfile(runCli, profile) {
  try {
    await runCli(['record-setup', JSON.stringify({ kind: 'useProfile', name: profile })]);
  } catch {
    /* best-effort */
  }
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
  // extraEnv: per-call env overrides (e.g. an environment's connection config
  // passed to an auth plugin during bootstrap). Merged over the fixed childEnv.
  return function runCli(args, extraEnv) {
    const callEnv = extraEnv ? { ...env, ...extraEnv } : env;
    return new Promise((resolve) => {
      execFile(
        bin,
        args,
        { env: callEnv, cwd, maxBuffer: 32 * 1024 * 1024 },
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

// The agent-browser session an authed replay drives. With a persona, reuse the
// profile's PERSISTENT, already-signed-in session (`<profile>-session`, the
// plugin's default) instead of a throwaway `replay-<sid>`: replay's useProfile
// bootstrap then no-ops on an authenticated session (and `/connect` pre-warms
// it), so re-runs skip the ~30s OAuth and are near-instant. Public (no-persona)
// scenarios keep a fresh per-sid session. Same-persona replays share one
// browser, so they serialise — covered by the "no double-fire" guard.
function sessionForReplay(sid, profile) {
  return profile ? `${profile}-session` : replaySessionFor(sid);
}

// Resolve the session a scenario's latest run drove (for the live screencast),
// from that run's recorded profile. Falls back to the fresh per-sid session.
async function replayStreamSession(root, sid) {
  const latest = await latestRunId(path.join(root, sid));
  if (latest) {
    const audit = await readJson(path.join(root, sid, 'replays', latest, 'audit.json'));
    if (audit && audit.profile) return sessionForReplay(sid, audit.profile);
  }
  return replaySessionFor(sid);
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

// Awaited counterpart used for mode changes. A warm agent-browser daemon keeps
// the launch mode it started with, so headed ↔ headless must close the session
// before the next CLI/plugin call launches it again. Unlike chat cleanup above,
// a failed close is surfaced — continuing would run in the wrong mode.
function makeBrowserSessionCloser({ bin, env, cwd }) {
  return function closeSession(session) {
    return new Promise((resolve, reject) => {
      execFile(
        bin || 'agent-browser',
        ['close', '--session', session],
        { env, cwd, maxBuffer: 1024 * 1024 },
        (err, _stdout, stderr) => {
          if (!err) return resolve();
          const detail = String(stderr || err.message || err).trim();
          reject(new Error(`could not recycle browser session ${session}: ${detail}`));
        },
      );
    });
  };
}

// Remember the requested mode per session while the workbench is running.
// Reusing the same mode preserves warm authenticated sessions; flipping it
// recycles exactly once. A daemon that predates this server has unknown mode,
// so adopt it safely by recycling it on first use. Acquisition is serialized,
// and an opposite-mode caller cannot recycle a session while an earlier
// connect/replay operation still holds a lease on it.
function makeBrowserModePreparer({ closeSession, activeSessions = listBrowserSessions }) {
  const states = new Map(); // session -> { headed, active }
  const pending = new Map();
  return function acquireBrowserSession(session, headed) {
    if (!session || !isSafeSegment(session)) {
      return Promise.reject(new Error('unsafe browser session'));
    }
    const desired = headed === true;
    const previous = pending.get(session) || Promise.resolve();
    const task = previous
      .catch(() => {})
      .then(async () => {
        const state = states.get(session);
        if (state && state.active > 0 && state.headed !== desired) {
          throw new Error(
            `browser session ${session} is busy in ${state.headed ? 'headed' : 'headless'} mode`,
          );
        }
        const liveUnknown = !state && (await activeSessions()).includes(session);
        if ((state && state.headed !== desired) || liveUnknown) await closeSession(session);

        const next = state || { headed: desired, active: 0 };
        next.headed = desired;
        next.active += 1;
        states.set(session, next);

        let released = false;
        return () => {
          if (released) return;
          released = true;
          next.active = Math.max(0, next.active - 1);
        };
      });
    pending.set(session, task);
    const clear = () => {
      if (pending.get(session) === task) pending.delete(session);
    };
    task.then(clear, clear);
    return task;
  };
}

async function prepareBrowserSession(deps, session, headed) {
  if (!deps || typeof deps.prepareBrowserSession !== 'function') return () => {};
  const release = await deps.prepareBrowserSession(session, headed === true);
  return typeof release === 'function' ? release : () => {};
}

// Acquire the session through child startup and, for the real detached spawner,
// hold it until the child exits. Test/custom replay deps that do not expose a
// completion promise release after startup, preserving the existing contract.
async function launchReplay(deps, sid, session, opts) {
  const release = await prepareBrowserSession(deps, session, opts && opts.headed);
  try {
    const out = await deps.replay(sid, session, opts);
    if (out && out.ok && out.done && typeof out.done.then === 'function') {
      out.done.then(release, release);
    } else {
      release();
    }
    return out;
  } catch (e) {
    release();
    throw e;
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

function replayArgs(sid, session, opts = {}) {
  const args = [
    'replay',
    sid,
    ...(session ? ['--session', session] : []),
    opts && opts.headed === true ? '--headed' : '--headless',
  ];
  if (opts && opts.profile) args.push('--profile', String(opts.profile));
  if (opts && opts.params && typeof opts.params === 'object') {
    for (const [k, v] of Object.entries(opts.params)) {
      if (k) args.push('--param', `${k}=${String(v)}`);
    }
  }
  return args;
}

function replayRunIds(root, sid) {
  if (!root || !isSafeSegment(sid)) return [];
  try {
    return fs
      .readdirSync(path.join(root, sid, 'replays'), { withFileTypes: true })
      .filter((e) => e.isDirectory() && isSafeSegment(e.name))
      .map((e) => e.name);
  } catch {
    return [];
  }
}

function writeJsonAtomicSync(file, value) {
  const tmp = `${file}.tmp-${process.pid}-${crypto.randomBytes(4).toString('hex')}`;
  try {
    fs.writeFileSync(tmp, JSON.stringify(value, null, 2) + '\n');
    fs.renameSync(tmp, file);
  } finally {
    try {
      fs.unlinkSync(tmp);
    } catch {
      /* rename succeeded or no temp was written */
    }
  }
}

// A replay can be terminated by a signal before Rust gets a chance to write
// status.json or finish audit.json. Without a host-side terminal marker, the
// Runs UI calls that dead process "in flight" forever. Only touch a run minted
// by THIS child and only when its audit is still incomplete.
function finalizeIncompleteReplay(root, sid, runsBefore, { code, signal } = {}) {
  const candidates = replayRunIds(root, sid)
    .filter((runId) => !runsBefore.has(runId))
    .sort()
    .reverse();
  for (const runId of candidates) {
    const runDir = path.join(root, sid, 'replays', runId);
    const auditPath = path.join(runDir, 'audit.json');
    let audit;
    try {
      audit = JSON.parse(fs.readFileSync(auditPath, 'utf8'));
    } catch {
      continue;
    }
    if (audit.finishedAt || audit.summary || typeof audit.exitCode === 'number') continue;

    const exitReason = signal
      ? `terminated by ${signal}`
      : typeof code === 'number'
        ? `exited ${code}`
        : 'exited without a status';
    const summary = `SUMMARY: 0/0 (FAIL — replay process ${exitReason} before finalizing)`;
    audit.finishedAt = new Date().toISOString();
    audit.exitCode = typeof code === 'number' && code !== 0 ? code : 1;
    audit.summary = summary;
    writeJsonAtomicSync(auditPath, audit);

    const statusPath = path.join(runDir, 'status.json');
    let status = null;
    try {
      status = JSON.parse(fs.readFileSync(statusPath, 'utf8'));
    } catch {
      status = null;
    }
    if (!status || status.state !== 'done') {
      writeJsonAtomicSync(statusPath, {
        state: 'done',
        currentIdx: Number(status && status.currentIdx) || 0,
        total: Number(status && status.total) || 0,
        ok: false,
      });
    }
    fs.writeFileSync(path.join(root, sid, 'replays', 'latest.txt'), runId + '\n');
    return { runId, summary };
  }
  return null;
}

const DEFAULT_REPLAY_SETUP_TIMEOUT_MS = 3 * 60 * 1000;

function replaySetupTimeoutMs(env) {
  const raw = env && env.AGENT_QA_REPLAY_SETUP_TIMEOUT_MS;
  if (raw == null || raw === '') return DEFAULT_REPLAY_SETUP_TIMEOUT_MS;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed < 0) return DEFAULT_REPLAY_SETUP_TIMEOUT_MS;
  return Math.floor(parsed);
}

function replayReportedProgress(root, sid, runsBefore) {
  for (const runId of replayRunIds(root, sid)) {
    if (runsBefore.has(runId)) continue;
    const runDir = path.join(root, sid, 'replays', runId);
    if (fs.existsSync(path.join(runDir, 'status.json')) || fs.existsSync(path.join(runDir, 'events.jsonl'))) {
      return true;
    }
    try {
      const audit = JSON.parse(fs.readFileSync(path.join(runDir, 'audit.json'), 'utf8'));
      if (audit.finishedAt || audit.summary || typeof audit.exitCode === 'number') return true;
    } catch {
      /* the child may not have minted its run yet */
    }
  }
  return false;
}

// Spawn a replay of a recorded scenario as a detached child. Replay is
// long-running (it drives a browser through every step), so we do NOT wait
// for it to finish — the viewer's live poll auto-follows the new run via
// status.json. Resolves quickly to { ok, pid } | { ok:false, error }.
//
// The setup watchdog covers the alive-but-stalled counterpart to the exit
// finalizer above: if a child writes no status/events for three minutes, stop
// it so the normal exit path can terminalize the run. Set
// AGENT_QA_REPLAY_SETUP_TIMEOUT_MS=0 to disable or tune the deadline.
function makeReplaySpawner({
  bin,
  env,
  cwd,
  root,
  spawnImpl = require('node:child_process').spawn,
  setupTimeoutMs = replaySetupTimeoutMs(env),
}) {
  // opts: { profile?, params?, env?, headed? } — a persona (forwarded as
  // `--profile`), environment values (each `--param k=v`), explicit browser
  // mode, and per-call env overrides (e.g. AGENT_QA_PLUGINS).
  return function replay(sid, session, opts = {}) {
    return new Promise((resolve) => {
      const args = replayArgs(sid, session, opts);
      const runsBefore = new Set(replayRunIds(root, sid));
      const childEnv = opts && opts.env ? { ...env, ...opts.env } : env;
      const child = spawnImpl(bin, args, { env: childEnv, cwd, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
      // Capture the replay child's output so a run that fails BEFORE writing a
      // step event (a crash, or an env.open bootstrap failure) still leaves a
      // diagnosable trail instead of a silently swallowed exit code. We keep the
      // detached/fire-and-forget model (the viewer follows status.json); we just
      // tee the tail to a per-scenario log and warn to the server log on failure.
      let tail = '';
      const onOut = (b) => { tail = (tail + b.toString()).slice(-4000); };
      if (child.stdout) child.stdout.on('data', onOut);
      if (child.stderr) child.stderr.on('data', onOut);
      let finish;
      let exited = false;
      let setupTimer = null;
      let forceKillTimer = null;
      const done = new Promise((resolveDone) => { finish = resolveDone; });
      child.once('exit', (code, signal) => {
        exited = true;
        if (setupTimer) clearTimeout(setupTimer);
        if (forceKillTimer) clearTimeout(forceKillTimer);
        let finalized = null;
        try {
          finalized = finalizeIncompleteReplay(root, sid, runsBefore, { code, signal });
        } catch (e) {
          console.error(`[replay ${sid}] could not finalize incomplete run: ${e.message || e}`);
        }
        if ((typeof code === 'number' && code !== 0) || signal || finalized) {
          const reason = signal ? `signal ${signal}` : `exit ${code}`;
          try {
            require('node:fs').writeFileSync(
              require('node:path').join(require('node:os').tmpdir(), `aqa-replay-${sid}.log`),
              tail || `${reason}\n${finalized ? finalized.summary : ''}\n`,
            );
          } catch { /* best effort */ }
          console.error(`[replay ${sid}] ${reason}:\n${(tail || finalized?.summary || '').slice(-1500)}`);
        }
        finish({ code, signal, finalizedRunId: finalized && finalized.runId });
      });
      let started = false;
      child.once('spawn', () => {
        if (started) return;
        started = true;
        child.unref();
        if (setupTimeoutMs > 0) {
          setupTimer = setTimeout(() => {
            setupTimer = null;
            if (exited || replayReportedProgress(root, sid, runsBefore)) return;
            const msg = `setup watchdog: no replay status after ${setupTimeoutMs}ms; terminating child`;
            tail = (tail + `\n[agent-qa host] ${msg}\n`).slice(-4000);
            console.error(`[replay ${sid}] ${msg}`);
            try {
              child.kill('SIGTERM');
              forceKillTimer = setTimeout(() => {
                if (!exited) {
                  tail = (tail + '\n[agent-qa host] setup child ignored SIGTERM; sending SIGKILL\n').slice(-4000);
                  try { child.kill('SIGKILL'); } catch { /* already gone */ }
                }
              }, 5000);
              forceKillTimer.unref?.();
            } catch {
              /* exit/error handler owns the terminal state */
            }
          }, setupTimeoutMs);
          setupTimer.unref?.();
        }
        resolve({ ok: true, pid: child.pid, done });
      });
      child.once('error', (err) => {
        exited = true;
        if (setupTimer) clearTimeout(setupTimer);
        if (forceKillTimer) clearTimeout(forceKillTimer);
        finish({ error: err });
        if (started) return;
        started = true;
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
  const parsed = lastJsonLine(r.stdout) || {};
  return {
    sid: parsed.sid || null,
    intent: parsed.intent || null,
    session: parsed.session || null,
    baseline: parsed.baseline || null,
    rows: Array.isArray(parsed.rows) ? parsed.rows : [],
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
      // Rust clears the only active recorder state. The empty scenario directory
      // remains on disk and stays hidden from the scenario list.
      const r = await deps.runCli(['buffer', 'discard']);
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

// Read the chat's active recorder state, or its last sealed scenario.
async function resolveChatRecording(dir) {
  const state = await readJson(path.join(dir, 'recorder-state.json'));
  if (state && state.sid) return { state, active: true };
  const last = (await readText(path.join(dir, 'scenario.last')) || '').trim();
  return { state: null, active: false, sid: last || null };
}

function normalizeStep(step, index) {
  return {
    stepIndex: index,
    stepId: step.id || `s${index}`,
    kind: step.kind || 'step',
    payload: step,
    intent: step.intent || null,
    recordedAt: step.recordedAt || null,
  };
}

async function chatRecordingState(entry, scenariosRoot) {
  const out = { recording: false, sid: null, intent: null, session: null, startedAt: null, baseline: null, flushed: false, steps: [], autoConnect: entry.autoConnect || null };
  const dir = entry.recordDir();
  if (!dir) return out;
  const recording = await resolveChatRecording(dir);
  if (recording.active) {
    const state = recording.state;
    out.sid = state.sid;
    out.intent = state.intent || null;
    out.session = state.session || null;
    out.startedAt = state.startedAt || null;
    out.baseline = state.baseline || null;
    out.steps = Array.isArray(state.steps) ? state.steps.map(normalizeStep) : [];
    if (scenariosRoot && isSafeSegment(state.sid) && await readJson(path.join(scenariosRoot, state.sid, 'scenario.json'))) {
      out.flushed = true;
      return out;
    }
    out.recording = true;
    return out;
  }
  if (!recording.sid) return out;
  out.sid = recording.sid;
  out.flushed = true;
  out.session = (entry.browser && entry.browser.name) || null;
  if (scenariosRoot && isSafeSegment(recording.sid)) {
    const scenario = await readJson(path.join(scenariosRoot, recording.sid, 'scenario.json'));
    if (!scenario) return { ...out, sid: null, flushed: false };
    out.intent = scenario.intent || null;
    out.steps = Array.isArray(scenario.steps) ? scenario.steps.map(normalizeStep) : [];
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
  const recording = await resolveChatRecording(dir);
  const sid = recording.active ? recording.state.sid : recording.sid;
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
// A minimal `res` that captures a JSON handler's response instead of writing to
// a socket — lets the background auto-connect below reuse handleConnect() with
// no HTTP round-trip (and no dependency on deps.baseUrl being set yet).
function makeCaptureRes() {
  const out = { status: 0, body: '' };
  return {
    _out: out,
    setHeader() {},
    writeHead(code) {
      out.status = code;
    },
    end(buf) {
      out.body = buf ? buf.toString() : '';
    },
  };
}

// Sign the default persona into a freshly-created chat's browser session, in the
// background, so the agent navigates an ALREADY-authenticated page instead of
// racing to connect (and recording a sign-in-page step first). Best-effort +
// non-blocking; only fires when a default/sole persona AND environment resolve
// (a configured, auth-walled workbench) — a no-op otherwise. Reuses the exact
// connect path the agent would call, so it also sets entry.connectedProfile
// (which makes `agent-qa start` record a useProfile baseline automatically).
async function autoConnectDefault(root, entry, deps) {
  try {
    if (!root || !deps || typeof deps.runCli !== 'function') return;
    const [persona, env] = await Promise.all([pickDefaultPersona(root), pickDefaultEnvironment(root)]);
    if (!persona || !env) return;
    entry.autoConnect = { state: 'connecting', personaId: persona.id, environmentId: env.id, at: Date.now() };
    const res = makeCaptureRes();
    await handleConnect({}, res, root, persona.id, deps, {
      session: entry.browser.name,
      recordDir: entry.recordDir(),
      entry,
      body: { personaId: persona.id, environmentId: env.id },
    });
    let parsed = {};
    try {
      parsed = JSON.parse(res._out.body || '{}');
    } catch {
      /* leave empty */
    }
    entry.autoConnect = {
      state: parsed.authenticated ? 'connected' : 'failed',
      personaId: persona.id,
      environmentId: env.id,
      at: Date.now(),
    };
  } catch {
    if (entry) entry.autoConnect = { state: 'failed', at: Date.now() };
  }
}

function createChatManager(deps, root) {
  const chat = deps && deps.chat;
  const recordRoot = deps && deps.recordRoot;
  // Localhost base URL of this server, so each chat's agent can call back to the
  // workbench (e.g. POST /api/chat/c/<id>/connect to sign a persona into its own
  // browser session). Set by start(); absent in tests.
  const baseUrl = deps && deps.baseUrl;
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
                // So the agent can self-serve persona sign-in for THIS chat:
                // POST {personaId} to $AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/connect.
                env.AGENT_QA_CHAT_ID = entry.id;
                if (baseUrl) env.AGENT_QA_BASE = baseUrl;
                // Once a persona is connected, `agent-qa start` defaults its
                // baseline to this profile (no --profile needed) so recordings
                // are replayable under that login.
                if (entry.connectedProfile) env.AGENT_QA_PROFILE = entry.connectedProfile;
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
    // Pre-sign-in the default persona into this chat's session (background) so an
    // auth-walled recording starts already authenticated — no reliance on the
    // agent remembering to connect before its first navigation.
    if (root) {
      Promise.resolve()
        .then(() => autoConnectDefault(root, entry, deps))
        .catch(() => {});
    }
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
    const dir = entry.recordDir();
    if (dir) {
      const recording = await resolveChatRecording(dir);
      if (recording.active) recorder = recording.state.session || null;
    }
    const mine = entry.browser.name;
    return sendJson(res, 200, {
      session: recorder || mine,
      recording: !!recorder && recorder === mine,
    });
  }

  // Current persona sign-in state for this chat's browser. The frontend polls
  // while the background default-persona connect is still running.
  if (sub === 'connection' && req.method === 'GET') {
    const auto = entry.autoConnect || {};
    return sendJson(res, 200, {
      state: entry.connectedProfile ? 'connected' : auto.state || 'disconnected',
      personaId: entry.connectedPersonaId || auto.personaId || null,
      environmentId: entry.connectedEnvironmentId || auto.environmentId || null,
      profile: entry.connectedProfile || null,
    });
  }

  // Live view of THIS chat's recording: the steps recorded so far + their
  // per-step screenshot/snapshot artifacts. Cheap file reads — pollable.
  if (sub === 'recording' && req.method === 'GET') {
    return sendJson(res, 200, await chatRecordingState(entry, scenariosRoot));
  }

  // Connect a persona's auth INTO this chat's own browser session, so the chat
  // agent operates an already-signed-in page (no credentials in the agent's
  // hands). Reuses the persona connect flow — resolve vault creds → profile-add
  // → profile-bootstrap → profile-status — but targets the chat's agent-browser
  // session via --session instead of the plugin's per-profile default.
  if (sub === 'connect' && req.method === 'POST') {
    let body = {};
    try {
      body = await readJsonBody(req);
    } catch (e) {
      return badRequest(res, String((e && e.message) || e));
    }
    const personaId = body.personaId ? String(body.personaId) : '';
    if (!personaId) return badRequest(res, 'personaId is required');
    return handleConnect(req, res, scenariosRoot, personaId, deps, {
      session: entry.browser.name,
      body,
      // Register/bootstrap the profile in THIS chat's record dir (so replay's
      // useProfile op finds it) and remember it on the entry so the agent's
      // bash + the in-progress recording bind to it.
      recordDir: entry.recordDir(),
      entry,
    });
  }

  // Replay a scenario the chat recorded, re-authenticating via the connected
  // persona. The agent CANNOT run `agent-qa replay` itself for an auth-walled
  // flow — replay's `useProfile` op needs the persona's credentials (incl.
  // vault refs) which only the workbench resolves. So this endpoint mirrors
  // /connect: resolve creds → run replay in THIS chat's (already-signed-in)
  // session with the profile + creds in env. Synchronous: returns pass/fail.
  if (sub === 'replay' && req.method === 'POST') {
    if (!deps || typeof deps.runCli !== 'function') {
      return sendJson(res, 503, { error: 'replay unavailable: agent-qa CLI not resolved' });
    }
    let body = {};
    try {
      body = await readJsonBody(req);
    } catch (e) {
      return badRequest(res, String((e && e.message) || e));
    }
    const sid = body.sid ? String(body.sid) : '';
    if (!sid || !isSafeSegment(sid)) return badRequest(res, 'sid is required');
    const headed = body.headed === true;
    const personaId = entry.connectedPersonaId;
    const profile = entry.connectedProfile;
    if (!personaId || !profile) {
      return badRequest(res, 'connect a persona first (POST /api/chat/c/<id>/connect)');
    }
    // Same cred layering as /connect and the Runs tab (resolveRunAuthEnv):
    // environment shared creds (e.g. the OAuth client id) under the persona's
    // identity creds, plus AGENT_QA_ENV_* — replay's useProfile re-bootstrap
    // needs all of it, not just the persona's entries. Environment: the one
    // named in the body, else the one connect used, else the default/sole.
    const envId = body.environmentId ? String(body.environmentId) : entry.connectedEnvironmentId || '';
    const resolved = await resolveRunAuthEnv(scenariosRoot, deps, {
      personaId,
      environmentId: envId,
    });
    if (resolved.error) return sendJson(res, 200, { ok: false, error: resolved.error });
    const extraEnv = { ...resolved.env };
    const rdir = entry.recordDir();
    if (rdir) extraEnv.AGENT_QA_RECORD_DIR = rdir;
    let releaseBrowser;
    try {
      releaseBrowser = await prepareBrowserSession(deps, entry.browser.name, headed);
    } catch (e) {
      return sendJson(res, 200, { ok: false, error: String((e && e.message) || e) });
    }
    try {
      const r = await deps.runCli(
        [
          'replay',
          sid,
          '--session',
          entry.browser.name,
          headed ? '--headed' : '--headless',
          '--profile',
          resolved.profile || profile,
        ],
        extraEnv,
      );
      const tail = (s) =>
        String(s || '')
          .split('\n')
          .filter(Boolean)
          .slice(-4)
          .join('\n');
      return sendJson(res, 200, {
        ok: r.code === 0,
        code: r.code,
        summary: tail(r.stdout),
        error: r.code === 0 ? undefined : tail(r.stderr) || tail(r.stdout),
      });
    } finally {
      releaseBrowser();
    }
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
  const chatManager = chat || createChatManager(deps, root);
  return async function handle(req, res) {
    try {
      const url = new URL(req.url, 'http://127.0.0.1');
      const p = url.pathname;

      // Editor write surface. Only mounted when a CLI runner is
      // provided (i.e. launched from the Node launcher with a resolved
      // Rust binary). Handles its own methods (GET + POST).
      const segAll = p.split('/').filter(Boolean);

      // App version for the sidebar. Works without a resolved CLI.
      if (segAll[0] === 'api' && segAll[1] === 'version' && req.method === 'GET') {
        return sendJson(res, 200, { version: await resolveAppVersion(deps) });
      }

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

      // Test-case authoring surface (pure JSON metadata under <root>/_cases).
      // Unconditional — works without a resolved CLI, like GET /api/scenarios.
      if (segAll[0] === 'api' && segAll[1] === 'cases') {
        return await handleCases(req, res, root, segAll.slice(2));
      }

      // Test sets (curated collections of cases) — pure JSON under <root>/_sets.
      // Like /api/cases, works without a resolved CLI.
      if (segAll[0] === 'api' && segAll[1] === 'sets') {
        return await handleSets(req, res, root, segAll.slice(2));
      }

      // Test plans (runnable scope of sets/cases) under <root>/_plans. CRUD is
      // pure JSON; the /run action needs deps.replay (handled within).
      if (segAll[0] === 'api' && segAll[1] === 'plans') {
        return await handlePlans(req, res, root, segAll.slice(2), deps);
      }

      // Personas (login identities → --profile) and environments (target
      // values → --param) — flat run-config records under <root>/_personas,
      // <root>/_environments.
      // Bootstrap a persona's login for an environment (needs deps.runCli).
      if (
        segAll[0] === 'api' &&
        segAll[1] === 'personas' &&
        segAll[3] === 'connect' &&
        segAll.length === 4 &&
        req.method === 'POST'
      ) {
        return await handleConnect(req, res, root, decodeURIComponent(segAll[2]), deps);
      }
      if (segAll[0] === 'api' && segAll[1] === 'personas') {
        return await handleSimpleRecords(req, res, root, segAll.slice(2), {
          dirname: '_personas',
          key: 'persona',
          plural: 'personas',
          normalize: normalizePersona,
        });
      }
      if (segAll[0] === 'api' && segAll[1] === 'environments') {
        return await handleSimpleRecords(req, res, root, segAll.slice(2), {
          dirname: '_environments',
          key: 'environment',
          plural: 'environments',
          normalize: normalizeEnvironment,
        });
      }

      // Read-only auth-plugin discovery for the run-config UI. Shells
      // `plugins list --json`; reports availability instead of erroring when no
      // CLI is resolved so the UI can show a plain "not configured" state.
      if (segAll[0] === 'api' && segAll[1] === 'plugins' && segAll.length === 2 && req.method === 'GET') {
        if (!deps || typeof deps.runCli !== 'function') {
          return sendJson(res, 200, { available: false, plugins: [] });
        }
        const r = await deps.runCli(
          ['plugins', 'list', '--json'],
          pluginsEnv(await readPluginPaths(root))
        );
        let plugins = [];
        try {
          plugins = JSON.parse(r.stdout || '[]');
        } catch {
          plugins = [];
        }
        return sendJson(res, 200, { available: true, plugins });
      }

      // Import a downloaded plugin file: save it under <root>/_config/plugins,
      // mark it executable, and register its path. Body { filename, contentBase64 }.
      if (
        segAll[0] === 'api' &&
        segAll[1] === 'config' &&
        segAll[2] === 'plugins' &&
        segAll[3] === 'import' &&
        segAll.length === 4 &&
        req.method === 'POST'
      ) {
        let body;
        try {
          body = await readJsonBody(req, 16 * 1024 * 1024);
        } catch (e) {
          return badRequest(res, String((e && e.message) || e));
        }
        // basename + safe charset; no traversal, no hidden/dotfiles.
        const safe = String(body.filename || '')
          .replace(/^.*[\\/]/, '')
          .replace(/[^A-Za-z0-9._-]/g, '_')
          .replace(/^\.+/, '');
        if (!safe) return badRequest(res, 'invalid filename');
        if (!body.contentBase64) return badRequest(res, 'missing file content');
        let buf;
        try {
          buf = Buffer.from(String(body.contentBase64), 'base64');
        } catch {
          return badRequest(res, 'invalid base64 content');
        }
        const dir = path.join(root, '_config', 'plugins');
        await fsp.mkdir(dir, { recursive: true });
        const dest = path.join(dir, safe);
        await fsp.writeFile(dest, buf);
        try {
          await fsp.chmod(dest, 0o755);
        } catch {
          /* best-effort on platforms without chmod */
        }
        const paths = await writePluginPaths(root, [...(await readPluginPaths(root)), dest]);
        return sendJson(res, 200, { ok: true, path: dest, paths });
      }

      // Install an extension package from npm/git/https (the `agent-qa install`
      // flow, so users don't drop to a terminal): lib/packages.js fetches it
      // into ~/.agent-qa/packages and wires ~/.agent-qa/agent-qa.toml with the
      // discovered plugins + skill dirs. Body { source }. Runs synchronously
      // (npm/git under the hood) — fine for this localhost single-user tool.
      // Install failures return 200 { ok:false, error } so the UI can show them.
      if (
        segAll[0] === 'api' &&
        segAll[1] === 'config' &&
        segAll[2] === 'plugins' &&
        segAll[3] === 'install' &&
        segAll.length === 4 &&
        req.method === 'POST'
      ) {
        let body;
        try {
          body = await readJsonBody(req);
        } catch (e) {
          return badRequest(res, String((e && e.message) || e));
        }
        const source = String((body && body.source) || '').trim();
        if (!source) {
          return badRequest(res, 'source is required (npm:<pkg> | git:<url> | https://…)');
        }
        let pkgs;
        try {
          pkgs = require('./packages.js');
        } catch {
          return sendJson(res, 503, { error: 'package installer unavailable' });
        }
        try {
          const r = pkgs.install(source);
          return sendJson(res, 200, {
            ok: true,
            name: r.name,
            plugins: (r.plugins || []).map((p) => ({ path: p.path, kinds: p.kinds || [] })),
            skills: (r.skills || []).length,
          });
        } catch (e) {
          return sendJson(res, 200, {
            ok: false,
            error: String((e && e.message) || e).slice(0, 2000),
          });
        }
      }

      // Installed extension packages (the `agent-qa install` registry) — for
      // the Plugins page's Update/Remove UI. Read-only, safe fields only.
      if (
        segAll[0] === 'api' &&
        segAll[1] === 'config' &&
        segAll[2] === 'packages' &&
        segAll.length === 3 &&
        req.method === 'GET'
      ) {
        let pkgs;
        try {
          pkgs = require('./packages.js');
        } catch {
          return sendJson(res, 200, { packages: [] });
        }
        const list = (pkgs.listPackages() || []).map((p) => ({
          source: p.source,
          name: p.name,
          scheme: p.scheme,
          plugins: (p.plugins || []).flatMap((x) => x.kinds || []),
          skills: (p.skills || []).length,
          personas: (p.personas || []).length,
          environments: (p.environments || []).length,
        }));
        return sendJson(res, 200, { packages: list });
      }

      // Re-pull (update) an installed package. Body { source }.
      if (
        segAll[0] === 'api' &&
        segAll[1] === 'config' &&
        segAll[2] === 'plugins' &&
        segAll[3] === 'update' &&
        segAll.length === 4 &&
        req.method === 'POST'
      ) {
        let body;
        try {
          body = await readJsonBody(req);
        } catch (e) {
          return badRequest(res, String((e && e.message) || e));
        }
        const source = String((body && body.source) || '').trim();
        if (!source) return badRequest(res, 'source is required');
        let pkgs;
        try {
          pkgs = require('./packages.js');
        } catch {
          return sendJson(res, 503, { error: 'package installer unavailable' });
        }
        try {
          const r = pkgs.update(source);
          if (!r.length) return sendJson(res, 200, { ok: false, error: `no installed package matching ${source}` });
          return sendJson(res, 200, { ok: true, updated: r.map((x) => x.name) });
        } catch (e) {
          return sendJson(res, 200, { ok: false, error: String((e && e.message) || e).slice(0, 2000) });
        }
      }

      // Uninstall an installed package. Body { source }. Drops it from the
      // registry, rewires agent-qa.toml, and deletes its fetched files.
      if (
        segAll[0] === 'api' &&
        segAll[1] === 'config' &&
        segAll[2] === 'plugins' &&
        segAll[3] === 'uninstall' &&
        segAll.length === 4 &&
        req.method === 'POST'
      ) {
        let body;
        try {
          body = await readJsonBody(req);
        } catch (e) {
          return badRequest(res, String((e && e.message) || e));
        }
        const source = String((body && body.source) || '').trim();
        if (!source) return badRequest(res, 'source is required');
        let pkgs;
        try {
          pkgs = require('./packages.js');
        } catch {
          return sendJson(res, 503, { error: 'package installer unavailable' });
        }
        try {
          const removed = pkgs.remove(source);
          if (!removed) return sendJson(res, 200, { ok: false, error: `no installed package matching ${source}` });
          return sendJson(res, 200, { ok: true, removed });
        } catch (e) {
          return sendJson(res, 200, { ok: false, error: String((e && e.message) || e).slice(0, 2000) });
        }
      }

      // Check for updates — per installed package + the app itself. Network-
      // bound (git ls-remote / npm view), so it's a GET the UI calls on demand.
      if (
        segAll[0] === 'api' &&
        segAll[1] === 'config' &&
        segAll[2] === 'packages' &&
        segAll[3] === 'updates' &&
        segAll.length === 4 &&
        req.method === 'GET'
      ) {
        let packages = [];
        try {
          packages = require('./packages.js').checkUpdates();
        } catch {
          /* installer unavailable */
        }
        let app = null;
        try {
          const pkgJson = require('../package.json');
          const current = await resolveAppVersion(deps);
          const latest = require('node:child_process')
            .execFileSync('npm', ['view', pkgJson.name, 'version'], { encoding: 'utf8', timeout: 20000 })
            .trim();
          app = {
            name: pkgJson.name,
            current,
            latest,
            updateAvailable: !!latest && latest !== current && current !== 'dev',
          };
        } catch {
          /* offline / not published */
        }
        return sendJson(res, 200, { packages, app });
      }

      // UI-managed plugin registry — list/set the auth-plugin paths the
      // workbench injects as AGENT_QA_PLUGINS. Pure JSON under <root>/_config.
      if (segAll[0] === 'api' && segAll[1] === 'config' && segAll[2] === 'plugins' && segAll.length === 3) {
        if (req.method === 'GET') {
          return sendJson(res, 200, { paths: await readPluginPaths(root) });
        }
        if (req.method === 'POST') {
          let body;
          try {
            body = await readJsonBody(req);
          } catch (e) {
            return badRequest(res, String((e && e.message) || e));
          }
          return sendJson(res, 200, { ok: true, paths: await writePluginPaths(root, body.paths) });
        }
        return sendJson(res, 405, { error: 'method not allowed' });
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
        // Optional persona/environment for this replay: { profile, params,
        // personaId, environmentId }. A named persona resolves + injects its
        // credentials so an auth-walled scenario re-authenticates on replay.
        let replayOpts = {};
        try {
          replayOpts = runOptsFromBody(await readJsonBody(req));
        } catch {
          /* no/!json body → defaults */
        }
        const auth = await resolveRunAuthEnv(root, deps, replayOpts);
        if (auth.error) return sendJson(res, 200, { ok: false, error: auth.error });
        replayOpts.env = auth.env;
        if (auth.profile) replayOpts.profile = auth.profile;
        const session = sessionForReplay(sid, replayOpts.profile);
        let out;
        try {
          out = await launchReplay(deps, sid, session, replayOpts);
        } catch (e) {
          return sendJson(res, 500, { error: String((e && e.message) || e) });
        }
        if (!out.ok) return sendJson(res, 500, { error: out.error || 'replay failed to start' });
        return sendJson(res, 202, { ok: true, sid, started: true });
      }

      // Delete a recorded scenario (POST): remove its dir + all replays via the
      // Rust CLI `scenario delete <sid> --yes`. Requires deps.runCli.
      if (
        req.method === 'POST' &&
        segAll[0] === 'api' &&
        segAll[1] === 'scenarios' &&
        segAll[3] === 'delete' &&
        segAll.length === 4
      ) {
        if (!deps || !deps.runCli) {
          return sendJson(res, 503, { error: 'delete unavailable: agent-qa CLI not resolved' });
        }
        const sid = decodeURIComponent(segAll[2]);
        if (!isSafeSegment(sid)) return badRequest(res, 'unsafe sid');
        const r = await deps.runCli(['scenario', 'delete', sid, '--yes']);
        if (r.spawnError) return sendJson(res, 503, { error: 'agent-qa CLI not runnable' });
        if (r.code !== 0) return sendJson(res, 500, { error: (r.stderr || '').trim() || 'delete failed' });
        return sendJson(res, 200, { ok: true, sid, deleted: true });
      }

      // Delete a single replay run (POST): remove its dir + repoint latest.txt.
      if (
        req.method === 'POST' &&
        segAll[0] === 'api' &&
        segAll[1] === 'scenarios' &&
        segAll[3] === 'runs' &&
        segAll[5] === 'delete' &&
        segAll.length === 6
      ) {
        const sid = decodeURIComponent(segAll[2]);
        const runId = decodeURIComponent(segAll[4]);
        if (!isSafeSegment(sid) || !isSafeSegment(runId)) return badRequest(res, 'unsafe id');
        const replaysDir = path.join(root, sid, 'replays');
        try {
          await fsp.rm(path.join(replaysDir, runId), { recursive: true, force: true });
        } catch (e) {
          return sendJson(res, 500, { error: 'delete failed: ' + (e.message || e) });
        }
        // If latest.txt pointed at the deleted run, repoint to the newest
        // remaining run (ids sort by timestamp), else drop the pointer.
        try {
          const latestPath = path.join(replaysDir, 'latest.txt');
          const latest = ((await readText(latestPath)) || '').trim();
          if (latest === runId) {
            const entries = await fsp.readdir(replaysDir, { withFileTypes: true }).catch(() => []);
            const runs = entries.filter((e) => e.isDirectory()).map((e) => e.name).sort();
            if (runs.length) await fsp.writeFile(latestPath, runs[runs.length - 1] + '\n');
            else await fsp.rm(latestPath, { force: true });
          }
        } catch {
          /* best-effort */
        }
        return sendJson(res, 200, { ok: true, sid, runId, deleted: true });
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
          const bridge = deps.liveForSession(await replayStreamSession(root, sid));
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

// Resolve the app version for the UI: the umbrella package.json carries the
// real version on a published install; in the source tree it's "0.0.0", so
// fall back to the resolved CLI binary's `--version`. Cached after first read.
let _appVersion = null;
async function resolveAppVersion(deps) {
  if (_appVersion) return _appVersion;
  let v = '';
  try {
    v = String(require('../package.json').version || '');
  } catch {
    /* ignore */
  }
  if ((!v || v === '0.0.0') && deps && typeof deps.runCli === 'function') {
    try {
      const r = await deps.runCli(['--version'], {});
      const m = /(\d+\.\d+\.\d+[^\s]*)/.exec(String(r.stdout || ''));
      if (m) v = m[1];
    } catch {
      /* CLI unavailable */
    }
  }
  _appVersion = v && v !== '0.0.0' ? v : 'dev';
  return _appVersion;
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
  const chatManager = createChatManager(deps, root);
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
  // Survive flaky third-party pi extensions. The chat agent loads the user's
  // pi extensions (~/.pi/agent/extensions); some hold a captured ctx in a timer
  // and throw "stale ctx" after we dispose a session on idle/new-chat. That's an
  // uncaught exception in a setTimeout — fatal by default. A localhost dev
  // server must not die because someone's titlebar-spinner extension threw, so
  // log it loudly and keep serving. (Opt out with AGENT_QA_STRICT_CRASH=1.)
  if (process.env.AGENT_QA_STRICT_CRASH !== '1' && !start._guarded) {
    start._guarded = true;
    process.on('uncaughtException', (err) => {
      console.error(`agent-qa web: ignored uncaught exception — ${(err && err.stack) || err}`);
    });
    process.on('unhandledRejection', (err) => {
      console.error(`agent-qa web: ignored unhandled rejection — ${(err && err.stack) || err}`);
    });
  }
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
    const cwd = opts.cwd || process.cwd();
    const agentBrowserBin = opts.agentBrowserBin || process.env.AGENT_BROWSER_BIN || 'agent-browser';
    const closeSession = makeBrowserSessionCloser({ bin: agentBrowserBin, env: childEnv, cwd });
    deps = {
      recordRoot,
      agentBrowserBin,
      runCli: makeCliRunner({ bin, env: childEnv, cwd }),
      replay: makeReplaySpawner({ bin, env: childEnv, cwd, root }),
      prepareBrowserSession: makeBrowserModePreparer({ closeSession }),
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
    // So each chat's agent can call back to sign a persona into its own session.
    deps.baseUrl = `http://${host}:${port}`;
  }

  const server = createServer(root, deps);

  server.on('error', (err) => {
    if (err && err.code === 'EADDRINUSE') {
      console.error(`agent-qa web: port ${port} is already in use on ${host}.`);
      console.error('Pass a different port: agent-qa web --port <N>');
    } else {
      console.error(`agent-qa web: ${err.message || err}`);
    }
    process.exit(1);
  });

  server.listen(port, host, () => {
    const url = `http://${host}:${port}/`;
    console.error(`agent-qa web — run viewer + authoring editor + chat`);
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
  listScenarios,
  listRuns,
  findActiveRunId,
  runDetail,
  scenarioSummary,
  makeCliRunner,
  lastJsonLine,
  createLiveBridge,
  createChatManager,
  autoConnectDefault,
  pickDefaultPersona,
  createServer,
  createRequestHandler,
  start,
  ARTIFACT_KINDS,
  EDIT_KINDS,
  _test: {
    replayArgs,
    runOptsFromBody,
    makeBrowserModePreparer,
    launchReplay,
    finalizeIncompleteReplay,
    makeReplaySpawner,
    replaySetupTimeoutMs,
  },
};
