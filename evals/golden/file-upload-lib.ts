import { toRecordDraft } from "./record-translate";
import { clickTrustedOrVisible } from "./visible";
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fileUploadUrl = "https://qaplayground.com/practice/file-upload";

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
}

export interface FileUploadGolden extends GoldenContext {
  fixturePath(name: string): string;
  openPage(): Promise<void>;
  openFixture(url: string, waitSelector: string, intent?: string): Promise<void>;
  upload(selector: string, fixtureName: string, intent: string): Promise<void>;
  uploadAbs(selector: string, absPath: string, scenarioValue: string, intent: string): Promise<void>;
  uploadMulti(selector: string, fixtureNames: string[], intent: string): Promise<void>;
  waitSelectorText(selector: string, text: string, intent: string): Promise<void>;
  waitSelectorVisible(selector: string, intent: string): Promise<void>;
  assertLiveCondition(expression: string, intent: string): Promise<void>;
  assertElementAttribute(selector: string, attribute: string, expected: string, intent: string, predicate?: string): Promise<void>;
  downloadBySelector(selector: string, scenarioRelPath: string, intent: string): Promise<void>;
  assertFileExists(scenarioRelPath: string, intent: string): Promise<void>;
  assertFileName(scenarioRelPath: string, expectedName: string, intent: string): Promise<void>;
  assertFileSizeGt(scenarioRelPath: string, bytes: number, intent: string): Promise<void>;
  assertFileContent(scenarioRelPath: string, needle: string, intent: string): Promise<void>;
  assertFileString(scenarioRelPath: string, predicate: string, value: string, intent: string): Promise<void>;
  assertRolePresent(role: string, name: string, intent: string): Promise<void>;
  assertRoleAbsent(role: string, name: string, intent: string): Promise<void>;
  clickSelector(selector: string, intent: string): Promise<void>;
  rightClickSelector(selector: string, intent: string): Promise<void>;
  focusBySelector(selector: string, intent: string): Promise<void>;
  pressKeyOn(selector: string, key: string, intent: string): Promise<void>;
  pressKey(key: string, intent: string): Promise<void>;
  waitText(selector: string, text: string, intent: string): Promise<void>;
  assertUrlEquals(url: string, intent: string): Promise<void>;
  assertElementVisible(selector: string, intent: string): Promise<void>;
  setViewport(width: number, height: number, intent: string): Promise<void>;
  assertNetworkFired(matcher: Record<string, unknown>, intent: string): Promise<void>;
  assertNetworkSilent(matcher: Record<string, unknown>, intent: string): Promise<void>;
  assertNetworkStatus(matcher: Record<string, unknown>, predicate: string, value: unknown, intent: string): Promise<void>;
  assertNetworkJson(matcher: Record<string, unknown>, path: string, predicate: string, value: unknown, intent: string): Promise<void>;
  dialogAccept(intent: string): Promise<void>;
  assertDialogOpen(intent: string): Promise<void>;
  assertDialogClosed(intent: string): Promise<void>;
  waitLiveSelector(selector: string, intent: string): Promise<void>;
}

