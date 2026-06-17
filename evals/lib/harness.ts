import { chmodSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

export type Provider = "opencode";

export interface EvalCase {
  id: string;
  suite: string;
  page?: string;
  name: string;
  prompt: string;
  extraConstraints?: string[];
  scenarioMatches(scenario: unknown): boolean;
}

export interface RunOptions {
  provider: Provider;
  model: string;
  json: boolean;
  list: boolean;
  continueOnFailure: boolean;
  timeoutMs: number;
  caseId?: string;
  suite?: string;
  page?: string;
}

export interface EvalReport {
  pass: boolean;
  caseId: string;
  caseName: string;
  suite: string;
  runId: string;
  provider: Provider;
  model: string;
  durationMs: number;
  resultRoot: string;
  scenariosRoot: string;
  recordRoot: string;
  scenario?: string;
  replay?: string;
  command: string[];
  forbiddenBehavior?: string;
  error?: string;
  stdout: string;
  stderr: string;
}

export interface RunSummary {
  pass: boolean;
  total: number;
  passed: number;
  failed: number;
  reports: EvalReport[];
}

const __dirname = dirname(fileURLToPath(import.meta.url));
export const evalsRoot = resolve(__dirname, "..");
export const repoRoot = resolve(evalsRoot, "..");

export const defaultModel = process.env.AGENT_QA_EVAL_MODEL || "github-copilot/gpt-5-mini";
export const defaultProvider = (process.env.AGENT_QA_EVAL_PROVIDER || "opencode") as Provider;

export function parseArgs(args: string[]): RunOptions {
  const options: RunOptions = {
    provider: defaultProvider,
    model: defaultModel,
    json: false,
    list: false,
    continueOnFailure: false,
    timeoutMs: 10 * 60 * 1000,
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    switch (arg) {
      case "--provider":
        options.provider = (args[++i] || "opencode") as Provider;
        break;
      case "--model":
        options.model = args[++i] || defaultModel;
        break;
      case "--case":
        options.caseId = args[++i] || "";
        break;
      case "--suite":
        options.suite = args[++i] || "";
        break;
      case "--page":
        options.page = args[++i] || "";
        break;
      case "--json":
        options.json = true;
        break;
      case "--list":
        options.list = true;
        break;
      case "--continue-on-failure":
        options.continueOnFailure = true;
        break;
      case "--timeout-ms":
        options.timeoutMs = Number(args[++i] || options.timeoutMs);
        break;
      case "--help":
      case "-h":
        printUsage();
        process.exit(0);
      default:
        throw new Error(`unknown argument: ${arg}`);
    }
  }

  if (options.provider !== "opencode") {
    throw new Error(`unsupported provider: ${options.provider}`);
  }
  if (!Number.isFinite(options.timeoutMs) || options.timeoutMs <= 0) {
    throw new Error("--timeout-ms must be a positive number");
  }
  return options;
}

export function printUsage(): void {
  console.log(`agent-qa evals

Usage: bun run run.ts [options]

Options:
  --case <id>        Run one case
  --suite <name>     Run a suite, e.g. saucedemo or qaplayground
  --page <slug>      Run one page within a suite, e.g. forms
  --provider <name>  Provider to use: opencode (default: opencode)
  --model <model>    Model to use (default: github-copilot/gpt-5-mini)
  --list             List matching cases without running them
  --continue-on-failure
                     Run every selected case instead of stopping on first failure
  --json             Print JSON report
  --timeout-ms <ms>  Timeout per case (default: 600000)
  --help, -h         Show help`);
}

export function textOf(value: unknown): string {
  return JSON.stringify(value).toLowerCase();
}

function mintRunId(evalCase: EvalCase): string {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  return `${evalCase.id}-${stamp}-${Math.random().toString(16).slice(2, 8)}`;
}

function sessionNameFor(evalCase: EvalCase): string {
  const prefix = evalCase.page || evalCase.suite;
  const tc = evalCase.id.match(/-(upload-|download-)?tc\d+/)?.[0]?.replace(/^-/, "") || "case";
  const suffix = Math.random().toString(16).slice(2, 8);
  return `eval-${prefix}-${tc}-${suffix}`.replace(/[^a-zA-Z0-9_-]/g, "-").slice(0, 48);
}

function buildPrompt(
  evalCase: EvalCase,
  agentQaBin: string,
  agentBrowserBin: string,
  sessionName: string,
  scenariosRoot: string,
  recordRoot: string,
): string {
  const extra = evalCase.extraConstraints?.map((line) => `- ${line}`).join("\n") || "";
  return `${evalCase.prompt}

Eval constraints:
- Work in ${repoRoot}.
- Use agent-qa/agent-browser only. Do not use Playwright, Puppeteer, Selenium, or custom browser scripts.
- First run \`${agentQaBin} skills get core\` and follow it.
- Use \`${agentQaBin}\` for every agent-qa command in this eval.
- Do not use plain \`agent-qa\`; use \`${agentQaBin}\` exactly.
- Use \`${agentBrowserBin}\` for every browser action in this eval.
- Do not use plain \`agent-browser\`; use \`${agentBrowserBin}\` exactly.
- Set AGENT_QA_SCENARIOS_DIR=${scenariosRoot} and AGENT_QA_RECORD_DIR=${recordRoot} for every agent-qa command.
- Use one explicit browser session for the whole recording: \`${sessionName}\`.
- Start with exactly this shape: \`AGENT_QA_SCENARIOS_DIR=${scenariosRoot} AGENT_QA_RECORD_DIR=${recordRoot} ${agentQaBin} start "<intent>" --session ${sessionName}\`.
- Drive browser actions with exactly this shape: \`${agentBrowserBin} --session ${sessionName} <verb> ...\`.
- Record actions with exactly this shape: \`AGENT_QA_SCENARIOS_DIR=${scenariosRoot} AGENT_QA_RECORD_DIR=${recordRoot} ${agentQaBin} record-step <kind> '<json>'\`.
- Do not pass \`--session\` to \`record-step\`, \`flush\`, or \`verify\`.
- If recording succeeds, run flush, verify, and replay.
- Stop at the first framework issue and report the command/output.
- Do not edit generated artifacts by hand: no scenario.json edits, no scenario.steps.jsonl edits, and no replay artifact edits.
- If replay fails, stop and report. Do not patch generated artifacts to make replay pass.
${extra}

Success target:
- A flushed scenario.json exists under ${scenariosRoot}.
- A replay for that scenario passes end-to-end.
`;
}

async function runOpencode(options: RunOptions, prompt: string, env: Record<string, string>): Promise<{ stdout: string; stderr: string; exitCode: number; command: string[] }> {
  const command = [
    "opencode",
    "run",
    "--model",
    options.model,
    "--dangerously-skip-permissions",
    "--dir",
    repoRoot,
    prompt,
  ];
  const proc = Bun.spawn(command, {
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
    env,
  });

  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    proc.kill();
  }, options.timeoutMs);

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  clearTimeout(timer);

  if (timedOut) {
    return {
      stdout,
      stderr: `${stderr}\n[eval] timed out after ${options.timeoutMs}ms`,
      exitCode: exitCode || 124,
      command,
    };
  }

  return { stdout, stderr, exitCode, command };
}

