// emulate-tc03 — timezone + locale overrides via the pooled CDP flat
// session (Emulation.setTimezoneOverride / setLocaleOverride — commands
// agent-browser's `set` surface doesn't expose). Overrides apply to the
// active page immediately; the scenario reads them through a click so a
// late-bound override still counts.

import { existsSync, mkdirSync, statSync, writeFileSync } from "fs";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");
const fixturesRoot = resolve(evalsRoot, "fixtures");

const tc = "emulate-tc03";
const intent =
  "emulate timezone/locale: Intl + navigator.language reflect the override";
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
    const url = new URL(req.url);
    if (url.pathname === "/headers") {
      return Response.json({ acceptLanguage: req.headers.get("accept-language") });
    }
    const rel = decodeURIComponent(url.pathname).replace(/^\/+/, "");
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
      intent: "open the tz fixture",
      kind: "do",
      verb: "goto",
      value: lit(`${base}/tz.html`),
    },
    {
      id: "s1",
      intent: "override timezone + locale",
      kind: "do",
      verb: "emulate",
      params: { timezone: "Europe/Stockholm", locale: "sv-SE" },
    },
    {
      id: "s2",
      intent: "read the environment",
      kind: "do",
      verb: "click",
      on: css("#read", "read button"),
    },
    {
      id: "s3",
      intent: "Intl resolves the overridden zone",
      kind: "check",
      claim: {
        subject: { element: css("#tz", "timezone div"), attribute: "text" },
        predicate: "equals",
        value: "Europe/Stockholm",
      },
      context: { timeoutMs: 15000 },
    },
    {
      id: "s4",
      intent: "navigator.language follows the locale",
      kind: "check",
      claim: {
        subject: { element: css("#lang", "language div"), attribute: "text" },
        predicate: "equals",
        value: "sv-SE",
      },
      context: { timeoutMs: 15000 },
    },
    {
      id: "s5",
      intent: "Intl's default locale is Swedish-flavoured",
      kind: "check",
      claim: {
        subject: { element: css("#loc", "locale div"), attribute: "text" },
        predicate: "contains",
        value: "sv",
      },
      context: { timeoutMs: 15000 },
    },
    {
      id: "s6",
      intent: "Accept-Language on the wire follows too",
      kind: "check",
      claim: {
        subject: { element: css("#alang", "accept-language div"), attribute: "text" },
        predicate: "contains",
        value: "sv",
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
