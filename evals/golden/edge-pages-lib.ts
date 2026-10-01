import { toRecordDraft } from "./record-translate";
import { checkVisibleEval, clickTrustedOrVisible, clickVisibleEval, fillVisibleEval, hoverVisibleEval, selectVisibleEval, trustedOrVisible } from "./visible";
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

// Golden coverage for the-internet.herokuapp.com — the canonical edge-case
// practice site. Each case records real actions against the live page, then
// replays the flushed scenario, exercising verbs the practice widgets never
// touch (dialogs, redirects, hovers, keys, downloads, dynamic content).

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const base = "https://the-internet.herokuapp.com";
// pagePath may be a path (joined to the site base) or a full URL — e.g. a
// credentialed URL for the basic_auth case.
export const edgeUrl = (path: string) =>
  path.startsWith("http://") || path.startsWith("https://") ? path : `${base}${path}`;

interface StepResult {
  name: string;
  command: string[];
  exitCode: number;
  stdout: string;
  stderr: string;
}

interface GoldenContext {
  tc: string;
  intent: string;
  runId: string;
  resultRoot: string;
  scenariosRoot: string;
  recordRoot: string;
  session: string;
  agentQa: string;
  agentBrowser: string;
  env: Record<string, string>;
  results: StepResult[];
  // Per-session mint for fillUnique: mirrors replay's scenario-scoped
  // {{vars._unique}} cache so two fields sharing one template (password +
  // confirm) observe the same record-side value.
  uniqueMint?: string;
}

export interface EdgeGolden extends GoldenContext {
  openPage(): Promise<void>;
  clickSelector(selector: string, intent: string): Promise<void>;
  clickRole(role: string, name: string, intent: string): Promise<void>;
  fillSelector(selector: string, value: string, intent: string): Promise<void>;
  fillSelectorReplayValue(selector: string, liveValue: string, replayValue: string, intent: string): Promise<void>;
  // Fill a selector with a template containing `{{vars._unique}}`: mints a
  // fresh value at record time for the live fill, and records the template
  // verbatim so replay mints a different one (uniqueness-constrained fields).
  fillUnique(selector: string, template: string, intent: string): Promise<void>;
  clearSelector(selector: string, intent: string): Promise<void>;
  selectOption(selector: string, value: string, intent: string): Promise<void>;
  checkSelector(selector: string, intent: string): Promise<void>;
  dblclickSelector(selector: string, intent: string): Promise<void>;
  hoverSelector(selector: string, intent: string): Promise<void>;
  pressKey(key: string, intent: string): Promise<void>;
  pressOn(selector: string, key: string, intent: string): Promise<void>;
  scrollToSelector(selector: string, intent: string): Promise<void>;
  scrollBottom(intent: string): Promise<void>;
  downloadBySelector(selector: string, scenarioRelPath: string, intent: string): Promise<void>;
  dragSelector(from: string, to: string, intent: string): Promise<void>;
  dragSelectorMouse(from: string, to: string, intent: string): Promise<void>;
  tabCommand(tail: string, intent: string): Promise<void>;
  reload(intent: string): Promise<void>;
  upload(selector: string, repoRelFixture: string, intent: string): Promise<void>;
  assertFileExists(scenarioRelPath: string, intent: string): Promise<void>;
  assertFileName(scenarioRelPath: string, expectedName: string, intent: string): Promise<void>;
  assertFileContent(scenarioRelPath: string, needle: string, intent: string): Promise<void>;
  dismissBySelector(selector: string, intent: string): Promise<void>;
  dialogAccept(intent: string, text?: string): Promise<void>;
  dialogDismiss(intent: string): Promise<void>;
  assertDialogText(text: string, intent: string): Promise<void>;
  assertDialogClosed(intent: string): Promise<void>;
  waitMs(ms: number, intent: string): Promise<void>;
  clickSelectorForce(selector: string, intent: string): Promise<void>;
  seedCookie(name: string, value: string, intent: string): Promise<void>;
  seedStorage(scope: "local" | "session", key: string, value: string, intent: string): Promise<void>;
  gotoUrl(url: string, intent: string): Promise<void>;
  waitSelector(selector: string, intent: string): Promise<void>;
  waitSelectorAbsent(selector: string, intent: string): Promise<void>;
  waitSelectorText(selector: string, text: string, intent: string): Promise<void>;
  waitText(text: string, intent: string): Promise<void>;
  waitLoad(state: string, intent: string): Promise<void>;
  enterFrame(selector: string, intent: string): Promise<void>;
  exitFrame(intent: string): Promise<void>;
  // `wait url` — poll resource timing until a matching request completed,
  // live at record and in the replayed scenario (`params.timeoutMs` honored).
  waitRequest(pattern: string, intent: string, timeoutMs?: number): Promise<void>;
  // `wait url` — poll resource timing until a matching request completed,
  // live at record and in the replayed scenario (`params.timeoutMs` honored).
  waitRequest(pattern: string, intent: string, timeoutMs?: number): Promise<void>;
  assertElementText(selector: string, expected: string, intent: string): Promise<void>;
  assertElementAttribute(selector: string, attribute: string, predicate: string, expected: string, intent: string): Promise<void>;
  assertElementCount(selector: string, count: number, intent: string): Promise<void>;
  assertElementCount(selector: string, predicate: string, count: number, intent: string): Promise<void>;
  assertElementAbsent(selector: string, intent: string): Promise<void>;
  assertElementPresent(selector: string, intent: string): Promise<void>;
  assertUrlContains(fragment: string, intent: string): Promise<void>;
  assertConsole(matcher: true | { type?: string; text?: string }, predicate: string, value: string | undefined, intent: string): Promise<void>;
  assertPageError(matcher: true | { text?: string; url?: string }, predicate: string, value: string | undefined, intent: string): Promise<void>;
  assertNetworkStatus(matcher: Record<string, unknown>, predicate: string, value: string | undefined, intent: string): Promise<void>;
  assertNetworkFired(matcher: Record<string, unknown>, mustFire: boolean, intent: string): Promise<void>;
  assertNetworkJson(matcher: Record<string, unknown>, path: string, predicate: string, value: unknown, intent: string): Promise<void>;
  assertNetworkSilent(matcher: Record<string, unknown>, intent: string): Promise<void>;
  assertCookie(name: string, expectPresent: boolean, intent: string): Promise<void>;
  assertStorage(keyOrMatcher: string | { key: string; scope?: string }, expectPresent: boolean, intent: string): Promise<void>;
  assertStyle(selector: string, cssProperty: string, expected: string, intent: string): Promise<void>;
  a11yAudit(matcher: true | Record<string, unknown>, predicate: string, value: number | undefined, intent: string): Promise<void>;
  frameInto(selector: string, intent: string): Promise<void>;
  frameMain(intent: string): Promise<void>;
  fillInFrame(frameSelector: string, selector: string, value: string, intent: string): Promise<void>;
  // Role-locator drives resolve through the a11y snapshot refs, which
  // pierce open shadow roots where a plain css selector cannot.
  typeRole(role: string, name: string, value: string, intent: string): Promise<void>;
  clickRoleLocator(role: string, name: string, intent: string): Promise<void>;
  assertRoleAttribute(role: string, name: string, attribute: string, expected: string, intent: string): Promise<void>;
}

