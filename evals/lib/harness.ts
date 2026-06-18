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
  fastPath?: string[];
  scenarioMatches(scenario: unknown): boolean;
}

export interface RunOptions {
  provider: Provider;
  model: string;
  json: boolean;
  list: boolean;
  continueOnFailure: boolean;
  scriptedFastPath: boolean;
  timeoutMs: number;
  idleTimeoutMs: number;
  maxCases?: number;
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
    scriptedFastPath: false,
    timeoutMs: 4 * 60 * 1000,
    idleTimeoutMs: Number(process.env.AGENT_QA_EVAL_IDLE_TIMEOUT_MS || "60000"),
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
      case "--scripted-fast-path":
        options.scriptedFastPath = true;
        break;
      case "--timeout-ms":
        options.timeoutMs = Number(args[++i] || options.timeoutMs);
        break;
      case "--idle-timeout-ms":
        options.idleTimeoutMs = Number(args[++i] || options.idleTimeoutMs);
        break;
      case "--max-cases":
        options.maxCases = Number(args[++i] || "0");
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
  if (!Number.isFinite(options.idleTimeoutMs) || options.idleTimeoutMs < 0) {
    throw new Error("--idle-timeout-ms must be zero or a positive number");
  }
  if (options.maxCases !== undefined && (!Number.isFinite(options.maxCases) || options.maxCases <= 0)) {
    throw new Error("--max-cases must be a positive number");
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
  --scripted-fast-path
                      Execute a case fastPath directly instead of invoking a model
  --json             Print JSON report
  --timeout-ms <ms>  Timeout per case (default: 240000)
  --idle-timeout-ms <ms>
                      Kill after no output for this long after first command (default: 60000)
  --max-cases <n>    Limit selected cases, useful for paced suite execution
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

export function buildPrompt(
  evalCase: EvalCase,
  agentQaBin: string,
  agentBrowserBin: string,
  sessionName: string,
  scenariosRoot: string,
  recordRoot: string,
): string {
  const extra = evalCase.extraConstraints?.map((line) => `- ${line}`).join("\n") || "";
  const fastPathCommands = evalCase.fastPath?.map((line) => line
    .replaceAll("{qa}", agentQaBin)
    .replaceAll("{browser}", agentBrowserBin)
    .replaceAll("{session}", sessionName)
    .replaceAll("{scenarios}", scenariosRoot)
    .replaceAll("{record}", recordRoot));

  if (fastPathCommands?.length) {
    const script = fastPathScript(fastPathCommands, agentQaBin);
    return `Run this agent-qa eval by executing the commands below in order.

Case: ${evalCase.name}
Success target: flushed matching scenario.json plus passing replay artifact.

Rules:
- Execute exactly one shell command: the script block below.
- Do not split it into separate commands.
- Do not explain, plan, inspect files, run diagnostics, or discover alternatives.
- Do not run Selenium, Playwright, Puppeteer, snapshots, help, version, session-list, or doctor commands.
- The script uses set -euo pipefail, captures <sid>, and stops at the first non-zero command.
- Use exactly these binaries: \`${agentQaBin}\` and \`${agentBrowserBin}\`.
- Quote CSS selectors exactly as shown.

\`\`\`bash
${script}
\`\`\`
`;
  }

  return `${evalCase.prompt}

Eval constraints:
- Work in ${repoRoot}.
- Use agent-qa/agent-browser only. Do not use Playwright, Puppeteer, Selenium, or custom browser scripts.
- First run \`${agentQaBin} skills get core\` and follow it.
- Do not load opencode skills or any non-agent-qa skill. The agent-qa core skill output and this prompt are the only instructions for the eval.
- Do not probe tool versions, available verbs, help output, session lists, or diagnostics. The allowed command shapes are already listed here.
- Use \`${agentQaBin}\` for every agent-qa command in this eval.
- Do not use plain \`agent-qa\`; use \`${agentQaBin}\` exactly.
- Use \`${agentBrowserBin}\` for every browser action in this eval.
- Do not use plain \`agent-browser\`; use \`${agentBrowserBin}\` exactly.
- Every \`${agentBrowserBin}\` call is hard-capped by the eval wrapper. You may run multiple browser commands in sequence for a scenario. If one browser command exits non-zero or times out, stop immediately and report that command/output. Do not retry the same failed command, do not increase its timeout, and do not run doctor/session-list diagnostics.
- Do not narrate progress between commands. Execute the next required command immediately.
- Do not print a plan, recap, diagnosis, or suggested fix before flush/verify/replay finishes or fails.
- Set AGENT_QA_SCENARIOS_DIR=${scenariosRoot} and AGENT_QA_RECORD_DIR=${recordRoot} for every agent-qa command.
- Use one explicit browser session for the whole recording: \`${sessionName}\`.
- Start with exactly this shape: \`AGENT_QA_SCENARIOS_DIR=${scenariosRoot} AGENT_QA_RECORD_DIR=${recordRoot} ${agentQaBin} start "<intent>" --session ${sessionName}\`.
- Drive browser actions with exactly this shape: \`${agentBrowserBin} --session ${sessionName} <verb> ...\`.
- Record actions with exactly this shape: \`AGENT_QA_SCENARIOS_DIR=${scenariosRoot} AGENT_QA_RECORD_DIR=${recordRoot} ${agentQaBin} record-step <kind> '<json>'\`.
- After every manual agent-browser action, immediately run the matching record-step before any explanation or further browser command. Do not add a duplicate record-step after agent-qa helpers that already auto-record, such as smart-click or fill-unique.
- Action method names such as clickSelector, clickRole, fillBySelector, and selectBySelector are record-step payload methods, not ${agentBrowserBin} verbs.
- Waits are recorded with ${agentQaBin} record-step wait payloads. Do not run ${agentBrowserBin} wait-for-selector, waitForSelector, or wait commands; those are not eval browser verbs.
- Do not pass \`--session\` to \`record-step\`, \`flush\`, or \`verify\`.
- If recording succeeds, run flush, verify, and replay.
- Keep the scenario minimal. Do not record duplicate checks for the same fact; one URL check plus one stable visible-state check is enough for simple navigation cases.
- Stop at the first framework issue and report the command/output.
- Stop after the first non-zero command. Do not try a corrected command, do not repair the buffer, and do not continue after parse/validation errors.
- Payload JSON keys must match the examples exactly: navigation uses route, wait uses condition.kind plus selector/text/pattern fields, action uses method plus args, assert uses kind plus args plus intent.
- Do not inspect repository files, grep examples, or read existing scenarios before recording. The skill output and this prompt are the only instructions for the eval.
- Do not edit generated artifacts by hand: no scenario.json edits, no scenario.steps.jsonl edits, and no replay artifact edits.
- If replay fails, stop and report. Do not patch generated artifacts to make replay pass.
${extra}

Success target:
- A flushed scenario.json exists under ${scenariosRoot}.
- A replay for that scenario passes end-to-end.
`;
}

async function streamToString(
  stream: ReadableStream<Uint8Array>,
  onChunk: (chunk: string) => void,
): Promise<string> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let output = "";
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    const chunk = decoder.decode(value, { stream: true });
    output += chunk;
    onChunk(chunk);
  }
  const finalChunk = decoder.decode();
  if (finalChunk) {
    output += finalChunk;
    onChunk(finalChunk);
  }
  return output;
}

