import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
export const evalsRoot = resolve(__dirname, "..");
export const repoRoot = resolve(evalsRoot, "..");
export const bankUrl = "https://qaplayground.com/bank";

export interface StepResult {
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

export interface BankGolden extends GoldenContext {
  start(): Promise<void>;
  openBank(): Promise<void>;
  fill(selector: string, value: string, intent: string): Promise<void>;
  clickSelector(selector: string, intent: string): Promise<void>;
  pressKey(key: string, intent: string): Promise<void>;
  navigate(route: string, intent: string): Promise<void>;
  waitSelector(selector: string, intent: string): Promise<void>;
  waitSelectorAbsent(selector: string, intent: string): Promise<void>;
  waitUrl(pattern: string, intent: string): Promise<void>;
  assertPresent(role: string, name: string, intent: string): Promise<void>;
  assertAbsent(role: string, name: string, intent: string): Promise<void>;
  assertLiveSelectorText(selector: string, text: string, intent: string): Promise<void>;
  assertLiveSelectorAbsent(selector: string, intent: string): Promise<void>;
  assertLivePasswordType(type: "password" | "text", intent: string): Promise<void>;
  finish(): Promise<void>;
}

function createContext(tc: string, intent: string): GoldenContext {
  const runId = `golden-bank-login-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-bank-${tc}-${Math.random().toString(16).slice(2, 8)}`;
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
  await run(ctx, `record ${kind}`, [ctx.agentQa, "record-step", kind, JSON.stringify(payload)]);
}

export async function runBankGolden(
  tc: string,
  intent: string,
  steps: (golden: BankGolden) => Promise<void>,
): Promise<void> {
  const ctx = createContext(tc, intent);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: BankGolden = {
    ...ctx,
    async start() {
      const start = await run(ctx, "start", [ctx.agentQa, "start", intent, "--session", ctx.session]);
      sid = start.match(/started sid=(\S+)/)?.[1] || "";
    },
    async openBank() {
      await run(ctx, "open bank", [ctx.agentBrowser, "--session", ctx.session, "open", bankUrl]);
      await record(ctx, "navigation", { route: bankUrl });
    },
    async fill(selector, value, stepIntent) {
      await run(ctx, `fill ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "fill", selector, value]);
      await record(ctx, "action", { method: "fillBySelector", args: [selector, value], intent: stepIntent });
    },
    async clickSelector(selector, stepIntent) {
      await run(ctx, `click ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "click", selector]);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async pressKey(key, stepIntent) {
      await run(ctx, `press ${key}`, [ctx.agentBrowser, "--session", ctx.session, "press", key]);
      await record(ctx, "action", { method: "pressKey", args: [key], intent: stepIntent });
    },
    async navigate(route, stepIntent) {
      await run(ctx, `open ${route}`, [ctx.agentBrowser, "--session", ctx.session, "open", route]);
      await record(ctx, "action", { method: "navigate", args: [route], intent: stepIntent });
    },
    async waitSelector(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async waitSelectorAbsent(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorAbsent", selector }, intent: stepIntent });
    },
    async waitUrl(pattern, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "url", pattern }, intent: stepIntent });
    },
    async assertPresent(role, name, stepIntent) {
      await record(ctx, "assert", { kind: "present", args: [role, name], intent: stepIntent });
    },
    async assertAbsent(role, name, stepIntent) {
      await record(ctx, "assert", { kind: "absent", args: [role, name], intent: stepIntent });
    },
    async assertLiveSelectorText(selector, text, stepIntent) {
      await run(ctx, `live check ${selector}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el || !(el.textContent || "").includes(${JSON.stringify(text)})) throw new Error(${JSON.stringify(stepIntent)}); return true; })()`,
      ]);
    },
    async assertLiveSelectorAbsent(selector, stepIntent) {
      await run(ctx, `live absent ${selector}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { if (document.querySelector(${JSON.stringify(selector)})) throw new Error(${JSON.stringify(stepIntent)}); return true; })()`,
      ]);
    },
    async assertLivePasswordType(type, stepIntent) {
      await run(ctx, `live password type ${type}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const actual = document.querySelector("#password")?.getAttribute("type"); if (actual !== ${JSON.stringify(type)}) throw new Error(${JSON.stringify(stepIntent)} + ": " + actual); return true; })()`,
      ]);
    },
    async finish() {
      await run(ctx, "flush", [ctx.agentQa, "flush"]);
      await run(ctx, "verify", [ctx.agentQa, "verify", sid]);
      await run(ctx, "replay", [ctx.agentQa, "replay", sid, "--session", `${ctx.session}-replay`]);
      pass = true;
    },
  };

  try {
    await golden.start();
    await steps(golden);
    await golden.finish();
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