function createContext(tc: string, intent: string, keepDialogs: boolean, label = "edge"): GoldenContext {
  const runId = `golden-${label}-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-${label}-${tc}-${Math.random().toString(16).slice(2, 8)}`;
  const agentQa = existsSync(resolve(repoRoot, "cli/target/debug/agent-qa"))
    ? resolve(repoRoot, "cli/target/debug/agent-qa")
    : "agent-qa";
  const agentBrowser = process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN || "agent-browser";

  mkdirSync(scenariosRoot, { recursive: true });
  mkdirSync(recordRoot, { recursive: true });

  return {
    tc,
    intent,
    runId,
    resultRoot,
    scenariosRoot,
    recordRoot,
    session,
    agentQa,
    agentBrowser,
    env: {
      ...(process.env as Record<string, string>),
      AGENT_QA_SCENARIOS_DIR: scenariosRoot,
      AGENT_QA_RECORD_DIR: recordRoot,
      AGENT_QA_REPO_ROOT: repoRoot,
      AGENT_QA_RECORD_SKIP_SIDECARS: "1",
      AGENT_QA_AGENT_BROWSER_TIMEOUT_MS: process.env.AGENT_QA_AGENT_BROWSER_TIMEOUT_MS || "10000",
      // Dialog cases keep native dialogs pending so recorded `dialog` verbs
      // observe them; pages that fire an alert on load (e.g. /download's
      // XSS-test filenames) instead want the daemon's auto-accept so `open`
      // isn't blocked.
      ...(keepDialogs ? { AGENT_BROWSER_NO_AUTO_DIALOG: "1" } : {}),
      NO_COLOR: "1",
    },
    results: [],
  };
}

