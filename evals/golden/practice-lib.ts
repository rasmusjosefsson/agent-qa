import { toRecordDraft } from "./record-translate";
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

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

export interface PracticeGolden extends GoldenContext {
  openPage(): Promise<void>;
  fill(selector: string, value: string, intent: string): Promise<void>;
  typeText(selector: string, text: string, intent: string): Promise<void>;
  clickSelector(selector: string, intent: string): Promise<void>;
  checkSelector(selector: string, intent: string): Promise<void>;
  selectOption(selector: string, value: string | string[], intent: string): Promise<void>;
  clearSelector(selector: string, intent: string): Promise<void>;
  focusSelector(selector: string, intent: string): Promise<void>;
  hoverSelector(selector: string, intent: string): Promise<void>;
  pressKey(key: string, intent: string): Promise<void>;
  pressOn(selector: string, key: string, intent: string): Promise<void>;
  dblclickSelector(selector: string, intent: string): Promise<void>;
  scrollToSelector(selector: string, intent: string): Promise<void>;
  reloadPage(intent: string): Promise<void>;
  tabAction(subcommand: string, intent: string): Promise<void>;
  clickNthOption(listboxSelector: string, nth: number, intent: string): Promise<void>;
  waitSelectorText(selector: string, text: string, intent: string): Promise<void>;
  waitSelectorVisible(selector: string, intent: string): Promise<void>;
  assertElementAttribute(selector: string, attribute: string, expected: string, intent: string): Promise<void>;
  assertElementAttributeMatch(selector: string, attribute: string, predicate: string, expected: string, intent: string): Promise<void>;
  assertUrlContains(fragment: string, intent: string): Promise<void>;
  assertElementText(selector: string, expected: string, intent: string): Promise<void>;
  assertElementAbsent(selector: string, intent: string): Promise<void>;
}

function createContext(suite: string, tc: string, intent: string): GoldenContext {
  const runId = `golden-${suite}-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-${suite}-${tc}-${Math.random().toString(16).slice(2, 8)}`;
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

export async function runPracticeGolden(
  suite: string,
  tc: string,
  intent: string,
  pageUrl: string,
  readySelector: string,
  steps: (golden: PracticeGolden) => Promise<void>,
): Promise<void> {
  const ctx = createContext(suite, tc, intent);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: PracticeGolden = {
    ...ctx,
    async openPage() {
      await run(ctx, `open ${suite}`, [ctx.agentBrowser, "--session", ctx.session, "open", pageUrl]);
      // Practice content hydrates client-side — wait for a page-specific anchor.
      await run(ctx, "wait practice content", [ctx.agentBrowser, "--session", ctx.session, "wait", readySelector]);
      await record(ctx, "navigation", { route: pageUrl });
      await record(ctx, "wait", { condition: { kind: "selector", selector: readySelector }, intent: `${suite} page rendered` });
    },
    async fill(selector, value, stepIntent) {
      await run(ctx, `fill ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "fill", selector, value]);
      await record(ctx, "action", { method: "fillBySelector", args: [selector, value], intent: stepIntent });
    },
    async typeText(selector, text, stepIntent) {
      await run(ctx, `type ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "type", selector, text]);
      await record(ctx, "action", { method: "fillBySelector", args: [selector, text], intent: stepIntent });
    },
    async clickSelector(selector, stepIntent) {
      await run(ctx, `click ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "click", selector]);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async checkSelector(selector, stepIntent) {
      await run(ctx, `check ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "check", selector]);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async selectOption(selector, value, stepIntent) {
      const values = Array.isArray(value) ? value : [value];
      await run(ctx, `select ${values}`, [ctx.agentBrowser, "--session", ctx.session, "select", selector, ...values]);
      await record(ctx, "action", { method: "selectBySelector", args: [selector, value], intent: stepIntent });
    },
    async clearSelector(selector, stepIntent) {
      await run(ctx, `clear ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "fill", selector, ""]);
      await record(ctx, "action", { method: "clearBySelector", args: [selector], intent: stepIntent });
    },
    async focusSelector(selector, stepIntent) {
      await run(ctx, `focus ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "focus", selector]);
      await record(ctx, "action", { method: "focusBySelector", args: [selector], intent: stepIntent });
    },
    async hoverSelector(selector, stepIntent) {
      await run(ctx, `hover ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "hover", selector]);
      await record(ctx, "action", { method: "hoverBySelector", args: [selector], intent: stepIntent });
    },
    async pressKey(key, stepIntent) {
      await run(ctx, `press ${key}`, [ctx.agentBrowser, "--session", ctx.session, "press", key]);
      await record(ctx, "action", { method: "pressKey", args: [key], intent: stepIntent });
    },
    async pressOn(selector, key, stepIntent) {
      await run(ctx, `press ${key} on ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "focus", selector]);
      await run(ctx, `key ${key}`, [ctx.agentBrowser, "--session", ctx.session, "press", key]);
      await record(ctx, "action", { method: "pressSelector", args: [selector, key], intent: stepIntent });
    },
    async dblclickSelector(selector, stepIntent) {
      await run(ctx, `dblclick ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "dblclick", selector]);
      await record(ctx, "action", { method: "dblclickBySelector", args: [selector], intent: stepIntent });
    },
    async scrollToSelector(selector, stepIntent) {
      await run(ctx, `scroll ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "scrollintoview", selector]);
      await record(ctx, "action", { method: "scrollToBySelector", args: [selector], intent: stepIntent });
    },
    async reloadPage(stepIntent) {
      await run(ctx, "reload", [ctx.agentBrowser, "--session", ctx.session, "reload"]);
      await record(ctx, "action", { method: "reloadPage", args: [], intent: stepIntent });
    },
    async tabAction(subcommand, stepIntent) {
      const parts = subcommand.split(/\s+/);
      await run(ctx, `tab ${subcommand}`, [ctx.agentBrowser, "--session", ctx.session, "tab", ...parts]);
      await record(ctx, "action", { method: "tabCommand", args: [subcommand], intent: stepIntent });
    },
    async clickNthOption(listboxSelector, nth, stepIntent) {
      const sel = `${listboxSelector} [role="option"]:nth-child(${nth})`;
      await run(ctx, `click ${sel}`, [ctx.agentBrowser, "--session", ctx.session, "click", sel]);
      await record(ctx, "action", { method: "clickNthOption", args: [listboxSelector, nth], intent: stepIntent });
    },
    async waitSelectorText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async waitSelectorVisible(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async assertElementAttribute(selector, attribute, expected, stepIntent) {
      await record(ctx, "assert", { kind: "elementAttribute", args: [selector, attribute, "equals", expected], intent: stepIntent });
    },
    async assertElementAttributeMatch(selector, attribute, predicate, expected, stepIntent) {
      await record(ctx, "assert", { kind: "elementAttribute", args: [selector, attribute, predicate, expected], intent: stepIntent });
    },
    async assertUrlContains(fragment, stepIntent) {
      await record(ctx, "assert", { kind: "url", args: [fragment], intent: stepIntent });
    },
    async assertElementText(selector, expected, stepIntent) {
      await record(ctx, "assert", { kind: "elementText", args: [selector, expected], intent: stepIntent });
    },
    async assertElementAbsent(selector, stepIntent) {
      await record(ctx, "assert", { kind: "elementAbsent", args: [selector], intent: stepIntent });
    },
  };

  try {
    const start = await run(ctx, "start", [ctx.agentQa, "start", intent, "--session", ctx.session]);
    sid = start.match(/started sid=(\S+)/)?.[1] || "";
    await steps(golden);
    await run(ctx, "verify", [ctx.agentQa, "verify"]);
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
