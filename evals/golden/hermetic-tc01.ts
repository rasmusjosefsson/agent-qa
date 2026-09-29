// hermetic-tc01 — the record→replay-offline loop end to end:
//   pass 1: `replay --har` against a live server captures network.har
//   pass 2: server flips to 503 on /api/*, `replay --mock-from <run> --offline`
//           must still pass — the HAR stubs serve every app fetch
//   pass 3 (control): same broken server, replay WITHOUT --mock-from must fail
//           (proves pass 2 was the stub, not a cached page or a leak)
//
// file:// pages cannot fetch, so the fixture is served over http by an
// in-process server; `live` toggles whether /api/* answers or 503s.

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "hermetic-tc01";
const intent = "replay --har captures traffic; --mock-from --offline replays with no backend";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-hermetic-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;

let live = true;
const server = Bun.serve({
  port: 0,
  fetch(req) {
    const url = new URL(req.url);
    const rel = decodeURIComponent(url.pathname).replace(/^\/+/, "");
    if (!live && rel.startsWith("api/")) {
      return Response.json({ error: "backend down" }, { status: 503 });
    }
    if (req.method !== "GET") {
      return Response.json({ error: "method not allowed" }, { status: 405 });
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
      intent: "open the fixture page",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: `${base}/network.html` },
    },
    {
      id: "s1",
      intent: "load users (GET 200)",
      kind: "do",
      verb: "click",
      on: css("#load", "load button"),
    },
    {
      id: "s2",
      intent: "users rendered from the response",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "users:3",
      },
    },
    {
      id: "s3",
      intent: "the API call fired and returned 200",
      kind: "check",
      claim: {
        subject: { network: { urlMatches: "/api/users\\.json", method: "GET" }, ofKind: "status" },
        predicate: "equals",
        value: "200",
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
  await run("scenario check", [agentQa, "scenario", "check", scenarioFile]);

  const live1 = await run("replay --har (live)", [
    agentQa, "replay", sid, "--session", `${session}-live`, "--har",
  ]);

  // The HAR lands under <sid>/replays/<runId>/network.har; latest.txt names it.
  const latest = readFileSync(resolve(scenarioDir, "replays", "latest.txt"), "utf8").trim();
  const har = resolve(scenarioDir, "replays", latest, "network.har");
  const harOk = existsSync(har);

  // Backend goes down for the API surface — only documents still serve.
  live = false;

  const live2 = await run("replay --mock-from --offline (backend down)", [
    agentQa, "replay", sid, "--session", `${session}-mock`,
    "--mock-from", latest, "--offline",
  ]);

  // Control: without the HAR, the same broken backend must fail the run.
  const control = await run("replay --offline (no mock — must fail)", [
    agentQa, "replay", sid, "--session", `${session}-ctl`, "--offline",
  ]);

  const pass =
    results[0].exitCode === 0 &&
    live1.exitCode === 0 &&
    harOk &&
    live2.exitCode === 0 &&
    control.exitCode !== 0;

  const report = {
    tc,
    intent,
    pass,
    resultRoot,
    harRun: latest,
    steps: results.map((s) => ({ name: s.name, exitCode: s.exitCode })),
    harOk,
  };
  writeFileSync(resolve(resultRoot, "golden-report.json"), JSON.stringify(report, null, 2) + "\n");

  for (const s of results) {
    console.log(`--- ${s.name} (exit ${s.exitCode})`);
    process.stdout.write(s.stdout);
    if (s.stderr.trim()) process.stderr.write(s.stderr);
  }
  if (!harOk) console.error(`--- network.har missing at ${har}`);
  console.log(pass ? `PASS ${tc}` : `FAIL ${tc}`);
  process.exit(pass ? 0 : 1);
} finally {
  server.stop(true);
}
