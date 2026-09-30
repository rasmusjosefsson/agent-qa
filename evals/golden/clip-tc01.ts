// clip-tc01 — authored golden for the `{"clipboard": true}` claim subject
// and `do/state`'s `clipboard` seed key. The fixture is served over http —
// the async clipboard API needs a secure context with an origin-scoped
// permission grant (file:// has no grantable origin). Covers: seeded write
// → claim read → page-side readText → real Control+c copy gesture →
// page-initiated writeText → empty → notExists.

import { existsSync, mkdirSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "clip-tc01";
const intent = "clipboard claim + do/state seeding round-trip";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;

const server = Bun.serve({
  // Fixed-ish port in the safe range — port:0 occasionally lands on a
  // port Chrome refuses (ERR_UNSAFE_PORT) or on one still in TIME_WAIT.
  port: Number(process.env.CLIP_PORT ?? 8977),
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
const clipClaim = (predicate: string, value?: unknown) => ({
  kind: "check",
  claim: { subject: { clipboard: true }, predicate, ...(value === undefined ? {} : { value }) },
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
      intent: "open the clipboard fixture",
      kind: "do",
      verb: "goto",
      // Per-run query defeats the warm-goto reuse of a reused session —
      // without it a stale tab from a previous run's fixture is kept.
      value: lit(`${base}/clipboard.html?r=${runId}`),
    },
    {
      id: "s1",
      intent: "seed the clipboard",
      kind: "do",
      verb: "state",
      params: { clipboard: "aq-seeded-clip" },
    },
    { id: "s2", intent: "claim reads the seeded text", ...clipClaim("equals", "aq-seeded-clip") },
    {
      id: "s3",
      intent: "the page's own readText sees it too (grant is origin-scoped)",
      kind: "do",
      verb: "click",
      on: css("#readBtn", "read button"),
    },
    {
      id: "s4",
      intent: "page rendered the clipboard content",
      kind: "check",
      claim: {
        subject: { element: css("#clipOut", "clip output"), attribute: "text" },
        predicate: "contains",
        value: "clip:aq-seeded-clip",
      },
    },
    {
      id: "s5",
      intent: "field gets new text",
      kind: "do",
      verb: "clear",
      on: css("#src", "source textarea"),
    },
    {
      id: "s5b",
      intent: "type the replacement text",
      kind: "do",
      verb: "type",
      on: css("#src", "source textarea"),
      value: lit("clip-from-field"),
    },
    { id: "s6", intent: "select the field text", kind: "do", verb: "press", on: css("#src", "source textarea"), value: lit("Control+a") },
    { id: "s7", intent: "copy it with a real key chord", kind: "do", verb: "press", on: css("#src", "source textarea"), value: lit("Control+c") },
    { id: "s8", intent: "claim sees the copied field text", ...clipClaim("equals", "clip-from-field") },
    {
      id: "s9",
      intent: "page-initiated writeText under a user gesture",
      kind: "do",
      verb: "click",
      on: css("#copyBtn", "copy button"),
    },
    { id: "s10", intent: "claim sees the button-copied text", ...clipClaim("equals", "clip-from-field") },
    {
      id: "s11",
      intent: "clear the clipboard",
      kind: "do",
      verb: "state",
      params: { clipboard: "" },
    },
    { id: "s12", intent: "empty clipboard reads as notExists", ...clipClaim("notExists") },
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

// Async spawn — the fixture server lives on this event loop, so a
// synchronous spawn would freeze it and the page would never load.
async function run(name: string, cmd: string[]): Promise<void> {
  const proc = Bun.spawn(cmd, {
    env: {
      ...process.env,
      AGENT_QA_SCENARIOS_DIR: scenariosRoot,
      AGENT_QA_RECORD_DIR: recordRoot,
    },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  results.push({ name, command: cmd, exitCode, stdout, stderr });
}

try {
  await run("scenario check", [agentQa, "scenario", "check", scenarioFile]);
  await run("replay", [agentQa, "replay", sid, "--session", `${session}-replay`]);
} finally {
  server.stop();
}

const check = results[0];
const replay = results[1];
const pass = check.exitCode === 0 && replay.exitCode === 0;

const report = {
  tc,
  intent,
  pass,
  resultRoot,
  steps: results.map((s) => ({ name: s.name, exitCode: s.exitCode })),
};
writeFileSync(resolve(resultRoot, "golden-report.json"), JSON.stringify(report, null, 2) + "\n");

console.log(pass ? `PASS ${tc}` : `FAIL ${tc}`);
for (const s of results) {
  console.log(`  ${s.exitCode === 0 ? "ok" : "fail"} ${s.name} (exit ${s.exitCode})`);
  if (s.exitCode !== 0) console.log(s.stderr.slice(-800));
}
process.exit(pass ? 0 : 1);