function createEvalBin(agentQaTarget: string, agentBrowserTarget: string): { binDir: string; agentQa: string; agentBrowser: string } {
  const binDir = "/tmp/agent-qa-eval-bin";
  mkdirSync(binDir, { recursive: true });
  const agentQa = resolve(binDir, "agent-qa");
  const agentBrowser = resolve(binDir, "agent-browser");
  writeFileSync(agentQa, `#!/bin/sh\nexec ${JSON.stringify(agentQaTarget)} "$@"\n`, "utf-8");
  writeFileSync(agentBrowser, `#!/bin/sh\nexec ${JSON.stringify(agentBrowserTarget)} "$@"\n`, "utf-8");
  chmodSync(agentQa, 0o755);
  chmodSync(agentBrowser, 0o755);
  return { binDir, agentQa, agentBrowser };
}

function writeStatus(resultRoot: string, status: Record<string, unknown>): void {
  writeFileSync(
    resolve(resultRoot, "status.json"),
    JSON.stringify({ updatedAt: new Date().toISOString(), ...status }, null, 2),
    "utf-8",
  );
}

function readJson(path: string): unknown | undefined {
  try {
    return JSON.parse(readFileSync(path, "utf-8"));
  } catch {
    return undefined;
  }
}

function replayPassed(replayJson: unknown): boolean {
  if (!replayJson || typeof replayJson !== "object") return false;
  const exitCode = (replayJson as { exitCode?: unknown }).exitCode;
  if (exitCode === 0) return true;
  const text = textOf(replayJson);
  return text.includes("pass") && text.includes("summary");
}

