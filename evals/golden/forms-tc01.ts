#!/usr/bin/env bun
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const runId = `golden-forms-tc01-${new Date().toISOString().replace(/[:.]/g, "-")}`;
const resultRoot = resolve(evalsRoot, "results", runId);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
const session = `golden-forms-tc01-${Math.random().toString(16).slice(2, 8)}`;
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

async function main(): Promise<void> {
  let sid = "";
  let pass = false;
  let error = "";
  try {
    const start = await run("start", [agentQa, "start", "Forms TC01 valid submit", "--session", session]);
    sid = start.match(/started sid=(\S+)/)?.[1] || "";

    await run("open forms", [agentBrowser, "--session", session, "open", "https://qaplayground.com/practice/forms"]);
    await record("navigation", { route: "https://qaplayground.com/practice/forms" });

    await fill("#firstName", "John", "fill first name");
    await fill("#lastName", "Doe", "fill last name");
    await fill("#email", "john@example.com", "fill email");
    await fill("#phone", "9876543210", "fill phone");
    await fill("#dob", "1995-06-15", "fill date of birth");
    await clickSelector("#gender-male", "select male gender");
    await clickSelector('[data-testid="select-country"]', "open country dropdown");
    await clickText("India", "select India");
    await fill("#city", "Mumbai", "fill city");
    await fill("#password", "pass123", "fill password");
    await fill("#confirmPassword", "pass123", "fill confirm password");
    await clickSelector('[data-testid="checkbox-terms"]', "accept terms");
    await clickSelector('[data-testid="submit-form-btn"]', "submit form");
    await record("assert", {
      kind: "present",
      args: ["alert", "Form submitted successfully"],
      intent: "success message visible",
    });

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
