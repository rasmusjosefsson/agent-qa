import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const automationExerciseUrl = "https://www.automationexercise.com";

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

export interface AutomationExerciseGolden extends GoldenContext {
  openHome(): Promise<void>;
  openUrl(url: string, intent: string): Promise<void>;
  clickText(text: string, intent: string): Promise<void>;
  domClickSelector(selector: string, intent: string): Promise<void>;
  fillSelector(selector: string, value: string, intent: string): Promise<void>;
  uploadSelector(selector: string, fixtureName: string, intent: string): Promise<void>;
  selectSelector(selector: string, value: string, intent: string): Promise<void>;
  waitUrl(path: string, intent: string): Promise<void>;
  waitSelector(selector: string, intent: string): Promise<void>;
  waitSelectorText(selector: string, text: string, intent: string): Promise<void>;
  waitText(text: string, intent: string): Promise<void>;
  assertLiveCondition(expression: string, intent: string): Promise<void>;
}

function createContext(tc: string, intent: string): GoldenContext {
  const runId = `golden-automation-exercise-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-ae-${tc}-${Math.random().toString(16).slice(2, 8)}`;
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
      AGENT_QA_RECORD_SKIP_SIDECARS: process.env.AGENT_QA_RECORD_SKIP_SIDECARS || "1",
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

export async function runAutomationExerciseGolden(
  tc: string,
  intent: string,
  steps: (golden: AutomationExerciseGolden) => Promise<void>,
): Promise<void> {
  const ctx = createContext(tc, intent);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: AutomationExerciseGolden = {
    ...ctx,
    async openHome() {
      await run(ctx, "open home", [ctx.agentBrowser, "--session", ctx.session, "open", automationExerciseUrl]);
      await record(ctx, "navigation", { route: automationExerciseUrl });
    },
    async openUrl(url, stepIntent) {
      await run(ctx, `open ${url}`, [ctx.agentBrowser, "--session", ctx.session, "open", url]);
      await record(ctx, "navigation", { route: url, intent: stepIntent });
    },
    async clickText(text, stepIntent) {
      await run(ctx, `click ${text}`, [ctx.agentBrowser, "--session", ctx.session, "click", text]);
      await record(ctx, "action", { method: "clickText", args: [text], intent: stepIntent });
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
    async fillSelector(selector, value, stepIntent) {
      await run(ctx, `fill ${selector}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found"); el.focus(); el.value = ${JSON.stringify(value)}; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); return true; })()`,
      ]);
      await record(ctx, "action", { method: "fillBySelector", args: [selector, value], intent: stepIntent });
    },
    async uploadSelector(selector, fixtureName, stepIntent) {
      const path = resolve(repoRoot, "evals/fixtures", fixtureName);
      await run(ctx, `upload ${fixtureName}`, [ctx.agentBrowser, "--session", ctx.session, "upload", selector, path]);
      await record(ctx, "action", { method: "uploadBySelector", args: [selector, `evals/fixtures/${fixtureName}`], intent: stepIntent });
    },
    async selectSelector(selector, value, stepIntent) {
      await run(ctx, `select ${selector}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found"); const value = ${JSON.stringify(value)}; const option = Array.from(el.options).find((item) => item.value === value || item.text === value); if (!option) throw new Error("option not found: " + value); el.value = option.value; el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); return true; })()`,
      ]);
      await record(ctx, "action", { method: "selectBySelector", args: [selector, value], intent: stepIntent });
    },
    async waitUrl(path, stepIntent) {
      await record(ctx, "assert", { kind: "url", args: [path], intent: stepIntent });
    },
    async waitSelector(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async waitSelectorText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async waitText(text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "text", text }, intent: stepIntent });
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
