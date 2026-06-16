import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

type Provider = "opencode";

interface Options {
  provider: Provider;
  model: string;
  json: boolean;
  timeoutMs: number;
  keepArtifacts: boolean;
}

interface EvalReport {
  pass: boolean;
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
  error?: string;
  stdout: string;
  stderr: string;
}

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..");
const defaultModel = process.env.AGENT_QA_EVAL_MODEL || "github-copilot/gpt-5.1-mini";
const defaultProvider = (process.env.AGENT_QA_EVAL_PROVIDER || "opencode") as Provider;

const USER_PROMPT = `use the agent-qa skill, go to https://www.saucedemo.com/ login as standard user, shop one item, checkout and finish the checkout record the full flow

when we face issue we need to stop, as we maybe need to improve our agent-qa framework...

in short im trying to improve the agent-qa framework here, and need a way to improve it...`;

function parseArgs(args: string[]): Options {
  const options: Options = {
    provider: defaultProvider,
    model: defaultModel,
    json: false,
    timeoutMs: 10 * 60 * 1000,
    keepArtifacts: true,
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
      case "--json":
        options.json = true;
        break;
      case "--timeout-ms":
        options.timeoutMs = Number(args[++i] || options.timeoutMs);
        break;
      case "--no-keep-artifacts":
        options.keepArtifacts = false;
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

function printUsage(): void {
  console.log(`agent-qa saucedemo eval

Usage: bun run saucedemo.ts [options]

Options:
  --provider <name>   Provider to use: opencode (default: opencode)
  --model <model>     Model to use (default: github-copilot/gpt-5.1-mini)
  --json              Print JSON report
  --timeout-ms <ms>   Timeout for the agent run (default: 600000)
  --help, -h          Show help`);
}

function mintRunId(): string {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  return `saucedemo-${stamp}-${Math.random().toString(16).slice(2, 8)}`;
}

function buildPrompt(agentQaBin: string, scenariosRoot: string, recordRoot: string): string {
  return `${USER_PROMPT}

Eval constraints:
- Work in ${repoRoot}.
- Use agent-qa/agent-browser only. Do not use Playwright or Puppeteer.
- First run \`${agentQaBin} skills get core\` and follow it.
- Use \`${agentQaBin}\` for every agent-qa command in this eval.
- Set AGENT_QA_SCENARIOS_DIR=${scenariosRoot} and AGENT_QA_RECORD_DIR=${recordRoot} for every agent-qa command.
- This scenario starts at a public login page. Do not bootstrap profiles.
- Use one explicit browser session for the whole recording.
- Fixed values are not unique. Do not use fill-unique for standard_user, secret_sauce, Test, User, or 12345.
- If recording succeeds, run flush, verify, and replay.
- Stop at the first framework issue and report the command/output.

Success target:
- A flushed scenario.json exists under ${scenariosRoot}.
- A replay for that scenario passes end-to-end.
`;
}

async function runOpencode(options: Options, prompt: string, env: Record<string, string>): Promise<{ stdout: string; stderr: string; exitCode: number; command: string[] }> {
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

  return Promise.race([
    (async () => {
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(proc.stdout).text(),
        new Response(proc.stderr).text(),
        proc.exited,
      ]);
      return { stdout, stderr, exitCode, command };
    })(),
    new Promise<never>((_, reject) =>
      setTimeout(() => {
        proc.kill();
        reject(new Error(`timed out after ${options.timeoutMs}ms`));
      }, options.timeoutMs),
    ),
  ]);
}

function readJson(path: string): any | undefined {
  try {
    return JSON.parse(readFileSync(path, "utf-8"));
  } catch {
    return undefined;
  }
}

function scenarioLooksLikeSaucedemo(scenario: any): boolean {
  const text = JSON.stringify(scenario).toLowerCase();
  return text.includes("saucedemo.com") &&
    text.includes("standard_user") &&
    text.includes("secret_sauce") &&
    text.includes("add to cart") &&
    text.includes("checkout") &&
    text.includes("checkout-complete");
}

