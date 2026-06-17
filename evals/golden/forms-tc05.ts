#!/usr/bin/env bun
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const runId = `golden-forms-tc05-${new Date().toISOString().replace(/[:.]/g, "-")}`;
const resultRoot = resolve(evalsRoot, "results", runId);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
const session = `golden-forms-tc05-${Math.random().toString(16).slice(2, 8)}`;
const agentQa = existsSync(resolve(repoRoot, "cli/target/debug/agent-qa"))
  ? resolve(repoRoot, "cli/target/debug/agent-qa")
  : "agent-qa";
const agentBrowser = process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN || "agent-browser";

mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const env = {
  ...(process.env as Record<string, string>),
  AGENT_QA_SCENARIOS_DIR: scenariosRoot,
  AGENT_QA_RECORD_DIR: recordRoot,
  NO_COLOR: "1",
};

interface StepResult {
  name: string;
  command: string[];
  exitCode: number;
  stdout: string;
  stderr: string;
}

const results: StepResult[] = [];

async function run(name: string, command: string[]): Promise<string> {
  console.error(`[golden] ${name}`);
  const proc = Bun.spawn(command, {
    cwd: repoRoot,
    env,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  results.push({ name, command, exitCode, stdout, stderr });
  if (exitCode !== 0) {
    throw new Error(`${name} failed (${exitCode})\n${command.join(" ")}\n${stdout}\n${stderr}`);
  }
  return stdout;
}

async function record(kind: string, payload: unknown): Promise<void> {
  await run(`record ${kind}`, [agentQa, "record-step", kind, JSON.stringify(payload)]);
}

async function fill(selector: string, value: string, intent: string): Promise<void> {
  await run(`fill ${selector}`, [agentBrowser, "--session", session, "fill", selector, value]);
  await record("action", { method: "fillBySelector", args: [selector, value], intent });
}

async function clickSelector(selector: string, intent: string): Promise<void> {
  await run(`click ${selector}`, [agentBrowser, "--session", session, "click", selector]);
  await record("action", { method: "clickSelector", args: [selector], intent });
}

async function clickText(text: string, intent: string): Promise<void> {
  await run(`click text ${text}`, [agentBrowser, "--session", session, "find", "text", text, "click"]);
  await record("action", { method: "clickByText", args: [text], intent });
}

async function assertLiveSelectorText(selector: string, text: string, intent: string): Promise<void> {
  await run(`live check ${selector}`, [
    agentBrowser,
    "--session",
    session,
    "eval",
    `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el || !(el.textContent || "").includes(${JSON.stringify(text)})) throw new Error(${JSON.stringify(intent)}); return true; })()`,
  ]);
}

async function waitSelector(selector: string, intent: string): Promise<void> {
  await record("wait", { condition: { kind: "selector", selector }, intent });
}

async function main(): Promise<void> {
  let sid = "";
  let pass = false;
  let error = "";
  try {
    const start = await run("start", [agentQa, "start", "Forms TC05 password minimum length validation", "--session", session]);
    sid = start.match(/started sid=(\S+)/)?.[1] || "";

    await run("open forms", [agentBrowser, "--session", session, "open", "https://qaplayground.com/practice/forms"]);
    await record("navigation", { route: "https://qaplayground.com/practice/forms" });

    await fill("#firstName", "Jane", "fill first name");
    await fill("#lastName", "Tester", "fill last name");
    await fill("#email", "jane.tester@example.com", "fill email");
    await fill("#phone", "5551234567", "fill phone");
    await fill("#dob", "1990-01-02", "fill date of birth");
    await clickSelector("#gender-female", "select female gender");
    await clickSelector('[data-testid="select-country"]', "open country dropdown");
    await clickText("India", "select India");
    await fill("#city", "Mumbai", "fill city");
    await fill("#password", "short", "fill too-short password");
    await fill("#confirmPassword", "short", "fill matching confirm password");
    await clickSelector('[data-testid="checkbox-terms"]', "accept terms");
    await clickSelector('[data-testid="submit-form-btn"]', "submit form with too-short password");
    await assertLiveSelectorText(
      '[data-testid="error-password"]',
      "Password must be at least 6 characters.",
      "password minimum-length validation error is visible",
    );
    await waitSelector('[data-testid="error-password"]', "password minimum-length validation error is visible");

    await run("flush", [agentQa, "flush"]);
    await run("verify", [agentQa, "verify", sid]);
    await run("replay", [agentQa, "replay", sid, "--session", `${session}-replay`]);
    pass = true;
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  const report = {
    pass,
    runId,
    sid,
    resultRoot,
    scenariosRoot,
    recordRoot,
    session,
    error,
    results,
  };
  writeFileSync(resolve(resultRoot, "golden-report.json"), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  process.exit(pass ? 0 : 1);
}

main();
