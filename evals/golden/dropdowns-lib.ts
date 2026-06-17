import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const dropdownsUrl = "https://qaplayground.com/practice/dropdowns";

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

export interface DropdownsGolden extends GoldenContext {
  openPage(): Promise<void>;
  openCustomDropdown(selector: string, intent: string): Promise<void>;
  openCustomDropdownLive(selector: string): Promise<void>;
  recordCustomDropdownOpen(selector: string, intent: string): Promise<void>;
  clickOption(text: string, intent: string): Promise<void>;
  selectCustom(selector: string, label: string, intent: string): Promise<void>;
  selectNative(selector: string, value: string | string[], intent: string): Promise<void>;
  waitSelector(selector: string, intent: string): Promise<void>;
  waitSelectorText(selector: string, text: string, intent: string): Promise<void>;
  assertUrl(): Promise<void>;
  assertLiveCondition(expression: string, intent: string): Promise<void>;
}

function createContext(tc: string, intent: string): GoldenContext {
  const runId = `golden-dropdowns-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-dropdowns-${tc}-${Math.random().toString(16).slice(2, 8)}`;
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

export async function runDropdownsGolden(
  tc: string,
  intent: string,
  steps: (golden: DropdownsGolden) => Promise<void>,
): Promise<void> {
  const ctx = createContext(tc, intent);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: DropdownsGolden = {
    ...ctx,
    async openPage() {
      await run(ctx, "open dropdowns", [ctx.agentBrowser, "--session", ctx.session, "open", dropdownsUrl]);
      await record(ctx, "navigation", { route: dropdownsUrl });
    },
    async openCustomDropdown(selector, stepIntent) {
      await run(ctx, `focus ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "focus", selector]);
      await run(ctx, `open ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "press", "Enter"]);
      await record(ctx, "action", { method: "pressSelector", args: [selector, "Enter"], intent: stepIntent });
    },
    async openCustomDropdownLive(selector) {
      await run(ctx, `focus ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "focus", selector]);
      await run(ctx, `open ${selector}`, [ctx.agentBrowser, "--session", ctx.session, "press", "Enter"]);
    },
    async recordCustomDropdownOpen(selector, stepIntent) {
      await record(ctx, "action", { method: "pressSelector", args: [selector, "Enter"], intent: stepIntent });
    },
    async clickOption(text, stepIntent) {
      await run(ctx, `click option ${text}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const want = ${JSON.stringify(text)}; const options = Array.from(document.querySelectorAll('[role="option"]')); const el = options.find((node) => (node.textContent || '').trim() === want && node.getClientRects().length > 0); if (!el) throw new Error('visible option not found: ' + want); el.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, view: window })); el.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, cancelable: true, view: window })); el.click(); return true; })()`,
      ]);
      await record(ctx, "action", { method: "clickRole", args: ["option", text], intent: stepIntent });
    },
    async selectCustom(selector, label, stepIntent) {
      await run(ctx, `select custom ${selector} ${label}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => new Promise((resolve, reject) => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return reject(new Error('selector not found')); el.focus(); el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true })); el.dispatchEvent(new KeyboardEvent('keyup', { key: 'Enter', code: 'Enter', bubbles: true, cancelable: true })); setTimeout(() => { try { const want = ${JSON.stringify(label)}; const options = Array.from(document.querySelectorAll('[role="option"]')); const hit = options.find((node) => (node.textContent || '').trim() === want && node.getClientRects().length > 0); if (!hit) throw new Error('option not found: ' + want); hit.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, view: window })); hit.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, cancelable: true, view: window })); hit.click(); resolve(true); } catch (err) { reject(err); } }, 50); }))()`,
      ]);
      await record(ctx, "action", { method: "selectBySelector", args: [selector, label], intent: stepIntent });
    },
    async selectNative(selector, value, stepIntent) {
      const values = Array.isArray(value) ? value : [value];
      await run(ctx, `select ${selector} ${values.join(",")}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error('selector not found'); const values = ${JSON.stringify(values)}; for (const option of el.options) option.selected = values.includes(option.value) || values.includes(option.text); el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); return true; })()`,
      ]);
      await record(ctx, "action", { method: "selectBySelector", args: [selector, value], intent: stepIntent });
    },
    async waitSelector(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async waitSelectorText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async assertUrl() {
      await record(ctx, "assert", { kind: "url", args: ["/practice/dropdowns"], intent: "dropdowns page loaded" });
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
