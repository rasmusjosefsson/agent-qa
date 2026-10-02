// agent-qa in-app chat backend (ESM).
//
// Wraps a single pi `AgentSession` so the localhost app's Chat tab can talk
// to the same agent you get in the terminal — same skills, extensions,
// models, and credentials, discovered from the repo cwd via the SDK's
// DefaultResourceLoader. It is a *thin* relay: the SDK owns the session, this
// module owns SSE fan-out + lifecycle (lazy spawn, idle dispose, shutdown).
//
// Split in two so it stays testable without a real LLM:
//   • createChatHub({ createSession })  — pure wrapper; inject a fake session
//     to test event→SSE fan-out, prompt/abort routing, and dispose().
//   • createChatBackend(config)         — resolves + imports the pi SDK, then
//     builds a real session factory and hands it to createChatHub().
//
// The browser↔server contract (/api/chat/* + SSE) is identical regardless of
// how the session is produced, so the front-end is written once.

import { createRequire } from 'node:module';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { existsSync, statSync, realpathSync, readFileSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, delimiter } from 'node:path';
import { homedir } from 'node:os';
import { spawn } from 'node:child_process';
import { createServer as createNetServer } from 'node:net';

const PI_PKG = '@earendil-works/pi-coding-agent';
const OPENCODE_PKG = '@opencode-ai/sdk';
const OPENCODE_BIN = 'opencode';

// 15 minutes of no chat activity tears the session down. A chat must survive
// SSE reconnects (the session holds conversation state), so this is the one
// deliberate, scoped exception to the "alive only while a stream is open" rule.
const DEFAULT_IDLE_MS = 15 * 60 * 1000;

// -------- JSON-safe SSE serialization --------

// Agent events / messages are plain data, but guard against cycles, functions,
// and BigInt so a single odd field can never throw mid-stream and kill the SSE
// connection.
function safeStringify(obj) {
  const seen = new WeakSet();
  return JSON.stringify(obj, function replacer(_key, value) {
    if (typeof value === 'function') return undefined;
    if (typeof value === 'bigint') return value.toString();
    if (value && typeof value === 'object') {
      if (seen.has(value)) return undefined;
      seen.add(value);
    }
    return value;
  });
}

function safeClone(obj) {
  if (obj == null) return obj;
  try {
    return JSON.parse(safeStringify(obj));
  } catch {
    return null;
  }
}

function modelInfo(model) {
  if (!model || typeof model !== 'object') return null;
  return {
    provider: model.provider ?? null,
    id: model.id ?? null,
    label: model.label ?? model.name ?? model.id ?? null,
    reasoning: !!model.reasoning,
    contextWindow: model.contextWindow ?? null,
  };
}

// -------- the pure wrapper --------

/**
 * @param {object} opts
 * @param {() => (Promise<any>|any)} opts.createSession  Factory returning an AgentSession.
 * @param {number} [opts.idleMs]   Dispose the session after this much inactivity (0 disables).
 * @param {{set:Function, clear:Function}} [opts.timers]  Injectable timers (tests).
 * @param {(msg:string)=>void} [opts.logger]
 */