function createContext(tc: string, intent: string): GoldenContext {
  const runId = `golden-file-upload-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-file-upload-${tc}-${Math.random().toString(16).slice(2, 8)}`;
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

export async function runFileUploadGolden(
  tc: string,
  intent: string,
  steps: (golden: FileUploadGolden) => Promise<void>,
): Promise<void> {
  const ctx = createContext(tc, intent);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: FileUploadGolden = {
    ...ctx,
    fixturePath(name) {
      return resolve(repoRoot, "evals/fixtures", name);
    },
    async openPage() {
      await run(ctx, "open file upload", [ctx.agentBrowser, "--session", ctx.session, "open", fileUploadUrl]);
      // Practice content hydrates client-side — wait for it before driving.
      await run(ctx, "wait practice content", [ctx.agentBrowser, "--session", ctx.session, "wait", '[data-testid="fu-single-input"]']);
      await record(ctx, "navigation", { route: fileUploadUrl });
      await record(ctx, "wait", { condition: { kind: "selector", selector: '#fu-single-input' }, intent: "upload input rendered" });
    },
    async openFixture(url, waitSelector, stepIntent = "fixture rendered") {
      await run(ctx, "open fixture", [ctx.agentBrowser, "--session", ctx.session, "open", url]);
      await run(ctx, "wait fixture", [ctx.agentBrowser, "--session", ctx.session, "wait", waitSelector]);
      await record(ctx, "navigation", { route: url });
      await record(ctx, "wait", { condition: { kind: "selector", selector: waitSelector }, intent: stepIntent });
    },
    async upload(selector, fixtureName, stepIntent) {
      const path = this.fixturePath(fixtureName);
      await run(ctx, `upload ${fixtureName}`, [ctx.agentBrowser, "--session", ctx.session, "upload", selector, path]);
      await record(ctx, "action", { method: "uploadBySelector", args: [selector, `evals/fixtures/${fixtureName}`], intent: stepIntent });
    },
    async uploadAbs(selector, absPath, scenarioValue, stepIntent) {
      await run(ctx, `upload ${scenarioValue}`, [ctx.agentBrowser, "--session", ctx.session, "upload", selector, absPath]);
      await record(ctx, "action", { method: "uploadBySelector", args: [selector, scenarioValue], intent: stepIntent });
    },
    async uploadMulti(selector, fixtureNames, stepIntent) {
      const paths = fixtureNames.map((n) => this.fixturePath(n));
      await run(ctx, `upload ${fixtureNames.join("+")}`, [ctx.agentBrowser, "--session", ctx.session, "upload", selector, ...paths]);
      await record(ctx, "action", {
        method: "uploadBySelector",
        args: [selector, fixtureNames.map((n) => `evals/fixtures/${n}`)],
        intent: stepIntent,
      });
    },
    async waitSelectorText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async waitSelectorVisible(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async assertLiveCondition(expression, stepIntent) {
      await run(ctx, `live condition ${stepIntent}`, [ctx.agentBrowser, "--session", ctx.session, "eval", expression]);
    },
    async assertElementAttribute(selector, attribute, expected, stepIntent, predicate = "equals") {
      await record(ctx, "assert", { kind: "elementAttribute", args: [selector, attribute, predicate, expected], intent: stepIntent });
    },
    async downloadBySelector(selector, scenarioRelPath, stepIntent) {
      // Replay resolves relative download destinations against the scenario
      // dir, so the recorded value stays portable; the live browser saves into
      // this run's result dir.
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
    async assertFileSizeGt(scenarioRelPath, bytes, stepIntent) {
      await record(ctx, "assert", { kind: "fileSizeGt", args: [scenarioRelPath, bytes], intent: stepIntent });
    },
    async assertFileContent(scenarioRelPath, needle, stepIntent) {
      await record(ctx, "assert", { kind: "fileContent", args: [scenarioRelPath, needle], intent: stepIntent });
    },
    async assertFileString(scenarioRelPath, predicate, value, stepIntent) {
      await record(ctx, "assert", { kind: "fileString", args: [scenarioRelPath, predicate, value], intent: stepIntent });
    },
    async assertRolePresent(role, name, stepIntent) {
      await record(ctx, "assert", { kind: "present", args: [role, name], intent: stepIntent });
    },
    async assertRoleAbsent(role, name, stepIntent) {
      await record(ctx, "assert", { kind: "absent", args: [role, name], intent: stepIntent });
    },
    async clickSelector(selector, stepIntent) {
      await clickTrustedOrVisible(ctx, (n, c) => run(ctx, n, c), selector);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async waitLiveSelector(selector, stepIntent) {
      // Live-side only: polls the live DOM; records nothing.
      await run(ctx, `wait ${selector} ${stepIntent}`, [ctx.agentBrowser, "--session", ctx.session, "wait", selector]);
    },
    async dialogAccept(stepIntent) {
      await record(ctx, "action", { method: "dialogAccept", args: [], intent: stepIntent });
    },
    async assertDialogOpen(stepIntent) {
      await record(ctx, "assert", { kind: "dialogOpen", args: [], intent: stepIntent });
    },
    async assertDialogClosed(stepIntent) {
      await record(ctx, "assert", { kind: "dialogClosed", args: [], intent: stepIntent });
    },
    async rightClickSelector(selector, stepIntent) {
      // agent-browser has no rightclick command — dispatch a contextmenu
      // directly on the live node; the recorded step is the real verb.
      // Synchronous is safe live: the daemon auto-accepts dialogs, so a
      // contextmenu→alert handler clears itself instead of deadlocking eval.
      await run(ctx, `rightclick ${selector}`, [
        ctx.agentBrowser, "--session", ctx.session, "eval",
        `document.querySelector(${JSON.stringify(selector)}).dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,cancelable:true,button:2}))`,
      ]);
      await record(ctx, "action", { method: "rightClickBySelector", args: [selector], intent: stepIntent });
    },
    async focusBySelector(selector, stepIntent) {
      await run(ctx, `focus ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "focus", selector]);
      await record(ctx, "action", { method: "focusBySelector", args: [selector], intent: stepIntent });
    },
    async pressKeyOn(selector, key, stepIntent) {
      // pressSelector focuses the locator then sends the key — the faithful
      // "press KEY on SELECTOR" replay (keyboard-operable downloads).
      await run(ctx, `press ${key} on ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "eval", `(document.querySelector(${JSON.stringify(selector)}))?.focus()`]);
      await run(ctx, `send ${key}`, [ctx.agentBrowser, "--session", ctx.session, "press", key]);
      await record(ctx, "action", { method: "pressSelector", args: [selector, key], intent: stepIntent });
    },
    async pressKey(key, stepIntent) {
      await run(ctx, `press ${key}`, [ctx.agentBrowser, "--session", ctx.session, "press", key]);
      await record(ctx, "action", { method: "pressKey", args: [key], intent: stepIntent });
    },
    async waitText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async assertUrlEquals(url, stepIntent) {
      await record(ctx, "assert", { kind: "url", args: [url, "equals"], intent: stepIntent });
    },
    async assertElementVisible(selector, stepIntent) {
      await record(ctx, "assert", { kind: "elementPresent", args: [selector], intent: stepIntent });
    },
    async setViewport(width, height, stepIntent) {
      // Record-side only — the do/viewport verb resizes the browser via CDP
      // at replay; agent-browser exposes no resize command.
      await record(ctx, "action", { method: "setViewport", args: [width, height], intent: stepIntent });
    },
    async assertNetworkFired(matcher, stepIntent) {
      await record(ctx, "assert", { kind: "networkFired", args: [matcher], intent: stepIntent });
    },
    async assertNetworkSilent(matcher, stepIntent) {
      await record(ctx, "assert", { kind: "networkFired", args: [matcher, false], intent: stepIntent });
    },
    async assertNetworkStatus(matcher, predicate, value, stepIntent) {
      await record(ctx, "assert", { kind: "networkStatus", args: [matcher, predicate, value], intent: stepIntent });
    },
    async assertNetworkJson(matcher, path, predicate, value, stepIntent) {
      await record(ctx, "assert", { kind: "networkJson", args: [matcher, path, predicate, value], intent: stepIntent });
    },
  };

  try {
    const start = await run(ctx, "start", [ctx.agentQa, "start", intent, "--session", ctx.session]);
    sid = start.match(/started sid=(\S+)/)?.[1] || "";
    await steps(golden);    await run(ctx, "verify", [ctx.agentQa, "verify"]);

    await run(ctx, "flush", [ctx.agentQa, "flush"]);
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
