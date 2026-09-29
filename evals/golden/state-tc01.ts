// state-tc01 — do/state seeds + storage/cookie claims, all on a local
// fixture that renders the stores on load:
//   - params.localStorage / sessionStorage / cookies seed before reload
//   - {"storage"} / {"cookie"} claims read them back
//   - clear* params wipe the page-visible stores (second reload renders "-")

import { existsSync, mkdirSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "state-tc01";
const intent = "do/state seeds local/session/cookie state; storage+cookie claims verify it";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-state-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;

const server = Bun.serve({
  port: 0,
  fetch(req) {
    const url = new URL(req.url);
    const rel = decodeURIComponent(url.pathname).replace(/^\/+/, "");
    if (req.method !== "GET") {
      return Response.json({ error: "method not allowed" }, { status: 405 });
    }
    const file = join(fixturesRoot, rel);
    if (!file.startsWith(fixturesRoot) || !existsSync(file) || !statSync(file).isFile()) {
      return Response.json({ error: "not found" }, { status: 404 });
    }
    const type = file.endsWith(".html") ? "text/html" : "application/json";
    return new Response(Bun.file(file), { headers: { "content-type": type } });
  },
});
const base = `http://127.0.0.1:${server.port}`;

const css = (value: string, reason: string) => ({
  raw: { kind: "css", value },
  reason,
});
const outText = (value: string) => ({
  subject: { element: css("#out", "rendered stores"), attribute: "text" },
  predicate: "equals",
  value,
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
      intent: "open the state fixture (stores empty)",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: `${base}/state.html` },
    },
    {
      id: "s1",
      intent: "renders empty stores",
      kind: "check",
      claim: outText("theme:- draft:- role:-"),
    },
    {
      id: "s2",
      intent: "seed all three stores",
      kind: "do",
      verb: "state",
      params: {
        localStorage: { theme: "dark" },
        sessionStorage: { draft: "yes" },
        cookies: [{ name: "role", value: "admin" }],
      },
    },
    {
      id: "s3",
      intent: "localStorage observed the seed",
      kind: "check",
      claim: {
        subject: { storage: { key: "theme", scope: "local" } },
        predicate: "equals",
        value: "dark",
      },
    },
    {
      id: "s4",
      intent: "sessionStorage observed the seed",
      kind: "check",
      claim: {
        subject: { storage: { key: "draft", scope: "session" } },
        predicate: "equals",
        value: "yes",
      },
    },
    {
      id: "s5",
      intent: "cookie observed the seed",
      kind: "check",
      claim: {
        subject: { cookie: "role" },
        predicate: "equals",
        value: "admin",
      },
    },
    {
      id: "s6",
      intent: "reload so the page reads the stores",
      kind: "do",
      verb: "goto",
      params: { reload: true },
      value: { from: "literal", literal: `${base}/state.html` },
    },
    {
      id: "s7",
      intent: "page rendered the seeded state",
      kind: "check",
      claim: outText("theme:dark draft:yes role:admin"),
    },
    {
      id: "s8",
      intent: "clear everything",
      kind: "do",
      verb: "state",
      params: { clearLocalStorage: true, clearSessionStorage: true, clearCookies: true },
    },
    {
      id: "s9",
      intent: "localStorage is gone",
      kind: "check",
      claim: {
        subject: { storage: "theme" },
        predicate: "notExists",
      },
    },
    {
      id: "s10",
      intent: "cookie is gone",
      kind: "check",
      claim: {
        subject: { cookie: "role" },
        predicate: "notExists",
      },
    },
    {
      id: "s11",
      intent: "reload into the empty state",
      kind: "do",
      verb: "goto",
      params: { reload: true },
      value: { from: "literal", literal: `${base}/state.html` },
    },
    {
      id: "s12",
      intent: "page rendered empty stores again",
      kind: "check",
      claim: outText("theme:- draft:- role:-"),
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
