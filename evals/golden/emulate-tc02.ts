// emulate-tc02 — geolocation end to end: emulate geo grants the
// browser-level permission + sets the override, so getCurrentPosition
// resolves instead of hanging headless. The fixture is served over http
// (geolocation does not resolve on file:// origins) and the emulate step
// precedes navigation — grants must exist before the page loads.

import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "emulate-tc02";
const intent = "emulate geo: override + permission grant makes getCurrentPosition resolve";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;

const server = Bun.serve({
  port: 0,
  fetch(req) {
    const rel = decodeURIComponent(new URL(req.url).pathname).replace(/^\/+/, "");
    const file = join(fixturesRoot, rel);
    if (!file.startsWith(fixturesRoot) || !existsSync(file) || !statSync(file).isFile()) {
      return Response.json({ error: "not found" }, { status: 404 });
    }
    return new Response(Bun.file(file), {
      headers: { "content-type": file.endsWith(".html") ? "text/html" : "text/plain" },
    });
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
      intent: "open the geo fixture",
      kind: "do",
      verb: "goto",
      value: lit(`${base}/geo.html`),
    },
    {
      id: "s1",
      intent: "emulate coordinates (override + permission grant)",
      kind: "do",
      verb: "emulate",
      params: { geo: { lat: 37.7749, lng: -122.4194 } },
    },
    {
      // Grants/overrides bind per navigation — reload so the page's geo
      // calls see the emulated state.
      id: "s2",
      intent: "reload so the emulation binds",
      kind: "do",
      verb: "goto",
      value: lit(`${base}/geo.html`),
      params: { reload: true },
    },
    {
      id: "s3",
      intent: "ask the page where it is",
      kind: "do",
      verb: "click",
      on: css("#locate", "locate button"),
    },
    {
      id: "s4",
      intent: "position resolved to the override",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "geo:37.7749,-122.4194",
      },
      context: { timeoutMs: 15000 },
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
  await run("check", [agentQa, "scenario", "check", scenarioFile]);
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