function forbiddenAgentBehavior(stdout: string, stderr: string): string | undefined {
  const output = `${stdout}\n${stderr}`;
  const patterns: Array<[RegExp, string]> = [
    [/apply_patch/i, "agent attempted to patch files during eval"],
    [/updated scenario file/i, "agent edited generated scenario.json"],
    [/scenario\.json edits/i, "agent relied on scenario.json edits"],
    [/manual edits/i, "agent relied on manual artifact edits"],
    [/fixed .*flushed scenario/i, "agent fixed the flushed scenario instead of re-recording"],
    [/move scenario\.json edits/i, "agent suggested preserving manual scenario edits"],
    [/\$\s+agent-qa\s/i, "agent used plain agent-qa instead of eval wrapper"],
    [/\$\s+agent-browser\s/i, "agent used plain agent-browser instead of eval wrapper"],
  ];
  for (const [pattern, reason] of patterns) {
    if (pattern.test(output)) return reason;
  }
  return undefined;
}

function inspectArtifacts(evalCase: EvalCase, scenariosRoot: string): { scenario?: string; replay?: string; error?: string } {
  if (!existsSync(scenariosRoot)) {
    return { error: `scenarios root does not exist: ${scenariosRoot}` };
  }

  for (const sid of readdirSync(scenariosRoot).sort()) {
    const scenarioPath = resolve(scenariosRoot, sid, "scenario.json");
    const scenario = readJson(scenarioPath);
    if (!scenario || !evalCase.scenarioMatches(scenario)) continue;

    const replaysRoot = resolve(scenariosRoot, sid, "replays");
    if (!existsSync(replaysRoot)) {
      return { scenario: scenarioPath, error: "scenario exists but no replays directory was created" };
    }
    for (const replayId of readdirSync(replaysRoot).sort().reverse()) {
      for (const fileName of ["audit.json", "replay.json"]) {
        const replayPath = resolve(replaysRoot, replayId, fileName);
        const replay = readJson(replayPath);
        if (replayPassed(replay)) {
          return { scenario: scenarioPath, replay: replayPath };
        }
      }
    }
    return { scenario: scenarioPath, error: "scenario exists but no passing replay was found" };
  }

  return { error: `no flushed scenario matched ${evalCase.id}` };
}