async function run(ctx: GoldenContext, name: string, command: string[]): Promise<string> {
  console.error(`[golden] ${name}`);
  const proc = Bun.spawn(command, {
    cwd: repoRoot,
    env: ctx.env,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  ctx.results.push({ name, command, exitCode, stdout, stderr });
  if (exitCode !== 0) {
    throw new Error(`${name} failed (${exitCode})\n${command.join(" ")}\n${stdout}\n${stderr}`);
  }
  return stdout;
}

async function record(ctx: GoldenContext, kind: string, payload: unknown): Promise<void> {
  const [draftKind, draft] = toRecordDraft(kind, payload);
  await run(ctx, `record ${kind}`, [ctx.agentQa, "record-step", draftKind, JSON.stringify(draft)]);
}

// A synthetic click on an element whose centre isn't hit-testable (below
// the fold, zero-size, or covered) dispatches to nothing while the driver
// still reports success — the recorded scenario then replays a click that
// never landed. Scroll it into view first so the scenario stays faithful;
// the scroll is recorded as its own step.
async function ensureHittable(ctx: GoldenContext, selector: string, stepIntent: string): Promise<void> {
  const expr = `(()=>{const el=document.querySelector(${JSON.stringify(selector)});if(!el)return'missing';const r=el.getBoundingClientRect();if(r.width===0||r.height===0)return'empty';const x=r.x+r.width/2,y=r.y+r.height/2;if(x<0||y<0||x>window.innerWidth||y>window.innerHeight)return'offscreen';const t=document.elementFromPoint(x,y);if(!t)return'uncovered';return t===el||el.contains(t)||t.contains(el)?'ok':'blocked'})()`;
  const hit = (await run(ctx, `hit-test ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "eval", expr])).replace(/"/g, "").trim();
  if (hit === "ok") return;
  await run(ctx, `scroll ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "scrollintoview", selector]);
  await record(ctx, "action", { method: "scrollToBySelector", args: [selector], intent: `bring ${stepIntent} into view` });
}

// Role+name → snapshot ref. The a11y snapshot's refs map pierces open
// shadow roots; agent-browser's `fill`/`click` accept `@<ref>` targets.
async function resolveSnapshotRef(ctx: GoldenContext, role: string, name: string): Promise<string> {
  const out = await run(ctx, `resolve ${role} "${name}"`, [
    ctx.agentBrowser,
    "--session",
    ctx.session,
    "--json",
    "snapshot",
    "-i",
  ]);
  const data = (JSON.parse(out).data ?? {}) as {
    refs?: Record<string, { role?: string; name?: string }>;
  };
  const want = name.toLowerCase();
  const matches = Object.entries(data.refs ?? {}).filter(
    ([, n]) => n.role === role && (n.name ?? "").toLowerCase().includes(want),
  );
  if (matches.length === 0) {
    throw new Error(`no ${role} named ${JSON.stringify(name)} in snapshot`);
  }
  if (matches.length > 1) {
    throw new Error(`${matches.length} ${role}s named ${JSON.stringify(name)} in snapshot`);
  }
  return matches[0][0];
}

export async function runEdgeGolden(
  tc: string,
  intent: string,
  pagePath: string,
  readySelector: string,
  steps: (golden: EdgeGolden) => Promise<void>,
  opts: { keepDialogs?: boolean; label?: string; flushArgs?: string[] } = {},
): Promise<void> {
  const ctx = createContext(tc, intent, opts.keepDialogs ?? false, opts.label ?? "edge");
  const pageUrl = edgeUrl(pagePath);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: EdgeGolden = {
    ...ctx,
    async openPage() {
      await run(ctx, `open ${pagePath}`, [ctx.agentBrowser, "--session", ctx.session, "open", pageUrl]);
      await run(ctx, "wait page content", [ctx.agentBrowser, "--session", ctx.session, "wait", readySelector]);
      await record(ctx, "navigation", { route: pageUrl });
      await record(ctx, "wait", { condition: { kind: "selector", selector: readySelector }, intent: "page rendered" });
    },
    async clickSelector(selector, stepIntent) {
      await ensureHittable(ctx, selector, stepIntent);
      await clickTrustedOrVisible(ctx, (n, c) => run(ctx, n, c), selector);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async clickRole(role, name, stepIntent) {
      await run(ctx, `click ${role} '${name}'`, [ctx.agentBrowser, "--session", ctx.session, "find", "role", role, "click", "--name", name]);
      await record(ctx, "action", { method: "clickRole", args: [role, name], intent: stepIntent });
    },
    async fillSelector(selector, value, stepIntent) {
      await trustedOrVisible(ctx, (n, c) => run(ctx, n, c), `fill ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "fill", selector, value], fillVisibleEval(selector, value));
      await record(ctx, "action", { method: "fillBySelector", args: [selector, value], intent: stepIntent });
    },
    async fillSelectorReplayValue(selector, liveValue, replayValue, stepIntent) {
      // Fill the live field with the literal but record a template (e.g.
      // {{vars._unique}}) — signups and other unique-value flows replay
      // with a fresh value instead of the recorded literal.
      await run(ctx, `fill ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "fill", selector, liveValue]);
      await record(ctx, "action", { method: "fillBySelector", args: [selector, replayValue], intent: stepIntent });
    },
    async fillUnique(selector, template, stepIntent) {
      ctx.uniqueMint ??= Array.from(crypto.getRandomValues(new Uint8Array(4)), (b) =>
        b.toString(16).padStart(2, "0"),
      ).join("");
      const resolved = template.replaceAll("{{vars._unique}}", ctx.uniqueMint);
      await run(ctx, `fill ${selector} (unique)`, [ctx.agentBrowser, "--session", ctx.session, "fill", selector, resolved]);
      await record(ctx, "action", { method: "fillBySelector", args: [selector, template], intent: stepIntent });
    },
    async clearSelector(selector, stepIntent) {
      // Keystroke clearing: focus + select-all + Backspace. `fill <sel> ""`
      // leaves framework-controlled inputs (React) stale — the DOM value is
      // empty but no onChange fires, so the app keeps behaving as filtered.
      await run(ctx, `focus ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "focus", selector]);
      await run(ctx, `press ctrl+a`, [ctx.agentBrowser, "--session", ctx.session, "press", "Control+a"]);
      await run(ctx, `press backspace`, [ctx.agentBrowser, "--session", ctx.session, "press", "Backspace"]);
      await record(ctx, "action", { method: "clearBySelector", args: [selector], intent: stepIntent });
    },
    async selectOption(selector, value, stepIntent) {
      await trustedOrVisible(ctx, (n, c) => run(ctx, n, c), `select ${value}`, [ctx.agentBrowser, "--session", ctx.session, "select", selector, value], selectVisibleEval(selector, Array.isArray(value) ? value : [value]));
      await record(ctx, "action", { method: "selectBySelector", args: [selector, value], intent: stepIntent });
    },
    async checkSelector(selector, stepIntent) {
      await ensureHittable(ctx, selector, stepIntent);
      await trustedOrVisible(ctx, (n, c) => run(ctx, n, c), `check ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "check", selector], checkVisibleEval(selector));
      await record(ctx, "action", { method: "checkBySelector", args: [selector], intent: stepIntent });
    },
    async hoverSelector(selector, stepIntent) {
      await trustedOrVisible(ctx, (n, c) => run(ctx, n, c), `hover ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "hover", selector], hoverVisibleEval(selector));
      await record(ctx, "action", { method: "hoverBySelector", args: [selector], intent: stepIntent });
    },
    async dblclickSelector(selector, stepIntent) {
      await run(ctx, `dblclick ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "dblclick", selector]);
      await record(ctx, "action", { method: "dblclickBySelector", args: [selector], intent: stepIntent });
    },
    async pressKey(key, stepIntent) {
      await run(ctx, `press ${key}`, [ctx.agentBrowser, "--session", ctx.session, "press", key]);
      await record(ctx, "action", { method: "pressKey", args: [key], intent: stepIntent });
    },
    async pressOn(selector, key, stepIntent) {
      await run(ctx, `focus ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "focus", selector]);
      await run(ctx, `key ${key}`, [ctx.agentBrowser, "--session", ctx.session, "press", key]);
      await record(ctx, "action", { method: "pressSelector", args: [selector, key], intent: stepIntent });
    },
    async scrollToSelector(selector, stepIntent) {
      await run(ctx, `scroll ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "scrollintoview", selector]);
      await record(ctx, "action", { method: "scrollToBySelector", args: [selector], intent: stepIntent });
    },
    async dismissBySelector(selector, stepIntent) {
      await run(ctx, `dismiss ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "eval", `(function(){document.querySelectorAll(${JSON.stringify(selector)}).forEach(function(e){e.remove()});return 1})()`]);
      await record(ctx, "action", { method: "dismissBySelector", args: [selector], intent: stepIntent });
    },
    async scrollBottom(stepIntent) {
      await run(ctx, "scroll to bottom", [ctx.agentBrowser, "--session", ctx.session, "eval", "(() => { window.scrollTo(0, document.body.scrollHeight); })()"]);
      await record(ctx, "action", { method: "scrollTop", args: [999999], intent: stepIntent });
    },
    async upload(selector, repoRelFixture, stepIntent) {
      const abs = resolve(repoRoot, repoRelFixture);
      if (!existsSync(abs)) throw new Error(`upload fixture missing: ${repoRelFixture}`);
      await run(ctx, `upload ${repoRelFixture}`, [ctx.agentBrowser, "--session", ctx.session, "upload", selector, abs]);
      await record(ctx, "action", { method: "uploadBySelector", args: [selector, repoRelFixture], intent: stepIntent });
    },
    async downloadBySelector(selector, scenarioRelPath, stepIntent) {
      const abs = resolve(ctx.resultRoot, "live-downloads", scenarioRelPath);
      mkdirSync(dirname(abs), { recursive: true });
      await run(ctx, `download ${scenarioRelPath}`, [ctx.agentBrowser, "--session", ctx.session, "download", selector, abs]);
      await record(ctx, "action", { method: "downloadBySelector", args: [selector, scenarioRelPath], intent: stepIntent });
    },
    async assertFileExists(scenarioRelPath, stepIntent) {
      await record(ctx, "assert", { kind: "fileExists", args: [scenarioRelPath], intent: stepIntent });
    },
    async assertFileName(scenarioRelPath, expectedName, stepIntent) {
      await record(ctx, "assert", { kind: "fileName", args: [scenarioRelPath, expectedName], intent: stepIntent });
    },
    async assertFileContent(scenarioRelPath, needle, stepIntent) {
      await record(ctx, "assert", { kind: "fileContent", args: [scenarioRelPath, needle], intent: stepIntent });
    },
    async dialogAccept(stepIntent, text) {
      const args = [ctx.agentBrowser, "--session", ctx.session, "dialog", "accept"];
      if (text !== undefined) args.push(text);
      await run(ctx, `dialog accept ${stepIntent}`, args);
      await record(ctx, "action", { method: "dialogAccept", args: text === undefined ? [] : [text], intent: stepIntent });
    },
    async dialogDismiss(stepIntent) {
      await run(ctx, `dialog dismiss ${stepIntent}`, [ctx.agentBrowser, "--session", ctx.session, "dialog", "dismiss"]);
      await record(ctx, "action", { method: "dialogDismiss", args: [], intent: stepIntent });
    },
    async assertDialogText(text, stepIntent) {
      // Alerts fired from async handlers (XHR `.then`, timers) surface a
      // beat after the triggering action returns, so poll like the
      // selector waits do instead of racing them.
      const deadline = Date.now() + 8000;
      let last = "";
      while (Date.now() < deadline) {
        const out = await run(ctx, `dialog status ${stepIntent}`, [
          ctx.agentBrowser, "--session", ctx.session, "--json", "dialog", "status",
        ]);
        const data = (JSON.parse(out).data ?? {}) as { hasDialog?: boolean; message?: string };
        if (data.hasDialog && (data.message ?? "").includes(text)) {
          await record(ctx, "assert", { kind: "dialogText", args: [text], intent: stepIntent });
          return;
        }
        last = data.hasDialog ? `dialog ${JSON.stringify(data.message)}` : "no dialog open";
        await new Promise((r) => setTimeout(r, 250));
      }
      throw new Error(`${stepIntent}: expected dialog containing ${JSON.stringify(text)}; last status: ${last}`);
    },
    async assertDialogClosed(stepIntent) {
      await record(ctx, "assert", { kind: "dialogClosed", args: [], intent: stepIntent });
    },
    async gotoUrl(url, stepIntent) {
      await run(ctx, `open ${url}`, [ctx.agentBrowser, "--session", ctx.session, "open", url]);
      await record(ctx, "action", { method: "navigate", args: [url], intent: stepIntent });
    },
    async seedCookie(name, value, stepIntent) {
      // do/state seeds via document.cookie — drive the live browser the
      // same way so the recorded step replays what the run observed.
      await run(ctx, `seedCookie ${name}`, [ctx.agentBrowser, "--session", ctx.session, "eval", `document.cookie=${JSON.stringify(`${name}=${value}; path=/`)}`]);
      await record(ctx, "action", { method: "seedState", args: [{ cookies: [{ name, value, path: "/" }] }], intent: stepIntent });
    },
    async clickSelectorForce(selector, stepIntent) {
      // Live-drive via el.click() — replay already activates selectors with
      // a native DOM click, so this keeps record/replay identical while
      // dodging agent-browser's live covered-element refusal (e.g. a link
      // whose click point sits under a still-animating drawer header).
      await run(ctx, `jsclick ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "eval", clickVisibleEval(selector)]);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async waitMs(ms, stepIntent) {
      await Bun.sleep(ms);
      await record(ctx, "wait", { condition: { kind: "duration", ms }, intent: stepIntent });
    },
    async enterFrame(selector, stepIntent) {
      await run(ctx, `frame ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "frame", selector]);
      await record(ctx, "action", { method: "enterFrame", args: [selector], intent: stepIntent });
    },
    async exitFrame(stepIntent) {
      await run(ctx, "frame main", [ctx.agentBrowser, "--session", ctx.session, "frame", "main"]);
      await record(ctx, "action", { method: "exitFrame", args: [], intent: stepIntent });
    },
    async waitSelector(selector, stepIntent) {
      await run(ctx, `wait ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "wait", selector]);
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async waitSelectorAbsent(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorAbsent", selector }, intent: stepIntent });
    },
    async waitSelectorText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async waitLoad(state, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "loadState", state }, intent: stepIntent });
    },
    async waitRequest(pattern, stepIntent, timeoutMs = 10_000) {
      // Record-time: poll the page's resource timing until a matching entry
      // completes, so steps that depend on the request's side effects (e.g.
      // an alert fired on ajax success) can observe them live.
      const deadline = Date.now() + timeoutMs;
      const glob = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&").replaceAll("*", ".*");
      const re = new RegExp(glob);
      for (;;) {
        const probe = await run(ctx, `poll ${pattern}`, [
          ctx.agentBrowser, "--session", ctx.session, "--json", "eval",
          `JSON.stringify(performance.getEntriesByType('resource').map(e=>e.name))`,
        ]);
        try {
          const names = JSON.parse(JSON.parse(probe).data ?? "[]") as string[];
          if (names.some((n) => re.test(n))) break;
        } catch { /* fall through to deadline check */ }
        if (Date.now() > deadline) {
          throw new Error(`${stepIntent}: no request matched ${pattern} within ${timeoutMs}ms`);
        }
        await Bun.sleep(400);
      }
      await record(ctx, "wait", {
        condition: { kind: "networkRequest", pattern, timeoutMs },
        intent: stepIntent,
      });
    },
    async assertElementText(selector, expected, stepIntent) {
      await record(ctx, "assert", { kind: "elementText", args: [selector, expected], intent: stepIntent });
    },
    async assertElementAttribute(selector, attribute, predicate, expected, stepIntent) {
      await record(ctx, "assert", { kind: "elementAttribute", args: [selector, attribute, predicate, expected], intent: stepIntent });
    },
    async assertElementCount(selector, predicateOrCount, countOrIntent, maybeIntent) {
      // Both call shapes: (selector, count, intent) and
      // (selector, predicate, count, intent) — the arg vector keeps the
      // discriminator so record-translate sees which one was used.
      const args = maybeIntent === undefined
        ? [selector, predicateOrCount]
        : [selector, predicateOrCount, countOrIntent];
      const intent = maybeIntent === undefined ? countOrIntent : maybeIntent;
      await record(ctx, "assert", { kind: "elementCount", args, intent });
    },
    async assertElementAbsent(selector, stepIntent) {
      await record(ctx, "assert", { kind: "elementAbsent", args: [selector], intent: stepIntent });
    },
    async dragSelector(from, to, stepIntent) {
      // agent-browser drag performs the gesture via trusted mouse input —
      // the same path do/drag takes at replay — so mouse-tracking widgets
      // (jQuery UI droppable, react-dnd, resizables) respond in both passes.
      await run(ctx, `drag ${from} → ${to}`, [
        ctx.agentBrowser, "--session", ctx.session, "drag", from, to,
      ]);
      await record(ctx, "action", { method: "dragBySelector", args: [from, to], intent: stepIntent });
    },
    async dragSelectorMouse(from, to, stepIntent) {
      // do/drag replays with trusted mouse input — drive the live run the
      // same way so libraries that listen for mouse (not DragEvent)
      // sequences, like jQuery UI draggable, actually move.
      await run(ctx, `mousedrag ${from} → ${to}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const f = document.querySelector(${JSON.stringify(from)}); const t = document.querySelector(${JSON.stringify(to)}); if (!f || !t) throw new Error('drag endpoint missing'); ${""} const fb = f.getBoundingClientRect(); const tb = t.getBoundingClientRect(); const cx = fb.x + fb.width / 2; const cy = fb.y + fb.height / 2; const tx = tb.x + tb.width / 2; const ty = tb.y + tb.height / 2; f.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, clientX: cx, clientY: cy, button: 0 })); document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, cancelable: true, clientX: cx + 10, clientY: cy + 10, button: 0 })); document.dispatchEvent(new MouseEvent('mousemove', { bubbles: true, cancelable: true, clientX: tx, clientY: ty, button: 0 })); document.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, cancelable: true, clientX: tx, clientY: ty, button: 0 })); return 'done'; })()`,
      ]);
      await record(ctx, "action", { method: "dragBySelector", args: [from, to], intent: stepIntent });
    },
    async seedStorage(scope, key, value, stepIntent) {
      const store = scope === "local" ? "localStorage" : "sessionStorage";
      await run(ctx, `seedStorage ${store}.${key}`, [ctx.agentBrowser, "--session", ctx.session, "eval", `${store}.setItem(${JSON.stringify(key)}, ${JSON.stringify(value)})`]);
      await record(ctx, "action", { method: "seedState", args: [{ [store]: { [key]: value } }], intent: stepIntent });
    },
    async waitText(text, stepIntent) {
      // Text assertions resolve via the a11y snapshot, which pierces open
      // shadow roots — a css wait could never see this text.
      await record(ctx, "wait", { condition: { kind: "text", text }, intent: stepIntent });
    },
    async tabCommand(tail, stepIntent) {
      await run(ctx, `tab ${tail}`, [ctx.agentBrowser, "--session", ctx.session, "tab", ...tail.split(" ")]);
      await record(ctx, "action", { method: "tabCommand", args: [tail], intent: stepIntent });
    },
    async assertElementPresent(selector, stepIntent) {
      await record(ctx, "assert", { kind: "elementPresent", args: [selector], intent: stepIntent });
    },
    async assertConsole(matcher, predicate, value, stepIntent) {
      await record(ctx, "assert", {
        kind: "consoleMessage",
        args: [matcher, predicate, value],
        intent: stepIntent,
      });
    },
    async assertPageError(matcher, predicate, value, stepIntent) {
      await record(ctx, "assert", {
        kind: "pageError",
        args: [matcher, predicate, value],
        intent: stepIntent,
      });
    },
    async reload(stepIntent) {
      await run(ctx, "reload", [ctx.agentBrowser, "--session", ctx.session, "reload"]);
      await record(ctx, "action", { method: "reloadPage", args: [], intent: stepIntent });
    },
    async assertCookie(name, expectPresent, stepIntent) {
      await record(ctx, "assert", {
        kind: "cookiePresent",
        args: [name, expectPresent],
        intent: stepIntent,
      });
    },
    async assertStorage(keyOrMatcher, expectPresent, stepIntent) {
      await record(ctx, "assert", {
        kind: "storagePresent",
        args: [keyOrMatcher, expectPresent],
        intent: stepIntent,
      });
    },
    async assertNetworkStatus(matcher, predicate, value, stepIntent) {
      await record(ctx, "assert", {
        kind: "networkStatus",
        args: [matcher, predicate, value],
        intent: stepIntent,
      });
    },
    async assertNetworkFired(matcher, mustFire, stepIntent) {
      await record(ctx, "assert", {
        kind: "networkFired",
        args: [matcher, mustFire],
        intent: stepIntent,
      });
    },
    async assertNetworkJson(matcher, path, predicate, value, stepIntent) {
      await record(ctx, "assert", {
        kind: "networkJson",
        args: [matcher, path, predicate, value],
        intent: stepIntent,
      });
    },
    async assertNetworkSilent(matcher, stepIntent) {
      await record(ctx, "assert", {
        kind: "networkFired",
        args: [matcher, false],
        intent: stepIntent,
      });
    },
    async assertStyle(selector, cssProperty, expected, stepIntent) {
      await record(ctx, "assert", { kind: "elementAttribute", args: [selector, `style:${cssProperty}`, "equals", expected], intent: stepIntent });
    },
    async a11yAudit(matcher, predicate, value, stepIntent) {
      await record(ctx, "assert", {
        kind: "a11yViolations",
        args: [matcher, predicate, value],
        intent: stepIntent,
      });
    },
    async typeRole(role, name, value, stepIntent) {
      const ref = await resolveSnapshotRef(ctx, role, name);
      await run(ctx, `fill ${role} "${name}"`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "fill",
        `@${ref}`,
        value,
      ]);
      await record(ctx, "action", {
        method: "typeByRole",
        args: [role, name, value],
        intent: stepIntent,
      });
    },
    async clickRoleLocator(role, name, stepIntent) {
      const ref = await resolveSnapshotRef(ctx, role, name);
      await run(ctx, `click ${role} "${name}"`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "click",
        `@${ref}`,
      ]);
      await record(ctx, "action", {
        method: "clickRole",
        args: [role, name],
        intent: stepIntent,
      });
    },
    async assertRoleAttribute(role, name, attribute, expected, stepIntent) {
      await record(ctx, "assert", {
        kind: "roleAttribute",
        args: [role, name, attribute, expected],
        intent: stepIntent,
      });
    },
    async assertUrlContains(fragment, stepIntent) {
      await record(ctx, "assert", { kind: "url", args: [fragment], intent: stepIntent });
    },
    async frameInto(selector, stepIntent) {
      await run(ctx, `frame ready ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "wait", selector]);
      await record(ctx, "action", { method: "frame", args: [selector], intent: stepIntent });
    },
    async frameMain(stepIntent) {
      await record(ctx, "action", { method: "frame", args: [], intent: stepIntent });
    },
    async fillInFrame(frameSelector, selector, value, stepIntent) {
      const expr = `(function(){var f=document.querySelector(${JSON.stringify(frameSelector)});if(!f||!f.contentDocument)return 'no frame';var el=f.contentDocument.querySelector(${JSON.stringify(selector)});if(!el)return 'no input';el.value=${JSON.stringify(value)};el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));return 'ok'})()`;
      const out = await run(ctx, `fill ${selector} in frame`, [ctx.agentBrowser, "--session", ctx.session, "eval", expr]);
      if (!out.includes("ok")) {
        throw new Error(`frame fill failed: ${out}`);
      }
      await record(ctx, "action", { method: "fillBySelector", args: [selector, value], intent: stepIntent });
    },
  };

  try {
    const start = await run(ctx, "start", [ctx.agentQa, "start", intent, "--session", ctx.session]);
    sid = start.match(/started sid=(\S+)/)?.[1] || "";
    await steps(golden);
    await run(ctx, "verify", [ctx.agentQa, "verify"]);
    await run(ctx, "flush", [ctx.agentQa, "flush", ...(opts.flushArgs ?? [])]);
    await run(ctx, "check", [ctx.agentQa, "scenario", "check", resolve(ctx.scenariosRoot, sid, "scenario.json")]);
    await run(ctx, "replay", [ctx.agentQa, "replay", sid, "--session", `${ctx.session}-replay`]);
    pass = true;
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  const report = {
    pass,
    runId: ctx.runId,
    sid,
    resultRoot: ctx.resultRoot,
    scenariosRoot: ctx.scenariosRoot,
    recordRoot: ctx.recordRoot,
    session: ctx.session,
    error,
    results: ctx.results,
  };
  writeFileSync(resolve(ctx.resultRoot, "golden-report.json"), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  process.exit(pass ? 0 : 1);
}

/// Structural goldens for authored-only constructs (group/loop/useTemplate)
/// that the record→flush path can't produce: writes the scenario JSON
/// directly and replays it.
export async function runAuthoredGolden(
  tc: string,
  intent: string,
  scenario: { steps: unknown[]; templates?: Record<string, unknown>; inputs?: Record<string, unknown> },
  opts: { label?: string } = {},
): Promise<void> {
  const ctx = createContext(tc, intent, false, opts.label ?? "struct");
  const sid = `s-${tc}-authored`;
  let pass = false;
  let error = "";

  const doc = {
    schema: "scenario/2",
    id: sid,
    intent,
    env: { open: [{ kind: "fresh" }] },
    producedBy: { producer: "llm-author", producedAt: new Date().toISOString() },
    ...(scenario.inputs ? { inputs: scenario.inputs } : {}),
    ...(scenario.templates ? { templates: scenario.templates } : {}),
    steps: scenario.steps,
  };
  mkdirSync(resolve(ctx.scenariosRoot, sid), { recursive: true });
  writeFileSync(resolve(ctx.scenariosRoot, sid, "scenario.json"), JSON.stringify(doc, null, 2));

  try {
    await run(ctx, "check", [ctx.agentQa, "scenario", "check", resolve(ctx.scenariosRoot, sid, "scenario.json")]);
    await run(ctx, "replay", [ctx.agentQa, "replay", sid, "--session", `${ctx.session}-replay`]);
    pass = true;
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  const report = {
    pass,
    runId: ctx.runId,
    sid,
    resultRoot: ctx.resultRoot,
    scenariosRoot: ctx.scenariosRoot,
    recordRoot: ctx.recordRoot,
    session: ctx.session,
    error,
    results: ctx.results,
  };
  writeFileSync(resolve(ctx.resultRoot, "golden-report.json"), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  process.exit(pass ? 0 : 1);
}
