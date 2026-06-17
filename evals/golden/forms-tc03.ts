#!/usr/bin/env bun
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const runId = `golden-forms-tc03-${new Date().toISOString().replace(/[:.]/g, "-")}`;
const resultRoot = resolve(evalsRoot, "results", runId);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
const session = `golden-forms-tc03-${Math.random().toString(16).slice(2, 8)}`;
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

async function assertLiveSelector(selector: string, intent: string): Promise<void> {
  await run(`live check ${selector}`, [
    agentBrowser,
    "--session",
    session,
    "eval",
    `(() => { if (!document.querySelector(${JSON.stringify(selector)})) throw new Error(${JSON.stringify(intent)}); return true; })()`,
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
    const start = await run("start", [agentQa, "start", "Forms TC03 invalid email validation", "--session", session]);
    sid = start.match(/started sid=(\S+)/)?.[1] || "";

    await run("open forms", [agentBrowser, "--session", session, "open", "https://qaplayground.com/practice/forms"]);
    await record("navigation", { route: "https://qaplayground.com/practice/forms" });

    await fill("#email", "not-an-email", "fill invalid email");
    await clickSelector('[data-testid="submit-form-btn"]', "submit invalid email form");
    await assertLiveSelector("#email:invalid", "email field is marked invalid");
    await waitSelector("#email:invalid", "email field is marked invalid");

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