export function createChatHub({
  createSession,
  listModels = null,
  idleMs = DEFAULT_IDLE_MS,
  timers = { set: setTimeout, clear: clearTimeout },
  logger = () => {},
} = {}) {
  if (typeof createSession !== 'function') {
    throw new TypeError('createChatHub: createSession is required');
  }

  const subscribers = new Set();
  let session = null;
  let unsub = null;
  let creating = null;
  let disposed = false;
  let idleTimer = null;
  // Set by newSession() so the next createSession starts a clean conversation
  // rather than resuming the persisted one.
  let nextFresh = false;

  function touch() {
    if (idleTimer) timers.clear(idleTimer);
    if (idleMs > 0 && !disposed) {
      idleTimer = timers.set(() => {
        logger('chat idle timeout — disposing session');
        teardownSession();
        broadcast('session', { type: 'session_idle' });
      }, idleMs);
      if (idleTimer && typeof idleTimer.unref === 'function') idleTimer.unref();
    }
  }

  function frame(event, obj) {
    return `event: ${event}\ndata: ${safeStringify(obj)}\n\n`;
  }

  function broadcast(event, obj) {
    const payload = frame(event, obj);
    for (const res of subscribers) {
      try {
        res.write(payload);
      } catch {
        subscribers.delete(res);
      }
    }
  }

  // Forward agent events verbatim (chat.js switches on event.type). The SDK
  // event shapes already match the plan's mapping table.
  function onSessionEvent(event) {
    if (!event || typeof event !== 'object') return;
    broadcast('agent', event);
  }

  async function ensureSession() {
    if (disposed) throw new Error('chat hub disposed');
    if (session) return session;
    if (!creating) {
      // `fresh` = the caller asked for a brand-new conversation (New chat) —
      // backends skip their persisted session resume for this one create.
      const fresh = nextFresh;
      nextFresh = false;
      creating = Promise.resolve()
        .then(() => createSession({ fresh }))
        .then((s) => {
          if (disposed) {
            try {
              s.dispose();
            } catch {
              /* ignore */
            }
            throw new Error('chat hub disposed');
          }
          session = s;
          unsub = s.subscribe(onSessionEvent);
          broadcast('session', { type: 'session_ready', sessionId: s.sessionId ?? null });
          return s;
        })
        .catch((err) => {
          creating = null;
          throw err;
        });
    }
    return creating;
  }

  function teardownSession() {
    if (idleTimer) {
      timers.clear(idleTimer);
      idleTimer = null;
    }
    if (unsub) {
      try {
        unsub();
      } catch {
        /* ignore */
      }
      unsub = null;
    }
    if (session) {
      try {
        session.dispose();
      } catch {
        /* ignore */
      }
      session = null;
    }
    creating = null;
  }

  // -------- public surface --------

  function subscribe(res) {
    if (disposed) {
      try {
        res.end();
      } catch {
        /* ignore */
      }
      return;
    }
    subscribers.add(res);
    touch();
    try {
      res.write(': connected\n\n');
    } catch {
      subscribers.delete(res);
    }
  }

  function unsubscribe(res) {
    subscribers.delete(res);
  }

  // Usage/cost accounting for the chat's model badge. pi's AgentSession
  // exposes getSessionStats(); the opencode adapter accumulates a compatible
  // shape on .usage from the server's per-message accounting.
  function sessionUsage(s) {
    try {
      if (s && typeof s.getSessionStats === 'function') {
        const st = s.getSessionStats();
        if (st && typeof st === 'object') {
          return { cost: st.cost ?? null, tokens: st.tokens || null };
        }
      }
      if (s && s.usage && typeof s.usage === 'object') return s.usage;
    } catch {
      /* best-effort */
    }
    return null;
  }

  async function getState() {
    const models = currentModels();
    if (!session) {
      return {
        started: false,
        streaming: false,
        messages: [],
        streamingMessage: null,
        model: null,
        thinkingLevel: null,
        thinkingLevels: [],
        models,
        sessionId: null,
        usage: null,
      };
    }
    const state = session.agent && session.agent.state ? session.agent.state : {};
    return {
      started: true,
      streaming: !!session.isStreaming,
      messages: safeClone(session.messages) || [],
      streamingMessage: safeClone(state.streamingMessage) || null,
      model: modelInfo(session.model),
      thinkingLevel: session.thinkingLevel ?? null,
      thinkingLevels: availableThinkingLevels(session),
      models,
      sessionId: session.sessionId ?? null,
      usage: sessionUsage(session),
    };
  }

  // Models the registry can actually authenticate (cheap, in-memory). Works
  // before a session exists so the composer can show the picker immediately.
  function currentModels() {
    try {
      return typeof listModels === 'function' ? listModels() || [] : [];
    } catch {
      return [];
    }
  }

  function availableThinkingLevels(s) {
    try {
      if (typeof s.getAvailableThinkingLevels === 'function') {
        return s.getAvailableThinkingLevels() || [];
      }
    } catch {
      /* ignore */
    }
    return [];
  }

  // Switch the active model. Spawns the session if needed (no LLM call), then
  // returns the fresh state so the caller can echo it straight back.
  async function setModel(sel) {
    if (!sel || !sel.id) throw new Error('model id is required');
    const s = await ensureSession();
    touch();
    const reg = s.modelRegistry;
    let m = null;
    if (reg && typeof reg.find === 'function') m = reg.find(sel.provider, sel.id);
    if (!m && reg && typeof reg.getAvailable === 'function') {
      m = (reg.getAvailable() || []).find(
        (x) => x && x.id === sel.id && (!sel.provider || x.provider === sel.provider),
      );
    }
    if (!m) throw new Error(`model not available: ${sel.provider || ''}/${sel.id}`);
    await s.setModel(m);
    return getState();
  }

  async function setThinkingLevel(level) {
    const s = await ensureSession();
    touch();
    if (typeof s.setThinkingLevel === 'function') s.setThinkingLevel(level);
    return getState();
  }

  async function prompt(text, opts = {}) {
    try {
      const s = await ensureSession();
      touch();
      const promptOpts = {};
      if (opts.streamingBehavior === 'steer' || opts.streamingBehavior === 'followUp') {
        promptOpts.streamingBehavior = opts.streamingBehavior;
      } else if (s.isStreaming) {
        // A new message while the agent is working: steer it in after the
        // current turn's tool calls finish (the natural chat behavior).
        promptOpts.streamingBehavior = 'steer';
      }
      await s.prompt(text, promptOpts);
    } catch (err) {
      broadcast('agent', { type: 'error', message: String((err && err.message) || err) });
      throw err;
    }
  }

  async function abort() {
    if (!session) return;
    touch();
    await session.abort();
  }

  // Drop the current conversation and let the next prompt spawn a fresh one.
  async function newSession() {
    nextFresh = true;
    teardownSession();
    if (!disposed) touch();
    broadcast('session', { type: 'session_reset' });
  }

  function dispose() {
    if (disposed) return;
    disposed = true;
    teardownSession();
    for (const res of subscribers) {
      try {
        res.end();
      } catch {
        /* ignore */
      }
    }
    subscribers.clear();
  }

  return {
    subscribe,
    unsubscribe,
    prompt,
    abort,
    newSession,
    getState,
    setModel,
    setThinkingLevel,
    dispose,
    get subscriberCount() {
      return subscribers.size;
    },
    get hasSession() {
      return !!session;
    },
    get disposed() {
      return disposed;
    },
  };
}

// -------- pi SDK resolution --------

function toExistingEntry(candidate) {
  if (!candidate) return null;
  let fsPath = candidate;
  if (candidate.startsWith('file://')) {
    try {
      fsPath = fileURLToPath(candidate);
    } catch {
      return null;
    }
  }
  if (!existsSync(fsPath)) return null;
  let st;
  try {
    st = statSync(fsPath);
  } catch {
    return null;
  }
  if (st.isDirectory()) {
    const dist = join(fsPath, 'dist', 'index.js');
    if (existsSync(dist)) return dist;
    const idx = join(fsPath, 'index.js');
    if (existsSync(idx)) return idx;
    return null;
  }
  return fsPath;
}