function replayPassed(replayJson: any): boolean {
  if (!replayJson || typeof replayJson !== "object") return false;
  if (replayJson.exitCode === 0) return true;
  const text = JSON.stringify(replayJson).toLowerCase();
  return text.includes("13/13") && text.includes("pass");
}

function inspectArtifacts(scenariosRoot: string): { scenario?: string; replay?: string; error?: string } {
  if (!existsSync(scenariosRoot)) {
    return { error: `scenarios root does not exist: ${scenariosRoot}` };
  }

  for (const sid of readdirSync(scenariosRoot).sort()) {
    const scenarioPath = resolve(scenariosRoot, sid, "scenario.json");
    const scenario = readJson(scenarioPath);
    if (!scenario || !scenarioLooksLikeSaucedemo(scenario)) continue;

    const replaysRoot = resolve(scenariosRoot, sid, "replays");
    if (!existsSync(replaysRoot)) {
      return { scenario: scenarioPath, error: "scenario exists but no replays directory was created" };
    }
    for (const replayId of readdirSync(replaysRoot).sort().reverse()) {
      const replayPath = resolve(replaysRoot, replayId, "replay.json");
      const replay = readJson(replayPath);
      if (replayPassed(replay)) {
        return { scenario: scenarioPath, replay: replayPath };
      }
    }
    return { scenario: scenarioPath, error: "scenario exists but no passing replay was found" };
  }

  return { error: "no flushed Saucedemo checkout scenario found" };
}

function printReport(report: EvalReport): void {
  console.log(`agent-qa Saucedemo eval: ${report.pass ? "PASS" : "FAIL"}`);
  console.log(`model: ${report.model}`);
  console.log(`duration: ${report.durationMs}ms`);
  console.log(`results: ${report.resultRoot}`);
  if (report.scenario) console.log(`scenario: ${report.scenario}`);
  if (report.replay) console.log(`replay: ${report.replay}`);
  if (report.error) console.log(`error: ${report.error}`);
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  const runId = mintRunId();
  const resultRoot = resolve(__dirname, "results", runId);
  const scenariosRoot = resolve(resultRoot, "scenarios");
  const recordRoot = resolve(resultRoot, "record");
  mkdirSync(scenariosRoot, { recursive: true });
  mkdirSync(recordRoot, { recursive: true });

  const localAgentQa = resolve(repoRoot, "cli/target/debug/agent-qa");
  const agentQaBin = existsSync(localAgentQa) ? localAgentQa : "agent-qa";
  const prompt = buildPrompt(agentQaBin, scenariosRoot, recordRoot);
  writeFileSync(resolve(resultRoot, "prompt.txt"), prompt, "utf-8");

  const env = {
    ...(process.env as Record<string, string>),
    AGENT_QA_SCENARIOS_DIR: scenariosRoot,
    AGENT_QA_RECORD_DIR: recordRoot,
    NO_COLOR: "1",
  };

  const start = performance.now();
  let stdout = "";
  let stderr = "";
  let exitCode = 1;
  let command: string[] = [];
  let error: string | undefined;

  try {
    const result = await runOpencode(options, prompt, env);
    stdout = result.stdout;
    stderr = result.stderr;
    exitCode = result.exitCode;
    command = result.command;
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  writeFileSync(resolve(resultRoot, "stdout.txt"), stdout, "utf-8");
  writeFileSync(resolve(resultRoot, "stderr.txt"), stderr, "utf-8");

  const artifacts = inspectArtifacts(scenariosRoot);
  const pass = !error && exitCode === 0 && !artifacts.error && Boolean(artifacts.scenario && artifacts.replay);
  const report: EvalReport = {
    pass,
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
    error: error || artifacts.error || (exitCode === 0 ? undefined : `agent exited ${exitCode}`),
    stdout,
    stderr,
  };

  writeFileSync(resolve(resultRoot, "report.json"), JSON.stringify(report, null, 2), "utf-8");
  if (options.json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printReport(report);
  }

  process.exit(pass ? 0 : 1);
}

main().catch((err) => {
  console.error(err instanceof Error ? err.stack || err.message : String(err));
  process.exit(1);
});
