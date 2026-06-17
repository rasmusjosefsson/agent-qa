import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const dynamicWaitsUrl = "https://qaplayground.com/practice/dynamic-waits";

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

export interface DynamicWaitsGolden extends GoldenContext {
  openPage(): Promise<void>;
  clickSelector(selector: string, intent: string): Promise<void>;
  domClickSelector(selector: string, intent: string): Promise<void>;
  clickText(text: string, intent: string): Promise<void>;
  waitDuration(ms: number, intent: string): Promise<void>;
  waitSelector(selector: string, intent: string): Promise<void>;
  waitSelectorAbsent(selector: string, intent: string): Promise<void>;
  waitSelectorText(selector: string, text: string, intent: string): Promise<void>;
  assertLiveSelectorText(selector: string, text: string, intent: string): Promise<void>;
  assertLiveSelectorAbsent(selector: string, intent: string): Promise<void>;
  assertLiveCondition(expression: string, intent: string): Promise<void>;
}

function createContext(tc: string, intent: string): GoldenContext {
  const runId = `golden-dynamic-waits-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-dw-${tc}-${Math.random().toString(16).slice(2, 8)}`;
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

export async function runDynamicWaitsGolden(
  tc: string,
  intent: string,
  steps: (golden: DynamicWaitsGolden) => Promise<void>,
): Promise<void> {
  const ctx = createContext(tc, intent);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: DynamicWaitsGolden = {
    ...ctx,
    async openPage() {
      await run(ctx, "open dynamic waits", [ctx.agentBrowser, "--session", ctx.session, "open", dynamicWaitsUrl]);
      await record(ctx, "navigation", { route: dynamicWaitsUrl });
    },
    async clickSelector(selector, stepIntent) {
      await run(ctx, `click ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "click", selector]);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async domClickSelector(selector, stepIntent) {
      await run(ctx, `dom click ${selector}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found"); el.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, view: window })); el.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, cancelable: true, view: window })); el.click(); return true; })()`,
      ]);
      await record(ctx, "action", { method: "clickSelector", args: [selector], intent: stepIntent });
    },
    async clickText(text, stepIntent) {
      await run(ctx, `click text ${text}`, [ctx.agentBrowser, "--session", ctx.session, "find", "text", text, "click"]);
      await record(ctx, "action", { method: "clickByText", args: [text], intent: stepIntent });
    },
    async waitDuration(ms, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "duration", ms }, intent: stepIntent });
    },
    async waitSelector(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async waitSelectorAbsent(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorAbsent", selector }, intent: stepIntent });
    },
    async waitSelectorText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async assertLiveSelectorText(selector, text, stepIntent) {
      await run(ctx, `live check ${selector}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const el = document.querySelector(${JSON.stringify(selector)}); if (el && (el.textContent || "").includes(${JSON.stringify(text)})) resolve(true); else if (Date.now() - started > 7000) reject(new Error(${JSON.stringify(stepIntent)})); else setTimeout(tick, 100); }; tick(); }))()`,
      ]);
    },
    async assertLiveSelectorAbsent(selector, stepIntent) {
      await run(ctx, `live absent ${selector}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { if (!document.querySelector(${JSON.stringify(selector)})) resolve(true); else if (Date.now() - started > 7000) reject(new Error(${JSON.stringify(stepIntent)})); else setTimeout(tick, 100); }; tick(); }))()`,
      ]);
    },
    async assertLiveCondition(expression, stepIntent) {
      await run(ctx, `live condition ${stepIntent}`, [ctx.agentBrowser, "--session", ctx.session, "eval", expression]);
    },
  };

  try {
    const start = await run(ctx, "start", [ctx.agentQa, "start", intent, "--session", ctx.session]);
    sid = start.match(/started sid=(\S+)/)?.[1] || "";
    await steps(golden);
    await run(ctx, "flush", [ctx.agentQa, "flush"]);
    await run(ctx, "verify", [ctx.agentQa, "verify", sid]);
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