export async function runCase(evalCase: EvalCase, options: RunOptions): Promise<EvalReport> {
  const runId = mintRunId(evalCase);
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const sessionName = sessionNameFor(evalCase);
  mkdirSync(scenariosRoot, { recursive: true });
  mkdirSync(recordRoot, { recursive: true });

  const localAgentQa = resolve(repoRoot, "cli/target/debug/agent-qa");
  const agentQaTarget = existsSync(localAgentQa) ? localAgentQa : "agent-qa";
  const agentBrowserTarget = process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN || "agent-browser";
  const evalBin = createEvalBin(agentQaTarget, agentBrowserTarget);
  const agentQaBin = evalBin.agentQa;
  const agentBrowserBin = evalBin.agentBrowser;
  const prompt = buildPrompt(evalCase, agentQaBin, agentBrowserBin, sessionName, scenariosRoot, recordRoot);
  writeFileSync(resolve(resultRoot, "prompt.txt"), prompt, "utf-8");
  writeStatus(resultRoot, {
    status: "starting",
    caseId: evalCase.id,
    caseName: evalCase.name,
    resultRoot,
    scenariosRoot,
    recordRoot,
    sessionName,
  });

  const env = {
    ...(process.env as Record<string, string>),
    AGENT_QA_SCENARIOS_DIR: scenariosRoot,
    AGENT_QA_RECORD_DIR: recordRoot,
    AGENT_QA_EVAL_SESSION: sessionName,
    PATH: `${evalBin.binDir}:${resolve(repoRoot, "cli/target/debug")}:${process.env.PATH || ""}`,
    NO_COLOR: "1",
  };

  const start = performance.now();
  console.error(`[eval] START ${evalCase.id}`);
  console.error(`[eval] results ${resultRoot}`);
  const heartbeat = setInterval(() => {
    const elapsedMs = Math.round(performance.now() - start);
    console.error(`[eval] HEARTBEAT ${evalCase.id} elapsed=${elapsedMs}ms results=${resultRoot}`);
    writeStatus(resultRoot, {
      status: "running",
      caseId: evalCase.id,
      elapsedMs,
      resultRoot,
      scenariosRoot,
      recordRoot,
      sessionName,
    });
  }, 30_000);
  let stdout = "";
  let stderr = "";
  let exitCode = 1;
  let command: string[] = [];
  let error: string | undefined;

  try {
    writeStatus(resultRoot, {
      status: "running",
      caseId: evalCase.id,
      resultRoot,
      scenariosRoot,
      recordRoot,
      sessionName,
    });
    const result = await runOpencode(options, prompt, env);
    stdout = result.stdout;
    stderr = result.stderr;
    exitCode = result.exitCode;
    command = result.command;
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  } finally {
    clearInterval(heartbeat);
  }

  writeFileSync(resolve(resultRoot, "stdout.txt"), stdout, "utf-8");
  writeFileSync(resolve(resultRoot, "stderr.txt"), stderr, "utf-8");

  const artifacts = inspectArtifacts(evalCase, scenariosRoot);
  const forbiddenBehavior = forbiddenAgentBehavior(stdout, stderr);
  const pass = !error &&
    exitCode === 0 &&
    !forbiddenBehavior &&
    !artifacts.error &&
    Boolean(artifacts.scenario && artifacts.replay);
  const report: EvalReport = {
    pass,
    caseId: evalCase.id,
    caseName: evalCase.name,
    suite: evalCase.suite,
    runId,
    provider: options.provider,
    model: options.model,
    durationMs: Math.round(performance.now() - start),
    resultRoot,
    scenariosRoot,
    recordRoot,
    scenario: artifacts.scenario,
    replay: artifacts.replay,
    command,
    forbiddenBehavior,
    error: error ||
      forbiddenBehavior ||
      artifacts.error ||
      (exitCode === 0 ? undefined : `agent exited ${exitCode}`),
    stdout,
    stderr,
  };

  writeFileSync(resolve(resultRoot, "report.json"), JSON.stringify(report, null, 2), "utf-8");
  writeStatus(resultRoot, {
    status: pass ? "passed" : "failed",
    caseId: evalCase.id,
    durationMs: report.durationMs,
    error: report.error,
    scenario: report.scenario,
    replay: report.replay,
  });
  console.error(`[eval] END ${evalCase.id} ${pass ? "PASS" : "FAIL"} duration=${report.durationMs}ms`);
  return report;
}

export function printReport(report: EvalReport): void {
  console.log(`${report.caseId}: ${report.pass ? "PASS" : "FAIL"}`);
  console.log(`model: ${report.model}`);
  console.log(`duration: ${report.durationMs}ms`);
  console.log(`results: ${report.resultRoot}`);
  if (report.scenario) console.log(`scenario: ${report.scenario}`);
  if (report.replay) console.log(`replay: ${report.replay}`);
  if (report.error) console.log(`error: ${report.error}`);
}
