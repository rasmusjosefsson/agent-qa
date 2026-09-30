// timing-tc01 — `{"timing": "<stepId>"}` claim coverage end to end:
//   a fixture with a fast click and a ~700ms API load proves the claim
//   reads this run's own events.jsonl — lt/gt on real step durations,
//   exists on a ran step, notExists on an id that never ran.
//
// file:// pages cannot fetch, so the fixture is served over http by an
// in-process server (api/slow.json answers after 700ms).

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "timing-tc01";
const intent = "timing claims compare this run's step durations";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-timing-${tc}-${runId}`);
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
    if (rel === "api/slow.json") {
      await Bun.sleep(700);
      return Response.json({ done: true });
    }
    if (req.method !== "GET") {
      return Response.json({ error: "method not allowed" }, { status: 405 });
    }
    const file = join(fixturesRoot, rel);
    if (!file.startsWith(fixturesRoot) || !existsSync(file) || !statSync(file).isFile()) {
      return Response.json({ error: "not found" }, { status: 404 });
    }
    const type = file.endsWith(".html") ? "text/html" : "text/plain";
    return new Response(Bun.file(file), { headers: { "content-type": type } });
  },
});
const base = `http://127.0.0.1:${server.port}`;

const css = (value: string, reason: string) => ({ raw: { kind: "css", value }, reason });
const lit = (literal: unknown) => ({ from: "literal", literal });

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
      intent: "open the timing fixture",
      kind: "do",
      verb: "goto",
      value: lit(`${base}/timing.html`),
    },
    {
      id: "s1",
      intent: "fast action — instant DOM write",
      kind: "do",
      verb: "click",
      on: css("#fast", "fast button"),
    },
    {
      id: "s2",
      intent: "fast text landed",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "fast",
      },
    },
    {
      id: "s3",
      intent: "kick off the 700ms API call",
      kind: "do",
      verb: "click",
      on: css("#slow", "slow button"),
    },
    {
      id: "s4",
      intent: "wait for the slow request (~700ms)",
      kind: "do",
      verb: "wait",
      params: { url: "*/api/slow.json", timeoutMs: 8000 },
    },
    {
      id: "s5",
      intent: "slow text landed",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "slow done",
      },
    },
    {
      id: "s6",
      intent: "the fast click really was fast",
      kind: "check",
      claim: { subject: { timing: "s1" }, predicate: "lt", value: 5000 },
    },
    {
      // The 700ms fetch is absorbed by the click's post-settle, so s3's
      // measured duration carries the delay and s4's wait matches fast.
      id: "s7",
      intent: "the fetch-starting click measured real time",
      kind: "check",
      claim: { subject: { timing: "s3" }, predicate: "gte", value: 400 },
    },
    {
      id: "s8",
      intent: "the wait matched fast — the entry already existed",
      kind: "check",
      claim: { subject: { timing: "s4" }, predicate: "lt", value: 3000 },
    },
    {
      id: "s9",
      intent: "a ran step has a timing row",
      kind: "check",
      claim: { subject: { timing: "s2" }, predicate: "exists" },
    },
    {
      id: "s10",
      intent: "a never-ran step id has none",
      kind: "check",
      claim: { subject: { timing: "sZZ" }, predicate: "notExists" },
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
  const r = { name, command: cmd, exitCode, stdout, stderr };
  results.push(r);
  if (exitCode !== 0) {
    console.error(stdout);
    console.error(stderr);
  }
  return r;
}

let pass = false;
let error = "";
try {
  results.push({
    ...(await run("check", [agentQa, "scenario", "check", scenarioFile])),
  });
  await run("replay", [agentQa, "replay", sid, "--session", `${session}-replay`]);
  pass = true;
} catch (err) {
  error = err instanceof Error ? err.message : String(err);
} finally {
  server.stop(true);
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