function appendEvent(eventsPath: string, event: Record<string, unknown>): void {
  writeFileSync(eventsPath, `${JSON.stringify({ ts: new Date().toISOString(), ...event })}\n`, {
    encoding: "utf-8",
    flag: "a",
  });
}

function commandLines(chunk: string): string[] {
  return chunk
    .split("\n")
    .map((line) => line.replace(/\u001b\[[0-9;]*m/g, ""))
    .filter((line) => /^\$\s+/.test(line.trim()))
    .map((line) => line.trim().replace(/^\$\s+/, ""));
}

function shellSingleQuote(value: string): string {
  return `'${value.replaceAll("'", `'\\''`)}'`;
}

function fastPathScript(commands: string[], agentQaBin: string): string {
  const lines = [
    "set -euo pipefail",
    `printf '$ %s\\n' ${shellSingleQuote(`${agentQaBin} skills get core`)}`,
    `${agentQaBin} skills get core`,
    "sid=",
  ];

  for (const command of commands) {
    lines.push(`printf '$ %s\\n' ${shellSingleQuote(command)}`);
    if (/\sstart\s/.test(command)) {
      lines.push(`start_output=$(${command} 2>&1)`);
      lines.push(`printf '%s\\n' "$start_output"`);
      lines.push(`sid=$(printf '%s\\n' "$start_output" | sed -n 's/^started sid=\\([^[:space:]]*\\).*$/\\1/p' | head -n 1)`);
      lines.push(`test -n "$sid"`);
    } else {
      lines.push(command.replace("replay <sid>", 'replay "$sid"'));
    }
  }

  return lines.join("\n");
}

async function runOpencode(
  options: RunOptions,
  prompt: string,
  env: Record<string, string>,
  eventsPath: string,
): Promise<{ stdout: string; stderr: string; exitCode: number; command: string[]; idleTimedOut?: boolean; lastCommand?: string }> {
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
  let idleTimedOut = false;
  let lastOutputAt = Date.now();
  let lastCommand: string | undefined;
  appendEvent(eventsPath, { stream: "harness", event: "spawn", command });
  const timer = setTimeout(() => {
    timedOut = true;
    appendEvent(eventsPath, { stream: "harness", event: "timeout", timeoutMs: options.timeoutMs, lastCommand });
    proc.kill();
  }, options.timeoutMs);

  const idleTimer = options.idleTimeoutMs > 0
    ? setInterval(() => {
        const idleMs = Date.now() - lastOutputAt;
        if (idleMs < options.idleTimeoutMs) return;
        idleTimedOut = true;
        appendEvent(eventsPath, { stream: "harness", event: "idle-timeout", idleMs, idleTimeoutMs: options.idleTimeoutMs, lastCommand });
        proc.kill();
      }, Math.min(options.idleTimeoutMs, 5_000))
    : undefined;

  const onChunk = (stream: "stdout" | "stderr", chunk: string) => {
    lastOutputAt = Date.now();
    appendEvent(eventsPath, { stream, event: "chunk", text: chunk });
    for (const commandLine of commandLines(chunk)) {
      lastCommand = commandLine;
      appendEvent(eventsPath, { stream: "harness", event: "command", command: commandLine });
    }
  };

  const [stdout, stderr, exitCode] = await Promise.all([
    streamToString(proc.stdout, (chunk) => onChunk("stdout", chunk)),
    streamToString(proc.stderr, (chunk) => onChunk("stderr", chunk)),
    proc.exited,
  ]);
  clearTimeout(timer);
  if (idleTimer) clearInterval(idleTimer);
  appendEvent(eventsPath, { stream: "harness", event: "exit", exitCode, timedOut, idleTimedOut, lastCommand });

  if (idleTimedOut) {
    return {
      stdout,
      stderr: `${stderr}\n[eval] idle timed out after ${options.idleTimeoutMs}ms without output${lastCommand ? ` after command: ${lastCommand}` : " before first command"}`,
      exitCode: exitCode || 124,
      command,
      idleTimedOut,
      lastCommand,
    };
  }

  if (timedOut) {
    return {
      stdout,
      stderr: `${stderr}\n[eval] timed out after ${options.timeoutMs}ms`,
      exitCode: exitCode || 124,
      command,
      lastCommand,
    };
  }

  return { stdout, stderr, exitCode, command, lastCommand };
}

async function runScriptedFastPath(
  script: string,
  env: Record<string, string>,
  eventsPath: string,
): Promise<{ stdout: string; stderr: string; exitCode: number; command: string[]; lastCommand?: string }> {
  const command = ["/bin/bash", "-lc", script];
  appendEvent(eventsPath, { stream: "harness", event: "spawn-scripted-fast-path", command });
  const proc = Bun.spawn(command, {
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
    env,
  });
  let lastCommand: string | undefined;
  const onChunk = (stream: "stdout" | "stderr", chunk: string) => {
    appendEvent(eventsPath, { stream, event: "chunk", text: chunk });
    for (const commandLine of commandLines(chunk)) {
      lastCommand = commandLine;
      appendEvent(eventsPath, { stream: "harness", event: "command", command: commandLine });
    }
  };
  const [stdout, stderr, exitCode] = await Promise.all([
    streamToString(proc.stdout, (chunk) => onChunk("stdout", chunk)),
    streamToString(proc.stderr, (chunk) => onChunk("stderr", chunk)),
    proc.exited,
  ]);
  appendEvent(eventsPath, { stream: "harness", event: "exit", exitCode, scriptedFastPath: true, lastCommand });
  return { stdout, stderr, exitCode, command, lastCommand };
}

function createEvalBin(agentQaTarget: string, agentBrowserTarget: string): { binDir: string; agentQa: string; agentBrowser: string } {
  const binDir = "/tmp/agent-qa-eval-bin";
  mkdirSync(binDir, { recursive: true });
  const agentQa = resolve(binDir, "agent-qa");
  const agentBrowser = resolve(binDir, "agent-browser");
  const resolvedAgentQaTarget = resolveExecutableTarget(agentQaTarget);
  const resolvedAgentBrowserTarget = resolveExecutableTarget(agentBrowserTarget);
  writeFileSync(agentQa, `#!/bin/sh
if [ "\${AGENT_QA_EVAL_COMPACT_SKILL:-1}" = "1" ] && [ "$1" = "skills" ] && [ "$2" = "get" ] && [ "$3" = "core" ]; then
  cat <<'EOF'
# agent-qa core eval contract

Record one replayable browser scenario. Use only these command shapes.

Start:
AGENT_QA_SCENARIOS_DIR=<dir> AGENT_QA_RECORD_DIR=<dir> agent-qa start "<intent>" --session <session>

Browser actions:
agent-browser --session <session> open <url>
agent-browser --session <session> click <visible text or CSS selector>
agent-browser --session <session> fill <CSS selector> <value>
agent-browser --session <session> upload <CSS selector> <file>
agent-browser --session <session> eval '<js expression>'

Do not use agent-browser launch, snapshot, wait, wait-for-selector, clickSelector, fillBySelector, selectBySelector, uploadBySelector, or clickRole as browser verbs. Do not add --url to open or --js to eval; pass the URL/expression as the next positional argument.
Quote CSS selectors that contain #, [, ], quotes, spaces, or shell metacharacters, for example agent-browser --session <session> fill '#search_product' jeans and agent-browser --session <session> click 'a[href="/test_cases"]'.

Record immediately after each manual browser action:
agent-qa record-step navigation '{"route":"https://example.com/path"}'
agent-qa record-step action '{"method":"clickSelector","args":["#selector"],"intent":"click target"}'
agent-qa record-step action '{"method":"fillBySelector","args":["#selector","value"],"intent":"fill target"}'
agent-qa record-step action '{"method":"selectBySelector","args":["#selector","value"],"intent":"select option"}'
agent-qa record-step action '{"method":"uploadBySelector","args":["#selector","evals/fixtures/file.txt"],"intent":"upload file"}'

Waits/checks:
agent-qa record-step wait '{"condition":{"kind":"selector","selector":"#selector"},"intent":"selector visible"}'
agent-qa record-step wait '{"condition":{"kind":"selectorText","selector":"#selector","text":"Expected text"},"intent":"text visible"}'
agent-qa record-step wait '{"condition":{"kind":"text","text":"Expected text"},"intent":"text visible"}'
agent-qa record-step wait '{"condition":{"kind":"url","pattern":"/path"},"intent":"url reached"}'
agent-qa record-step assert '{"kind":"url","args":["/path"],"intent":"url reached"}'

No aliases or invented keys: navigation not nav, route not url, condition.kind not selector at top level, text not pattern for text waits. Do not add notes or timeout keys.
CSS selectors do not go in assert present/absent args; use wait selector/selectorText for CSS checks.
Do not record duplicate steps after smart-click/fill-unique if you use them; prefer manual browser action plus record-step in evals.
Stop after the first non-zero command. Do not repair, retry, truncate, or continue.

Finish:
agent-qa flush
agent-qa verify
agent-qa replay <sid> --session <session>-replay
EOF
  exit 0
fi
exec ${JSON.stringify(resolvedAgentQaTarget)} "$@"
`, "utf-8");
  writeFileSync(agentBrowser, `#!/bin/sh
timeout_ms=\${AGENT_QA_EVAL_AGENT_BROWSER_TIMEOUT_MS:-45000}
timeout_sec=$(( (timeout_ms + 999) / 1000 ))
${JSON.stringify(resolvedAgentBrowserTarget)} "$@" &
child=$!
(
  sleep "$timeout_sec"
  if kill -0 "$child" 2>/dev/null; then
    printf '%s\n' "[eval-agent-browser] timed out after \${timeout_ms}ms: ${resolvedAgentBrowserTarget} $*" >&2
    kill "$child" 2>/dev/null
    sleep 2
    kill -9 "$child" 2>/dev/null
    exit 124
  fi
) &
watcher=$!
wait "$child"
status=$?
kill "$watcher" 2>/dev/null
wait "$watcher" 2>/dev/null
exit "$status"
`, "utf-8");
  chmodSync(agentQa, 0o755);
  chmodSync(agentBrowser, 0o755);
  return { binDir, agentQa, agentBrowser };
}

function resolveExecutableTarget(target: string): string {
  if (target.includes("/")) return target;
  return Bun.which(target) || target;
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
  const browserTimeouts = output.match(/\[eval-agent-browser\] timed out after/g) || [];
  if (browserTimeouts.length > 1) return "agent retried agent-browser after an eval-wrapper timeout";
  const shellFailure = output.match(/(?:zsh:\d+: no matches found: [^\n]+|Missing arguments for: [^\n]+|Usage: agent-browser [^\n]+)/)?.[0];
  if (shellFailure) return `agent browser command failed and was not handled as terminal: ${shellFailure}`;
  const firstCliError = output.match(/agent-qa record-step: [^\n]+|Unknown command: [^\n]+|agent-qa flush: [^\n]+|agent-qa verify: [^\n]+|agent-qa replay: [^\n]+/i)?.[0];
  if (firstCliError) return `agent continued after CLI error: ${firstCliError}`;
  const unsupportedBrowserFlag = output.match(/^.*\$\s+.*(?:\S*agent-browser|\/tmp\/agent-qa-eval-bin\/agent-browser).*\s(--url|--js)\b/im)?.[1];
  if (unsupportedBrowserFlag) return `agent used unsupported agent-browser flag ${unsupportedBrowserFlag}`;
  const agentBrowserCommand = /^.*\$\s+.*(?:\S*agent-browser|\/tmp\/agent-qa-eval-bin\/agent-browser)\s+(?:--session\s+\S+\s+)?(clickSelector|clickRole|fillBySelector|selectBySelector|uploadBySelector|wait-for-selector|waitForSelector|wait)\b/im;
  if (agentBrowserCommand.test(output)) {
    return "agent used a record-step method or unsupported wait as an agent-browser verb";
  }
  const patterns: Array<[RegExp, string]> = [
    [/apply_patch/i, "agent attempted to patch files during eval"],
    [/\$\s+.*agent-browser\s+--session\s+\S+\s+snapshot\b/i, "agent used a broad agent-browser snapshot during eval instead of selector checks"],
    [/updated scenario file/i, "agent edited generated scenario.json"],
    [/relied on scenario\.json edits|preserving manual scenario edits|manual scenario\.json edit/i, "agent relied on scenario.json edits"],
    [/manual edits/i, "agent relied on manual artifact edits"],
    [/fixed .*flushed scenario/i, "agent fixed the flushed scenario instead of re-recording"],
    [/move scenario\.json edits/i, "agent suggested preserving manual scenario edits"],
    [/shall i .*\?|do you want me to|if you confirm/i, "agent stopped to ask for confirmation instead of completing or failing the eval"],
    [/\$\s+agent-qa\s/i, "agent used plain agent-qa instead of eval wrapper"],
    [/\$\s+agent-browser\s/i, "agent used plain agent-browser instead of eval wrapper"],
    [/Unknown command: (launch|clickSelector|clickRole|fillBySelector|selectBySelector|uploadBySelector|wait-for-selector|waitForSelector|wait)\b/i, "agent used an unsupported agent-browser verb"],
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

function inspectRecordProgress(recordRoot: string): string {
  const stepsPath = resolve(recordRoot, "scenario.steps.jsonl");
  if (!existsSync(stepsPath)) return "no record steps were written";
  const content = readFileSync(stepsPath, "utf-8").trim();
  if (!content) return "record steps file is empty";
  return `${content.split("\n").length} record step(s) were written before failure`;
}

function timeoutDetail(recordRoot: string, artifacts: { scenario?: string; error?: string }): string {
  if (artifacts.scenario) {
    return `${artifacts.error || "timed out after scenario creation"}; scenario=${artifacts.scenario}`;
  }
  return inspectRecordProgress(recordRoot);
}

export async function runCase(evalCase: EvalCase, options: RunOptions): Promise<EvalReport> {
  const runId = mintRunId(evalCase);
  const resultRoot = resolve(evalsRoot, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  const eventsPath = resolve(resultRoot, "events.jsonl");
  const sessionName = sessionNameFor(evalCase);
  mkdirSync(scenariosRoot, { recursive: true });
  mkdirSync(recordRoot, { recursive: true });

  const localAgentQa = resolve(repoRoot, "cli/target/debug/agent-qa");
  const agentQaTarget = existsSync(localAgentQa) ? localAgentQa : "agent-qa";
  const agentBrowserTarget = process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN || "agent-browser";
  const evalBin = createEvalBin(agentQaTarget, agentBrowserTarget);
  const agentQaBin = evalBin.agentQa;
  const agentBrowserBin = evalBin.agentBrowser;
  const fastPathCommands = evalCase.fastPath?.map((line) => line
    .replaceAll("{qa}", agentQaBin)
    .replaceAll("{browser}", agentBrowserBin)
    .replaceAll("{session}", sessionName)
    .replaceAll("{scenarios}", scenariosRoot)
    .replaceAll("{record}", recordRoot));
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
    AGENT_QA_REPO_ROOT: repoRoot,
    AGENT_QA_EVAL_AGENT_BROWSER_TIMEOUT_MS: process.env.AGENT_QA_EVAL_AGENT_BROWSER_TIMEOUT_MS || "45000",
    AGENT_QA_AGENT_BROWSER_TIMEOUT_MS: process.env.AGENT_QA_AGENT_BROWSER_TIMEOUT_MS || "45000",
    AGENT_QA_RECORD_SKIP_SIDECARS: process.env.AGENT_QA_RECORD_SKIP_SIDECARS || "1",
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
    const result = options.scriptedFastPath && fastPathCommands?.length
      ? await runScriptedFastPath(fastPathScript(fastPathCommands, agentQaBin), env, eventsPath)
      : await runOpencode(options, prompt, env, eventsPath);
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
  const timeoutMatch = stderr.match(/\[eval\] timed out after \d+ms/);
  const idleTimeoutMatch = stderr.match(/\[eval\] idle timed out after \d+ms[^\n]*/);
  const completedArtifacts = Boolean(artifacts.scenario && artifacts.replay && !artifacts.error);
  const pass = !error &&
    !forbiddenBehavior &&
    completedArtifacts &&
    (exitCode === 0 || Boolean(timeoutMatch) || Boolean(idleTimeoutMatch));
  const failureError = pass
    ? undefined
    : error ||
      forbiddenBehavior ||
      idleTimeoutMatch?.[0] ||
      (!completedArtifacts && timeoutMatch ? `${timeoutMatch[0]} (${timeoutDetail(recordRoot, artifacts)})` : undefined) ||
      artifacts.error ||
      (exitCode === 0 ? undefined : `agent exited ${exitCode}`);
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
    error: failureError,
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