function findNodeModulesPackageEntry(moduleUrl, packageName) {
  let dir;
  try {
    dir = dirname(fileURLToPath(moduleUrl));
  } catch {
    return null;
  }
  const packageParts = packageName.split('/').filter(Boolean);
  for (;;) {
    const entry = toExistingEntry(join(dir, 'node_modules', ...packageParts));
    if (entry) return entry;
    const parent = dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

function whichOnPath(name, env) {
  const PATH = env.PATH || env.Path || '';
  if (!PATH) return null;
  const exts =
    process.platform === 'win32' ? (env.PATHEXT || '.EXE;.CMD;.BAT').split(';') : [''];
  for (const dir of PATH.split(delimiter)) {
    if (!dir) continue;
    for (const ext of exts) {
      const full = join(dir, name + ext);
      if (existsSync(full)) return full;
    }
  }
  return null;
}

/**
 * Resolve the pi SDK entry (dist/index.js) to an importable file:// URL.
 * Order (first hit wins):
 *   1. explicit sdkPath / AGENT_QA_PI_SDK (file or package dir)
 *   2. package directory from this module's node_modules chain (our optional dep)
 *   3. bare specifier from this module's node_modules chain
 *   4. a global install next to the running node binary
 *   5. a `pi` binary on PATH → realpath → dist/cli.js → sibling dist/index.js
 *
 * The direct node_modules walk matters for SDK releases whose package exports
 * define only an ESM `import` condition. `createRequire().resolve()` uses the
 * CommonJS condition and skips those otherwise-valid colocated installs.
 *
 * @returns {string} file:// URL
 */
export function resolvePiSdkUrl({ sdkPath, env = process.env, moduleUrl = import.meta.url } = {}) {
  const candidates = [];

  const explicit = sdkPath || env.AGENT_QA_PI_SDK;
  if (explicit) candidates.push(explicit);

  candidates.push(findNodeModulesPackageEntry(moduleUrl, PI_PKG));

  try {
    const require = createRequire(moduleUrl);
    candidates.push(require.resolve(PI_PKG));
  } catch {
    /* package may export only the ESM import condition */
  }

  try {
    const execDir = dirname(process.execPath);
    candidates.push(join(execDir, '..', 'lib', 'node_modules', PI_PKG, 'dist', 'index.js'));
  } catch {
    /* ignore */
  }

  try {
    const piPath = whichOnPath('pi', env);
    if (piPath) {
      // The launcher is typically a symlink into the package's dist/cli.js.
      const real = realpathSync(piPath);
      candidates.push(join(dirname(real), 'index.js'));
    }
  } catch {
    /* ignore */
  }

  for (const c of candidates) {
    const entry = toExistingEntry(c);
    if (entry) return pathToFileURL(entry).href;
  }

  throw new Error(
    `pi SDK (${PI_PKG}) not found. Install it, or set AGENT_QA_PI_SDK to its ` +
      `dist/index.js (or package dir).`,
  );
}

export async function loadPiSdk(config = {}) {
  const url = resolvePiSdkUrl(config);
  const sdk = await import(url);
  return { sdk, url };
}

// -------- agent-qa awareness (primer + skill discovery) --------

// Appended to the chat agent's system prompt so it behaves like the terminal
// agent-qa skill instead of a blank coding agent. Without this the pi session
// has no idea it's inside agent-qa — it never loads the recorder/replay skills
// and tries to "drive a browser" by guessing (the classic
// "Unknown agent: agent-browser" + wrong-host failures). Kept vendor-neutral:
// any app-specific host/route knowledge comes from the loaded skills, never
// from this string.
const AGENT_QA_PRIMER = [
  'You are the built-in assistant of the **agent-qa workbench**. agent-qa drives a',
  'REAL browser (Chrome over CDP) to open, inspect, record, and replay UI flows',
  'against live web apps. You are already running inside an agent-qa environment —',
  'act on that without being told to "use agent-qa".',
  '',
  'Operating rules:',
  '- A dedicated browser session is already bound to THIS chat via the env var',
  '  $AGENT_BROWSER_SESSION. All browser work happens there. Do NOT launch your own',
  '  browser and do NOT try to delegate to a sub-agent named "agent-browser" —',
  '  `agent-browser` and `agent-qa` are command-line tools you call from the bash',
  '  tool (e.g. `agent-browser open <url>`).',
  '- For ANY task that opens, navigates, inspects, records, or replays a page, FIRST',
  '  run `agent-qa skills get core` and use it for command + flag SYNTAX — never guess',
  '  commands. BUT `core` is the manual for the STANDALONE CLI. Where its runbook',
  '  conflicts with the WORKBENCH RULES below (sessions, sign-in, profiles), the',
  '  workbench rules WIN. Concretely: IGNORE core\'s example `SESSION=…; --session',
  '  "$SESSION"` lines and its `profile-list` / `profile-bootstrap` / `profile-add`',
  '  prerequisites — those are standalone-CLI mechanics, not how this workbench works.',
  '- App-specific overlays (the correct host, environment, and known routes for a',
  '  given product) ship as skills. Before navigating a specific app, run',
  '  `agent-qa skills list`, load the matching overlay skill(s) with',
  '  `agent-qa skills get <name>`, and use the host/route they specify. Never guess a',
  '  host or URL path.',
  '',
  'SESSION — non-negotiable, overrides anything core says:',
  '- NEVER invent a session name and NEVER set your own SESSION shell variable',
  '  (e.g. `SESSION="accounts-create-$(date +%s)"` is WRONG). Do NOT pass --session to',
  '  `agent-qa start` — it already defaults to $AGENT_BROWSER_SESSION. Pass',
  '  --session "$AGENT_BROWSER_SESSION" to EVERY `agent-browser` command. Any other',
  '  session is a fresh, UNAUTHENTICATED browser — the classic failure.',
  '',
  'SHELL — each bash tool call runs in a FRESH shell; variables do NOT persist between',
  'calls. A `SID=$(agent-qa start …)` in one call is empty in the next. `record-step`,',
  '`flush`, and `verify` do not need the sid (they use this chat\'s record dir), so just',
  'call them bare. When you DO need the sid (e.g. `list <sid>`, `replay <sid>`), copy the',
  'literal sid `agent-qa start` printed and paste it — never re-derive it with',
  '`ls -t … | head -1` (that can grab a different, concurrent scenario).',
  '',
  'SIGN-IN — the workbench AUTO-signs-in the default persona when a chat opens, so you',
  'are usually ALREADY authenticated. Your job: VERIFY before recording; sign in',
  'explicitly only if the auto-sign-in is still finishing, failed, or you need a',
  'different persona.',
  '- GUEST MODE — check `curl -s "$AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/connection"`:',
  '  when it returns `"guest": true` the user deliberately picked "No sign-in".',
  '  Browse and record ANONYMOUSLY — never connect a persona, never push sign-in,',
  '  and never treat a sign-in screen on the site itself as a problem to fix;',
  '  it is the page under test. A `useProfile` baseline is absent on purpose.',
  '  If the user later connects a persona, guest clears and normal rules apply.',
  '- VERIFY FIRST — before `agent-qa start` and before recording anything: open the',
  '  target URL, snapshot, and confirm you are PAST the sign-in screen. If you see a',
  '  "Sign in" heading, the auto-sign-in is still finishing (~30-60s on a fresh chat)',
  '  or failed — connect explicitly (below), WAIT for it, re-open, and only then start.',
  '  Do NOT run `agent-qa start` / `record-step` while a snapshot still shows sign-in.',
  '- Workbench logins are "personas", NOT CLI profiles. `agent-qa profile-list` is',
  '  IRRELEVANT here and will usually be empty — that is EXPECTED and does NOT mean',
  '  "no auth available", so never decide auth from it, and never tell the user to log',
  '  in or click anything. Never run `agent-qa profile-add` / `profile-bootstrap`',
  '  yourself — they cannot see the credentials; only the workbench resolves them',
  '  (incl. vault refs).',
  '- List personas: `curl -s "$AGENT_QA_BASE/api/personas"` (each has id, name,',
  '  profile, maybe `default: true`). Pick the one the user names; else the default;',
  '  else the only one; if several exist and none is default, ASK.',
  '- Sign the chosen persona into THIS chat\'s browser by POSTing its id AND the',
  '  target environment id (from `curl -s "$AGENT_QA_BASE/api/environments"`):',
  '    curl -s -X POST "$AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/connect" \\',
  '      -H "content-type: application/json" -d \'{"personaId":"<id>","environmentId":"<env>"}\'',
  '  Include the environmentId — the environment carries shared app config (e.g. an',
  '  OAuth client id) the auth plugin needs; without it sign-in fails "auth-failed:',
  '  <VAR> unset". (If you omit it the workbench falls back to the default/sole',
  '  environment, but pass it explicitly when there\'s more than one.)',
  '  This ONE call resolves credentials + runs the auth plugin. It BLOCKS until',
  '  sign-in finishes (~30-60s) — do not `sleep` and guess; wait for it to return and',
  '  READ the JSON. `{"authenticated":true,…}` = signed in. Connect BEFORE the first',
  '  navigation.',
  '- After a successful connect the tab is NOT on your target — the login redirect',
  '  left it on the app landing (or a blank/redirect page). So you MUST re-open the',
  '  target URL yourself (`agent-browser --session "$AGENT_BROWSER_SESSION" open <url>`)',
  '  and THEN snapshot. A snapshot taken right after connect, without re-opening, will',
  '  show the stale pre-connect page — that is NOT an auth failure.',
  '- If connect returns `{"authenticated":false,…}` (or non-2xx), the response has a',
  '  `log` array of the profile-add / profile-bootstrap / vault steps with stdout+stderr.',
  '  Report the FAILING step\'s message verbatim (e.g. unresolved vault ref, bad',
  '  credentials, no auth plugin) — that is the actual problem to fix. NEVER ask the user',
  '  to sign in, open a login page, or hand you credentials: the workbench holds them,',
  '  and the browser is headless to the user anyway. Retry connect once; if it still',
  '  fails, surface the log and stop.',
  '',
  'RECORDING an authenticated flow:',
  '  (1) Confirm you are signed in FIRST (the chat auto-connects the default persona;',
  '      if a snapshot still shows sign-in, connect explicitly and wait — see SIGN-IN).',
  '      Being connected binds that profile to the scenario so it RE-AUTHENTICATES on',
  '      replay (the workbench sets it as the recording baseline + $AGENT_QA_PROFILE, so',
  '      a plain `agent-qa start "<intent>"` records under that login — no --profile).',
  '  (2) `agent-qa start "<intent>"` (no --session, no --profile needed). Do this ONLY',
  '      after (1) shows you are past sign-in — starting earlier records a bad step 0.',
  '  (3) Snapshot to CONFIRM the target page (not sign-in) before the FIRST record-step.',
  '      If a "Sign in" heading shows, auth is not ready — connect, wait, re-open; do',
  '      NOT proceed and do NOT ask the user to sign in.',
  '  (4) Only THEN record the navigation step + the rest. NEVER record or assert the',
  '      sign-in page as if it were the target. If you already recorded a step whose',
  '      snapshot captured the sign-in/loading page (e.g. you navigated before auth was',
  '      ready), `agent-qa truncate <N>` back to before it and re-record after auth —',
  '      do not leave a step 0 whose keyframe is the sign-in page (it breaks `compare`).',
  '  (5) To REPLAY/verify an authenticated scenario, do NOT run `agent-qa replay`',
  '      yourself — it cannot re-authenticate (the credentials live in the workbench,',
  '      not your shell). POST the sid to the workbench, which re-auths and replays in',
  '      this chat\'s signed-in session:',
  '        curl -s -X POST "$AGENT_QA_BASE/api/chat/c/$AGENT_QA_CHAT_ID/replay" \\',
  '          -H "content-type: application/json" -d \'{"sid":"<sid>"}\'',
  '      It returns {"ok":true,...} on a clean replay, or {"ok":false,"error":…}. (Raw',
  '      `agent-qa replay` only works for scenarios that need no sign-in.)',
  '- Prefer agent-qa / agent-browser commands over any other automation. If a command',
  '  fails, report the exact error verbatim; do not silently switch tools.',
].join('\n');

const expandTilde = (p) => (p && p.startsWith('~') ? join(homedir(), p.slice(1)) : p);

// Resolve the agent-qa skill directories so the chat agent can see (and load on
// demand) the same skills the CLI serves — `core` is bundled with the binary,
// but overlays like route catalogs / host rules live in dirs listed under
// `[skills] extra-dirs` in agent-qa.toml. We read the global config plus the
// nearest agent-qa.toml walked up from cwd (per-repo overlay), mirroring the
// CLI's own discovery order. Best-effort: returns [] if nothing is found.
function resolveAgentQaSkillDirs(cwd) {
  const tomlPaths = [join(homedir(), '.agent-qa', 'agent-qa.toml')];
  if (process.env.XDG_CONFIG_HOME) {
    tomlPaths.push(join(process.env.XDG_CONFIG_HOME, 'agent-qa', 'agent-qa.toml'));
  }
  let d = cwd || process.cwd();
  for (;;) {
    const p = join(d, 'agent-qa.toml');
    if (existsSync(p)) {
      tomlPaths.push(p);
      break;
    }
    const parent = dirname(d);
    if (parent === d) break;
    d = parent;
  }

  // Reuse the package manager's tested toml splitter when available; fall back to
  // a tiny [skills] extractor so a missing sibling never breaks the chat.
  let splitToml = null;
  try {
    splitToml = createRequire(import.meta.url)('./packages.js').splitToml;
  } catch {
    /* fall back to the regex extractor below */
  }
  const extractSkillsDirs = (text) => {
    if (splitToml) {
      try {
        return splitToml(text).extraDirs || [];
      } catch {
        /* fall through */
      }
    }
    const m = /\[skills\]([\s\S]*?)(?:\n\[|$)/.exec(text || '');
    if (!m) return [];
    return [...m[1].matchAll(/"([^"]+)"/g)].map((q) => q[1]);
  };

  const dirs = [];
  for (const p of tomlPaths) {
    let text;
    try {
      text = readFileSync(p, 'utf8');
    } catch {
      continue;
    }
    for (const raw of extractSkillsDirs(text)) {
      const abs = expandTilde(raw);
      if (abs && existsSync(abs) && !dirs.includes(abs)) dirs.push(abs);
    }
  }
  return dirs;
}

// Session persistence for the pi backend. sessionDir holds the
// SessionManager's JSONL files; `current.path` inside it marks the file the
// last live session wrote, so a session recreated after idle teardown (or a
// server restart) reopens the same conversation instead of starting blank.
async function openOrCreateSessionManager({ SessionManager, cwd, sessionDir, remembered, fresh }) {
  const fallback = () => ({ manager: SessionManager.inMemory(cwd), file: null });
  if (!sessionDir || !SessionManager || typeof SessionManager.create !== 'function') return fallback();
  try {
    await mkdir(sessionDir, { recursive: true });
    const marker = join(sessionDir, 'current.path');
    let prior = fresh ? null : remembered;
    if (!prior && !fresh) {
      try {
        prior = (await readFile(marker, 'utf8')).trim() || null;
      } catch {
        prior = null;
      }
    }
    let manager = null;
    if (prior && existsSync(prior) && typeof SessionManager.open === 'function') {
      try {
        manager = SessionManager.open(prior, sessionDir, cwd);
      } catch {
        manager = null; // corrupt/moved file → fall through to a fresh session
      }
    }
    if (!manager) manager = SessionManager.create(cwd, sessionDir);
    const file = typeof manager.getSessionFile === 'function' ? manager.getSessionFile() : null;
    if (file) await writeFile(marker, file);
    return { manager, file };
  } catch {
    return fallback();
  }
}

// Build a createSession() that mints a fresh in-memory AgentSession bound to
// the repo cwd, inheriting skills/extensions/models/keys via the SDK's default
// resource loader + auth storage — i.e. the same agent as the terminal.
function makeSessionFactory(sdk, config = {}, shared = {}) {
  const {
    createAgentSession,
    AuthStorage,
    ModelRegistry,
    SessionManager,
    SettingsManager,
    DefaultResourceLoader,
    getAgentDir,
    createReadToolDefinition,
    createBashToolDefinition,
    createEditToolDefinition,
    createWriteToolDefinition,
  } = sdk;
  const cwd = config.cwd || process.cwd();
  const agentDir =
    config.agentDir ||
    (typeof getAgentDir === 'function' ? getAgentDir() : join(homedir(), '.pi', 'agent'));
  // Resolved once: the agent-qa skill dirs to surface to every chat session.
  const agentQaSkillDirs = resolveAgentQaSkillDirs(cwd);
  // Optional per-conversation env injected into every bash spawn (e.g. a
  // dedicated AGENT_BROWSER_SESSION so this chat's browsing + recording land in
  // its own browser). Read lazily so a New-chat session rotation is picked up.
  const bashEnv = typeof config.bashEnv === 'function' ? config.bashEnv : null;
  // JSONL file of the session this factory most recently opened — lets a
  // session recreated after idle teardown resume the same conversation.
  let rememberedSessionFile = null;

  return async function createSession({ fresh } = {}) {
    // Share one AuthStorage + ModelRegistry with the hub's model picker so the
    // model listed in /state is the same object setModel() resolves against.
    const authStorage = shared.authStorage || AuthStorage.create();
    const modelRegistry = shared.modelRegistry || ModelRegistry.create(authStorage);
    const opts = {
      cwd,
      authStorage,
      modelRegistry,
    };
    if (agentDir) opts.agentDir = agentDir;

    // Make the chat agent agent-qa-aware. A custom resource loader keeps every
    // pi default (cwd skills, AGENTS.md, extensions, themes) but ALSO appends
    // the agent-qa primer to the system prompt and registers the agent-qa skill
    // dirs so `core` + app overlays show up and load on demand — i.e. the chat
    // behaves like the terminal agent-qa skill instead of a blank coding agent.
    // Best-effort: any failure falls back to the SDK's default loader so chat
    // keeps working (just without the primer).
    try {
      if (DefaultResourceLoader) {
        const settingsManager =
          SettingsManager && typeof SettingsManager.create === 'function'
            ? SettingsManager.create(cwd, agentDir)
            : undefined;
        const loader = new DefaultResourceLoader({
          cwd,
          agentDir,
          settingsManager,
          appendSystemPrompt: [AGENT_QA_PRIMER],
          additionalSkillPaths: agentQaSkillDirs,
        });
        await loader.reload();
        opts.resourceLoader = loader;
        if (settingsManager) opts.settingsManager = settingsManager;
      }
    } catch {
      /* fall back to the SDK's default resource loader */
    }

    // Default tools = full parity with the terminal (read/bash/edit/write +
    // any custom/extension tools). Pass an allowlist to run a limited mode.
    if (Array.isArray(config.tools) && config.tools.length) opts.tools = config.tools;

    // With a per-conversation bashEnv, swap the built-in read/bash/edit/write
    // for definitions that inject the env into every bash spawn (keeping
    // extension/custom tools enabled via noTools:'builtin'). This is how each
    // chat gets its own browser session without a process-global env var.
    if (bashEnv && typeof createBashToolDefinition === 'function') {
      const spawnHook = (ctx) => ({ ...ctx, env: { ...ctx.env, ...(bashEnv() || {}) } });
      opts.noTools = 'builtin';
      opts.tools = undefined;
      opts.customTools = [
        createReadToolDefinition(cwd),
        createBashToolDefinition(cwd, { spawnHook }),
        createEditToolDefinition(cwd),
        createWriteToolDefinition(cwd),
      ];
    }

    // Persist the conversation so the idle teardown (DEFAULT_IDLE_MS) doesn't
    // lose history: with config.sessionDir the SessionManager writes an
    // append-only JSONL file there, and the next lazy session reopens it.
    // `current.path` marks the file the last session wrote — SessionManager
    // picks its own filename, so we track it ourselves.
    const sessionManager = await openOrCreateSessionManager({
      SessionManager,
      cwd,
      sessionDir: config.sessionDir,
      remembered: fresh ? null : rememberedSessionFile,
      fresh,
    });
    if (sessionManager.file) rememberedSessionFile = sessionManager.file;
    opts.sessionManager = sessionManager.manager;

    const { session } = await createAgentSession(opts);

    // Default to a cheaper model for debugging (the UI can still switch). Prefer
    // AGENT_QA_CHAT_MODEL (substring match on provider/id/label), else the first
    // Haiku; if neither matches, keep the SDK default. Also ensures /state
    // reports a concrete model instead of an empty picker.
    try {
      const avail =
        (typeof modelRegistry.getAvailable === 'function' ? modelRegistry.getAvailable() : []) || [];
      const want = (process.env.AGENT_QA_CHAT_MODEL || '').toLowerCase();
      const key = (m) =>
        `${m?.provider || ''}/${m?.id || ''} ${m?.label || m?.name || ''}`.toLowerCase();
      const pick = avail.find((m) => (want ? key(m).includes(want) : /haiku/.test(key(m))));
      if (pick && typeof session.setModel === 'function') await session.setModel(pick);
    } catch {
      /* keep the SDK default */
    }

    return session;
  };
}

// -------- opencode backend (v2 SDK over `opencode serve`) --------
//
// Unlike pi (in-process AgentSession), opencode runs an HTTP server: the chat
// spawns `opencode serve` once per chat, talks to it via @opencode-ai/sdk's v2
// client, and adapts the client to the pi-shaped session contract createChatHub
// already speaks (subscribe/prompt/abort/messages/model/dispose). Events come
// from one SSE stream per server (client.event.subscribe()); we filter to our
// sessionID and map session.next.* events onto the pi event vocabulary the Chat
// tab's reducer renders.

/** file:// URL for @opencode-ai/sdk — same resolution ladder as the pi SDK. */
export function resolveOpencodeSdkUrl({ sdkPath, env = process.env, moduleUrl = import.meta.url } = {}) {
  const candidates = [];
  const explicit = sdkPath || env.AGENT_QA_OPENCODE_SDK;
  if (explicit) candidates.push(explicit);
  candidates.push(findNodeModulesPackageEntry(moduleUrl, OPENCODE_PKG));
  try {
    const require = createRequire(moduleUrl);
    candidates.push(require.resolve(OPENCODE_PKG));
  } catch {
    /* package may export only the ESM import condition */
  }
  for (const c of candidates) {
    const entry = toExistingEntry(c);
    if (entry) return pathToFileURL(entry).href;
  }
  throw new Error(
    `opencode SDK (${OPENCODE_PKG}) not found. Install it, or set ` +
      `AGENT_QA_OPENCODE_SDK to its dist/index.js (or package dir).`,
  );
}

export async function loadOpencodeSdk(config = {}) {
  const url = resolveOpencodeSdkUrl(config);
  const sdk = await import(url);
  return { sdk, url };
}

/**
 * Which chat backends can actually run right now. `available` means the code
 * path resolves AND any external binary the backend needs is on PATH (pi runs
 * in-process, opencode needs the `opencode` CLI for `opencode serve`).
 */
export function detectChatBackends({ env = process.env, moduleUrl = import.meta.url } = {}) {
  let pi = false;
  try {
    resolvePiSdkUrl({ env, moduleUrl });
    pi = true;
  } catch {
    pi = false;
  }
  let opencodeSdk = false;
  try {
    resolveOpencodeSdkUrl({ env, moduleUrl });
    opencodeSdk = true;
  } catch {
    opencodeSdk = false;
  }
  const opencodeBin = whichOnPath(OPENCODE_BIN, env);
  return {
    pi: { id: 'pi', available: pi, install: 'npm i -g @earendil-works/pi-coding-agent' },
    opencode: {
      id: 'opencode',
      available: opencodeSdk && !!opencodeBin,
      sdkAvailable: opencodeSdk,
      binary: opencodeBin,
      install: 'npm i -g opencode-ai',
    },
  };
}

// Map a v2 SSE event onto the pi-shaped agent events chatReducer consumes.
// Global-stream events carry their payload in `properties`; the durable
// per-session stream calls the same field `data` — take either.
// Returns null for events with no UI mapping.
export function mapOpencodeEvent(ev) {
  const data = ev && typeof ev === 'object' ? (ev.properties ?? ev.data ?? {}) : {};
  switch (ev && ev.type) {
    case 'session.next.prompt.admitted':
      return { type: 'agent_start' };
    case 'session.next.step.started':
      return { type: 'turn_start' };
    case 'session.next.step.ended':
      return { type: 'turn_end' };
    case 'session.next.text.started':
      return { type: 'message_update', assistantMessageEvent: { type: 'text_start' } };
    case 'session.next.text.delta':
      return {
        type: 'message_update',
        assistantMessageEvent: { type: 'text_delta', delta: data.delta || '' },
      };
    case 'session.next.text.ended':
      return { type: 'message_end' };
    case 'session.next.reasoning.delta':
      return {
        type: 'message_update',
        assistantMessageEvent: { type: 'thinking_delta', delta: data.delta || '' },
      };
    case 'session.next.tool.called':
      return {
        type: 'tool_execution_start',
        toolCallId: data.callID,
        toolName: data.tool,
        args: data.input,
      };
    case 'session.next.tool.progress':
      return {
        type: 'tool_execution_update',
        toolCallId: data.callID,
        partialResult: data.output ?? data.result,
      };
    case 'session.next.tool.success':
      return {
        type: 'tool_execution_end',
        toolCallId: data.callID,
        toolName: data.tool,
        isError: false,
        result: data.result ?? data.content,
      };
    case 'session.next.tool.failed':
      return {
        type: 'tool_execution_end',
        toolCallId: data.callID,
        toolName: data.tool,
        isError: true,
        result: data.error ?? data.result,
      };
    case 'session.error': {
      const err = data.error;
      const msg =
        (err && (err.message || err.errorMessage || err.name)) ||
        (typeof err === 'string' ? err : null) ||
        'agent error';
      return { type: 'error', message: msg };
    }
    case 'session.idle':
      return { type: 'agent_end' };
    default:
      return null;
  }
}

// SessionMessage projection (session.messages) → pi-shaped {role, content}
// rows the reducer's rehydrate path renders. Returns an array because one
// assistant message can carry tool results that pi models as separate rows.
export function mapOpencodeMessage(m) {
  if (!m || typeof m !== 'object') return [];
  if (m.type === 'user') {
    return [{ role: 'user', content: [{ type: 'text', text: m.text || '' }] }];
  }
  if (m.type !== 'assistant') return [];
  const content = [];
  const results = [];
  for (const part of m.content || []) {
    if (!part) continue;
    if (part.type === 'text' && part.text) {
      content.push({ type: 'text', text: part.text });
    } else if (part.type === 'reasoning' && part.text) {
      content.push({ type: 'thinking', thinking: part.text });
    } else if (part.type === 'tool') {
      const st = part.state || {};
      content.push({
        type: 'toolCall',
        id: part.id,
        name: part.name,
        arguments: st.input,
      });
      if (st.status === 'completed' || st.status === 'error') {
        const errMsg = st.error && (st.error.message || String(st.error));
        results.push({
          role: 'toolResult',
          toolCallId: part.id,
          toolName: part.name,
          isError: st.status === 'error',
          content: st.status === 'error' ? errMsg || 'tool failed' : (st.result ?? st.content ?? ''),
        });
      }
    }
  }
  return [{ role: 'assistant', content }, ...results];
}

function freePort() {
  return new Promise((resolve, reject) => {
    const srv = createNetServer();
    srv.once('error', reject);
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

// Spawn `opencode serve` ourselves rather than via the SDK's createOpencode():
// the SDK pins the process env at spawn time, while each chat needs its own
// env (AGENT_BROWSER_SESSION et al. reach the agent's bash tool through the
// server process). Resolves with { url, close } once the server reports ready.
function spawnOpencodeServer({ cwd, env = process.env, extraEnv = {}, timeoutMs = 15000, logger = () => {} } = {}) {
  return (async () => {
    const bin = whichOnPath(OPENCODE_BIN, env);
    if (!bin) {
      throw new Error(`opencode CLI not found on PATH — install with ${'npm i -g opencode-ai'}`);
    }
    const port = await freePort();
    const proc = spawn(bin, ['serve', `--hostname=127.0.0.1`, `--port=${port}`], {
      cwd,
      env: { ...env, ...extraEnv },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let output = '';
    const url = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        proc.kill();
        reject(new Error(`opencode serve did not start within ${timeoutMs}ms`));
      }, timeoutMs);
      if (typeof timer.unref === 'function') timer.unref();
      const onData = (chunk) => {
        output += chunk;
        const m = /on\s+(https?:\/\/\S+)/.exec(output);
        if (m) {
          clearTimeout(timer);
          resolve(m[1]);
        }
      };
      proc.stdout.on('data', onData);
      proc.stderr.on('data', onData);
      proc.once('exit', (code) => {
        clearTimeout(timer);
        reject(new Error(`opencode serve exited ${code}: ${output.trim()}`));
      });
      proc.once('error', (err) => {
        clearTimeout(timer);
        reject(err);
      });
    });
    logger(`opencode serve up at ${url}`);
    return {
      url,
      close() {
        try {
          proc.kill();
        } catch {
          /* already gone */
        }
      },
    };
  })();
}

// Adapt one opencode session to the pi-session contract createChatHub speaks.
function makeOpencodeSessionAdapter({ client, sessionID, models, cwd }) {
  const subscribers = new Set();
  let pump = null;
  let disposed = false;
  let streaming = false;
  let messages = [];
  let messagesFetched = false;
  let usage = null;
  let model = null;
  let onSessionEvent = () => {};

  const adapter = {
    sessionId: sessionID,
    agent: { state: {} },
    // A minimal modelRegistry shim for the hub's setModel path.
    modelRegistry: {
      find: (provider, id) =>
        (models() || []).find((m) => m.id === id && (!provider || m.provider === provider)) || null,
      getAvailable: () => models() || [],
    },
    subscribe(cb) {
      subscribers.add(cb);
      ensurePump();
      return () => subscribers.delete(cb);
    },
    async prompt(text, opts = {}) {
      const res = await client.session.prompt({
        sessionID,
        prompt: { text },
        ...(streaming || opts.streamingBehavior === 'steer' ? { delivery: 'steer' } : {}),
      });
      if (res && res.error) throw new Error(res.error.message || JSON.stringify(res.error));
      refreshMessages();
    },
    async abort() {
      try {
        await client.session.interrupt({ sessionID });
      } catch {
        /* idle interruption is a no-op server-side */
      }
    },
    async setModel(m) {
      if (!m || !m.id) throw new Error('model id is required');
      const res = await client.session.switchModel({
        sessionID,
        model: { providerID: m.provider, id: m.id },
      });
      if (res && res.error) throw new Error(res.error.message || JSON.stringify(res.error));
      model = { provider: m.provider, id: m.id, label: m.label || m.id };
    },
    setThinkingLevel() {
      /* opencode has no thinking-level control */
    },
    getAvailableThinkingLevels: () => [],
    dispose() {
      disposed = true;
      subscribers.clear();
      if (pump) {
        pump.cancelled = true;
        pump = null;
      }
    },
  };

  Object.defineProperty(adapter, 'isStreaming', { get: () => streaming });
  Object.defineProperty(adapter, 'messages', { get: () => messages });
  Object.defineProperty(adapter, 'model', { get: () => model });
  Object.defineProperty(adapter, 'usage', { get: () => usage });

  function emit(ev) {
    if (!ev) return;
    for (const cb of subscribers) {
      try {
        cb(ev);
      } catch {
        /* subscriber threw */
      }
    }
  }

  async function refreshMessages() {
    try {
      const res = await client.session.messages({ sessionID, order: 'asc' });
      const rows = (res && (res.data?.data ?? res.data)) || [];
      const out = [];
      // Sum per-message cost/token accounting the server reports on each
      // assistant message — exposed via /state for the chat's usage badge.
      let cost = 0;
      let input = 0;
      let output = 0;
      let reasoning = 0;
      for (const m of rows) {
        out.push(...mapOpencodeMessage(m));
        const info = (m && (m.info || m)) || {};
        if (info.type !== 'assistant') continue;
        cost += Number(info.cost) || 0;
        input += Number(info.tokens && info.tokens.input) || 0;
        output += Number(info.tokens && info.tokens.output) || 0;
        reasoning += Number(info.tokens && info.tokens.reasoning) || 0;
      }
      messages = out;
      usage = { cost, tokens: { input, output, reasoning, total: input + output + reasoning } };
      messagesFetched = true;
    } catch {
      /* keep last snapshot */
    }
  }

  function ensurePump() {
    if (pump || disposed) return;
    const pumpState = { cancelled: false };
    pump = pumpState;
    (async () => {
      try {
        // The GLOBAL stream (/api/event) — not session.events — carries
        // session.idle / session.status / session.error alongside every
        // session.next.* durable event, so one subscription covers the whole
        // lifecycle. Payload fields live under `properties` here.
        const sub = await client.event.subscribe();
        for await (const ev of sub.stream) {
          if (pumpState.cancelled || disposed) break;
          if (!ev || typeof ev !== 'object') continue;
          const d = ev.properties ?? ev.data ?? {};
          // Events belonging to another session on this server are noise.
          if (d.sessionID && d.sessionID !== sessionID) continue;
          if (ev.type === 'session.status') {
            if (d.status && d.status.type === 'busy') streaming = true;
            if (d.status && d.status.type === 'idle') streaming = false;
          }
          if (ev.type === 'session.idle') streaming = false;
          if (
            ev.type === 'session.next.prompt.admitted' ||
            ev.type === 'session.next.tool.called' ||
            ev.type === 'session.next.step.started'
          ) {
            streaming = true;
          }
          if (ev.type === 'message.updated' || ev.type === 'session.idle') refreshMessages();
          onSessionEvent(ev);
          emit(mapOpencodeEvent(ev));
        }
      } catch (err) {
        if (!disposed && !pumpState.cancelled) {
          emit({ type: 'error', message: `opencode event stream: ${(err && err.message) || err}` });
        }
      }
    })();
  }

  // Seed the initial model + history when the session is created.
  adapter.init = async () => {
    try {
      const res = await client.session.get({ sessionID });
      const info = res && (res.data ?? res);
      if (info && info.model) {
        model = { provider: info.model.providerID, id: info.model.id, label: info.model.id };
      }
    } catch {
      /* model picker still works via setModel */
    }
    await refreshMessages();
  };
  adapter.__onEvent = (cb) => {
    onSessionEvent = cb || (() => {});
  };
  adapter.__setStreamingForTests = (v) => {
    streaming = v;
  };
  void cwd;
  void messagesFetched;

  return adapter;
}

/**
 * Build a chat hub on the opencode v2 stack: `opencode serve` subprocess +
 * @opencode-ai/sdk client. The CLI binary must be on PATH; the SDK resolves
 * like the pi SDK (node_modules chain, AGENT_QA_OPENCODE_SDK, or sdkPath).
 * @param {object} config  Same knobs as createChatBackend plus
 *   config.bashEnv — env merged into the `opencode serve` process so the
 *   agent's bash tool sees this chat's AGENT_BROWSER_SESSION et al.
 */
export async function createOpencodeBackend(config = {}) {
  const { sdk } = await loadOpencodeSdk(config);
  if (typeof sdk.createOpencodeClient !== 'function') {
    throw new Error(`${OPENCODE_PKG} resolved but exports no createOpencodeClient (v2 SDK required)`);
  }
  const cwd = config.cwd || process.cwd();
  const bashEnv = typeof config.bashEnv === 'function' ? config.bashEnv() || {} : {};
  const server = await spawnOpencodeServer({
    cwd,
    env: config.env || process.env,
    extraEnv: bashEnv,
    logger: config.logger,
  });
  const client = sdk.createOpencodeClient({ baseUrl: server.url, directory: cwd });

  // Model catalog for the picker (client.model.list → ModelV2Info[]).
  let modelCache = null;
  const listModels = async () => {
    try {
      const res = await client.model.list({ location: { directory: cwd } });
      const rows = (res && (res.data?.data ?? res.data)) || [];
      const out = [];
      for (const m of Array.isArray(rows) ? rows : []) {
        if (!m || !m.id || !m.providerID) continue;
        out.push({ provider: m.providerID, id: m.id, label: m.name || m.id });
      }
      modelCache = out;
      return out;
    } catch {
      return modelCache || [];
    }
  };
  // Warm once so the picker isn't empty on first open; refresh failures are
  // non-fatal (opencode can also pick a default model server-side).
  await listModels();

  // The `opencode serve` child outlives any one adapter — idle teardown only
  // disposes the adapter (its event pump), not the server-side session. Keep
  // the sessionID so a recreated session RESUMES the same conversation
  // (init() re-pulls messages) instead of starting blank; `fresh` (New chat)
  // deliberately drops it and opens a new one.
  let sessionID = null;
  const createSession = async ({ fresh } = {}) => {
    let isNew = false;
    if (fresh || !sessionID) {
      const res = await client.session.create({ location: { directory: cwd } });
      const info = res && (res.data ?? res);
      sessionID = info && (info.id || info.sessionID);
      if (!sessionID) throw new Error('opencode session.create returned no session id');
      isNew = true;
    }
    const adapter = makeOpencodeSessionAdapter({
      client,
      sessionID,
      models: () => modelCache,
      cwd,
    });
    await adapter.init();
    // Same cheap-default rule the pi path applies: on a NEW session only,
    // prefer AGENT_QA_CHAT_MODEL (substring match), else the first Haiku — a
    // resumed session keeps whatever model it already runs.
    if (isNew) {
      try {
        const avail = modelCache || [];
        const want = (process.env.AGENT_QA_CHAT_MODEL || '').toLowerCase();
        const key = (m) => `${m?.provider || ''}/${m?.id || ''} ${m?.label || ''}`.toLowerCase();
        const pick = avail.find((m) => (want ? key(m).includes(want) : /haiku/.test(key(m))));
        if (pick) await adapter.setModel({ provider: pick.provider, id: pick.id, label: pick.label });
      } catch {
        /* keep the server default */
      }
    }
    return adapter;
  };

  const hub = createChatHub({
    createSession,
    listModels: () => modelCache || [],
    idleMs: config.idleMs,
    logger: config.logger,
  });
  // Closing the hub also stops the `opencode serve` child.
  const dispose = hub.dispose;
  hub.dispose = () => {
    try {
      server.close();
    } finally {
      dispose();
    }
  };
  return hub;
}

/**
 * Resolve the configured chat backend and return a ready chat hub.
 * @param {object} config
 * @param {string} [config.backend]    'pi' | 'opencode' — defaults to
 *   $AGENT_QA_CHAT_BACKEND, then 'pi' when its SDK resolves, else 'opencode'
 *   when usable. An explicit value is honored strictly (errors surface).
 * @param {string} [config.cwd]        cwd for skill/extension discovery (repo root).
 * @param {string} [config.agentDir]   global config dir (~/.pi/agent by default).
 * @param {string} [config.sdkPath]    explicit SDK path override.
 * @param {string[]} [config.tools]    tool allowlist (omit for full parity).
 * @param {number} [config.idleMs]
 * @param {(msg:string)=>void} [config.logger]
 */
export async function createChatBackend(config = {}) {
  const env = config.env || process.env;
  let backend = config.backend || env.AGENT_QA_CHAT_BACKEND || null;
  if (!backend) {
    const det = detectChatBackends({ env });
    backend = det.pi.available ? 'pi' : det.opencode.available ? 'opencode' : 'pi';
  }
  if (backend !== 'opencode' && backend !== 'pi') {
    throw new Error(`unknown chat backend "${backend}" — expected pi or opencode`);
  }
  const hub = backend === 'opencode' ? await createOpencodeBackend(config) : await createPiBackend(config);
  // Stamp the backend id into /api/chat/state so the UI can label it.
  const getState = hub.getState;
  if (typeof getState === 'function') {
    hub.getState = async () => ({ backend, ...(await getState.call(hub)) });
  }
  return hub;
}

async function createPiBackend(config = {}) {
  const { sdk } = await loadPiSdk(config);
  const { AuthStorage, ModelRegistry } = sdk;
  // One registry, shared between the session factory and the model picker.
  const authStorage = AuthStorage.create();
  const modelRegistry = ModelRegistry.create(authStorage);
  const listModels = () => {
    try {
      const all =
        typeof modelRegistry.getAvailable === 'function' ? modelRegistry.getAvailable() : [];
      return (all || []).map(modelInfo).filter(Boolean);
    } catch {
      return [];
    }
  };
  const createSession = makeSessionFactory(sdk, config, { authStorage, modelRegistry });
  return createChatHub({
    createSession,
    listModels,
    idleMs: config.idleMs,
    logger: config.logger,
  });
}

export const __internal = {
  safeStringify,
  safeClone,
  modelInfo,
  toExistingEntry,
  whichOnPath,
  resolveAgentQaSkillDirs,
  AGENT_QA_PRIMER,
  spawnOpencodeServer,
  makeOpencodeSessionAdapter,
  freePort,
};
