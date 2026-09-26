import { toRecordDraft } from "./record-translate";
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const alertsUrl = "https://qaplayground.com/practice/alerts-dialogs";
// Native alert/confirm/prompt do not exist on the live page — TC01-TC06 run
// against the bundled fixture instead.
const dialogsFixtureUrl = `file://${resolve(evalsRoot, "fixtures", "dialogs.html")}`;

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

export interface AlertsDialogsGolden extends GoldenContext {
  openPage(): Promise<void>;
  openFixture(): Promise<void>;
  clickSelector(selector: string, intent: string): Promise<void>;
  domClickSelector(selector: string, intent: string): Promise<void>;
  waitSelector(selector: string, intent: string): Promise<void>;
  waitSelectorAbsent(selector: string, intent: string): Promise<void>;
  waitSelectorText(selector: string, text: string, intent: string): Promise<void>;
  assertLiveCondition(expression: string, intent: string): Promise<void>;
  assertSelectorTextEquals(selector: string, text: string, intent: string): Promise<void>;
  dialogAccept(intent: string, text?: string): Promise<void>;
  dialogDismiss(intent: string): Promise<void>;
  assertDialogText(text: string, intent: string): Promise<void>;
  assertDialogClosed(intent: string): Promise<void>;
}

function createContext(tc: string, intent: string): GoldenContext {
  const runId = `golden-alerts-dialogs-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const session = `golden-alerts-${tc}-${Math.random().toString(16).slice(2, 8)}`;
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
      // Keep native dialogs pending so the record session's `dialog` verbs
      // have something to observe; replay re-enables this itself whenever the
      // scenario contains dialog steps.
      AGENT_BROWSER_NO_AUTO_DIALOG: "1",
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

export async function runAlertsDialogsGolden(
  tc: string,
  intent: string,
  steps: (golden: AlertsDialogsGolden) => Promise<void>,
): Promise<void> {
  const ctx = createContext(tc, intent);
  let sid = "";
  let pass = false;
  let error = "";

  const golden: AlertsDialogsGolden = {
    ...ctx,
    async openPage() {
      await run(ctx, "open alerts dialogs", [ctx.agentBrowser, "--session", ctx.session, "open", alertsUrl]);
      // Practice content hydrates client-side — wait for it before driving.
      await run(ctx, "wait practice content", [ctx.agentBrowser, "--session", ctx.session, "wait", '[data-testid="scenarios-list"]']);
      await record(ctx, "navigation", { route: alertsUrl });
      await record(ctx, "wait", { condition: { kind: "selector", selector: '[data-testid="scenarios-list"]' }, intent: "practice scenarios rendered" });
    },
    async openFixture() {
      await run(ctx, "open dialogs fixture", [ctx.agentBrowser, "--session", ctx.session, "open", dialogsFixtureUrl]);
      await run(ctx, "wait fixture content", [ctx.agentBrowser, "--session", ctx.session, "wait", '[data-testid="dialogs-fixture"]']);
      await record(ctx, "navigation", { route: dialogsFixtureUrl });
      await record(ctx, "wait", { condition: { kind: "selector", selector: '[data-testid="dialogs-fixture"]' }, intent: "dialogs fixture rendered" });
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
    async waitSelector(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selector", selector }, intent: stepIntent });
    },
    async waitSelectorAbsent(selector, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorAbsent", selector }, intent: stepIntent });
    },
    async waitSelectorText(selector, text, stepIntent) {
      await record(ctx, "wait", { condition: { kind: "selectorText", selector, text }, intent: stepIntent });
    },
    async assertLiveCondition(expression, stepIntent) {
      await run(ctx, `live condition ${stepIntent}`, [ctx.agentBrowser, "--session", ctx.session, "eval", expression]);
    },
    async assertSelectorTextEquals(selector, text, stepIntent) {
      await run(ctx, `live text equals ${stepIntent}`, [
        ctx.agentBrowser,
        "--session",
        ctx.session,
        "eval",
        `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) throw new Error("selector not found"); const actual = (el.textContent || "").trim(); if (actual !== ${JSON.stringify(text)}) throw new Error("text mismatch: " + JSON.stringify(actual)); return true; })()`,
      ]);
      await record(ctx, "assert", { kind: "elementText", args: [selector, text], intent: stepIntent });
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
      // Verify the pending dialog's message live, then record the claim so
      // replay checks it before the dialog is resolved.
      const out = await run(ctx, `dialog status ${stepIntent}`, [
        ctx.agentBrowser, "--session", ctx.session, "--json", "dialog", "status",
      ]);
      const data = (JSON.parse(out).data ?? {}) as { hasDialog?: boolean; message?: string };
      if (!data.hasDialog) throw new Error(`${stepIntent}: no dialog is currently open`);
      if (!(data.message ?? "").includes(text)) {
        throw new Error(`${stepIntent}: dialog text ${JSON.stringify(data.message)} does not contain ${JSON.stringify(text)}`);
      }
      await record(ctx, "assert", { kind: "dialogText", args: [text], intent: stepIntent });
    },
    async assertDialogClosed(stepIntent) {
      await record(ctx, "assert", { kind: "dialogClosed", args: [], intent: stepIntent });
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
