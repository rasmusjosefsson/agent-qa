// ws-tc01 — WebSocket + SSE traffic is visible to network claims.
// The daemon's fetch/XHR hook is blind to sockets and streams, so the
// runner's in-process server hosts all three endpoints on one origin:
// a static ws.html, a /socket upgrade that echoes `pong:<msg>`, and an
// /events SSE stream. The scenario round-trips a socket frame, opens the
// stream, then proves the CDP-side capture saw each: the socket entry
// (`urlMatches ws://`), a frame payload (`wsPayloadContains`), and the
// EventSource's 101-less HTTP response.

import { existsSync, mkdirSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "ws-tc01";
const intent =
  "ws/sse capture: socket frames and event-streams appear in network claims";
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
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname === "/socket") {
      return srv.upgrade(req)
        ? undefined
        : Response.json({ error: "upgrade refused" }, { status: 400 });
    }
    if (url.pathname === "/events") {
      const stream = new ReadableStream({
        start(c) {
          c.enqueue(new TextEncoder().encode("data: tick-0\n\n"));
          // Stream stays open — SSE is a long-lived response.
        },
      });
      return new Response(stream, {
        headers: { "content-type": "text/event-stream" },
      });
    }
    const file = join(fixturesRoot, "ws.html");
    if (!existsSync(file) || !statSync(file).isFile()) {
      return Response.json({ error: "not found" }, { status: 404 });
    }
    return new Response(Bun.file(file), {
      headers: { "content-type": "text/html" },
    });
  },
  websocket: {
    message(ws, msg) {
      ws.send(`pong:${msg}`);
    },
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
      intent: "open the sockets fixture",
      kind: "do",
      verb: "goto",
      value: lit(`${base}/ws.html`),
    },
    {
      id: "s1",
      intent: "connect the socket (page sends 'hello' on open)",
      kind: "do",
      verb: "click",
      on: css("#connect", "connect button"),
    },
    {
      id: "s2",
      intent: "the echo frame round-tripped",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "ws:pong:hello",
      },
      context: { timeoutMs: 15000 },
    },
    {
      id: "s3",
      intent: "open the event stream",
      kind: "do",
      verb: "click",
      on: css("#events", "events button"),
    },
    {
      id: "s4",
      intent: "the first SSE tick arrived",
      kind: "check",
      claim: {
        subject: { element: css("#out", "output div"), attribute: "text" },
        predicate: "equals",
        value: "sse:tick-0",
      },
      context: { timeoutMs: 15000 },
    },
    {
      id: "s5",
      intent: "CDP capture saw the websocket itself",
      kind: "check",
      claim: {
        subject: { network: { urlMatches: "^ws://.*socket$" }, ofKind: "fired" },
        predicate: "exists",
      },
      context: { timeoutMs: 15000 },
    },
    {
      id: "s6",
      intent: "the echo frame's payload is inspectable",
      kind: "check",
      claim: {
        subject: {
          network: { urlMatches: "^ws://", wsPayloadContains: "pong:hello" },
          ofKind: "fired",
        },
        predicate: "exists",
      },
      context: { timeoutMs: 15000 },
    },
    {
      id: "s7",
      intent: "the SSE response shows up as a 200 GET",
      kind: "check",
      claim: {
        subject: { network: { urlMatches: "/events$", method: "GET" }, ofKind: "status" },
        predicate: "equals",
        value: "200",
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
