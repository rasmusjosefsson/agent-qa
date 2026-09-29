// claims-tc01 — console claims + do/wait url + do/wait idle, all on the
// local fixture server:
//   - {"console"} subjects: notExists before errors, exists after, count
//   - do/wait url — gates on the Resource Timing entry for a slow fetch
//   - do/wait idle — gates on session network quiescence after two
//     parallel slow fetches
//
// The fixture is served over http by an in-process server with a delayed
// /api/slow*.json route so the waits are observable.

import { existsSync, mkdirSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "claims-tc01";
const intent = "console claims + wait url + wait idle on local fixtures";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-claims-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;

const server = Bun.serve({
  port: 0,
  async fetch(req) {
    const url = new URL(req.url);
    const rel = decodeURIComponent(url.pathname).replace(/^\/+/, "");
    if (req.method !== "GET") {
      return Response.json({ error: "method not allowed" }, { status: 405 });
    }
    // Deliberately slow endpoints — wait url / wait idle must observe them.
    if (rel === "api/slow.json" || rel === "api/slow2.json") {
      await new Promise((r) => setTimeout(r, 600));
      return Response.json({ ok: rel });
    }
    const file = join(fixturesRoot, rel);
    if (!file.startsWith(fixturesRoot) || !existsSync(file) || !statSync(file).isFile()) {
      return Response.json({ error: "not found" }, { status: 404 });
    }
    const type = file.endsWith(".json")
      ? "application/json"
      : file.endsWith(".html")
        ? "text/html"
        : "text/plain";
    return new Response(Bun.file(file), { headers: { "content-type": type } });
  },
});
const base = `http://127.0.0.1:${server.port}`;

const css = (value: string, reason: string) => ({
  raw: { kind: "css", value },
  reason,
});

const sid = `s-${tc}-authored`;
const scenario = {
  schema: "scenario/2",
  id: sid,
  intent,
  env: { open: [{ kind: "fresh" }] },
  producedBy: { producer: "llm-author", producedAt: new Date().toISOString() },
  steps: [
    {
      id: "s0",
      intent: "open the console fixture",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: `${base}/console.html` },
    },
    {
      id: "s1",
      intent: "page logged its boot line",
      kind: "check",
      claim: {
        subject: { console: { type: "log", text: "boot ok" } },
        predicate: "exists",
      },
    },
    {
      id: "s2",
      intent: "no errors before the button",
      kind: "check",
      claim: {
        subject: { console: { type: "error" } },
        predicate: "notExists",
      },
    },
    {
      id: "s3",
      intent: "trigger the error",
      kind: "do",
      verb: "click",
      on: css("#err", "error button"),
    },
    {
      id: "s4",
      intent: "the error was logged",
      kind: "check",
      claim: {
        subject: { console: { type: "error", text: "boom" } },
        predicate: "exists",
      },
    },
    {
      id: "s5",
      intent: "still exactly one error",
      kind: "check",
      claim: {
        subject: { console: { type: "error" } },
        predicate: "countEquals",
        value: 1,
      },
    },
    {
      id: "s6",
      intent: "fire the slow fetch",
      kind: "do",
      verb: "click",
      on: css("#slow", "slow fetch button"),
    },
    {
      id: "s7",
      intent: "wait until the request completes",
      kind: "do",
      verb: "wait",
      params: { url: "*/api/slow.json", timeoutMs: 8000 },
    },
    {
      id: "s8",
      intent: "page updated after the completed request",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "slow done",
      },
    },
    {
      id: "s9",
      intent: "fire two parallel slow fetches",
      kind: "do",
      verb: "click",
      on: css("#slow2", "parallel fetch button"),
    },
    {
      id: "s10",
      intent: "wait for network quiescence",
      kind: "do",
      verb: "wait",
      params: { idle: true, idleMs: 250, timeoutMs: 8000 },
    },
    {
      id: "s11",
      intent: "both parallel fetches finished",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "slow2 done",
      },
    },
  ],
};

const scenarioDir = resolve(scenariosRoot, sid);
mkdirSync(scenarioDir, { recursive: true });
const scenarioFile = resolve(scenarioDir, "scenario.json");
writeFileSync(scenarioFile, JSON.stringify(scenario, null, 2) + "\n");

interface StepResult {
  name: string;
  command: string[];
  exitCode: number;
  stdout: string;
  stderr: string;
}
const results: StepResult[] = [];

// Async spawn — a sync spawn blocks the event loop and starves the
// in-process server the page under test is fetched from.
async function run(name: string, cmd: string[]): Promise<StepResult> {
  console.error(`[golden] ${name}`);
  const proc = Bun.spawn(cmd, {
    cwd: repoRoot,
    env: {
      ...process.env,
      AGENT_QA_SCENARIOS_DIR: scenariosRoot,
      AGENT_QA_RECORD_DIR: recordRoot,
      ...(process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN
        ? { AGENT_BROWSER_BIN: process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN }
        : {}),
    },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  const res = { name, command: cmd, exitCode, stdout, stderr };
  results.push(res);
  return res;
}

try {
  const check = await run("scenario check", [agentQa, "scenario", "check", scenarioFile]);
  const replay = await run("replay (all claims)", [
    agentQa, "replay", sid, "--session", session,
  ]);

  const pass = check.exitCode === 0 && replay.exitCode === 0;
  const report = {
    tc,
    intent,
    pass,
    resultRoot,
    steps: results.map((s) => ({ name: s.name, exitCode: s.exitCode })),
  };
  writeFileSync(resolve(resultRoot, "golden-report.json"), JSON.stringify(report, null, 2) + "\n");

  for (const s of results) {
    console.log(`--- ${s.name} (exit ${s.exitCode})`);
    process.stdout.write(s.stdout);
    if (s.stderr.trim()) process.stderr.write(s.stderr);
  }
  console.log(pass ? `PASS ${tc}` : `FAIL ${tc}`);
  process.exit(pass ? 0 : 1);
} finally {
  server.stop(true);
}
