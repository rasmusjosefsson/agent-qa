// emulate-tc01 — authored golden for the `emulate` verb: apply a device
// preset + an extra request header, then prove both reached the wire via
// httpbingo's echo endpoints.
// Self-contained (this branch predates edge-pages-lib): writes the
// scenario/2 doc, runs `scenario check` + `replay`, emits
// golden-report.json under results/.

import { mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "emulate-tc01";
const intent = "device preset + header emulation reach the server";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-emulate-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;

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
      intent: "emulate an iPhone 12 with a tagged header",
      kind: "do",
      verb: "emulate",
      params: { device: "iPhone 12", headers: { "X-QA-Suite": "golden" } },
    },
    {
      id: "s1",
      intent: "load the UA echo endpoint",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: "https://httpbingo.org/user-agent" },
    },
    {
      id: "s2",
      intent: "capture the echoed UA",
      kind: "do",
      verb: "read",
      on: css("body", "echoed JSON body"),
      saveAs: "uaBody",
    },
    {
      id: "s3",
      intent: "the server saw the iPhone UA",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "uaBody" },
        predicate: "contains",
        value: "iPhone",
      },
    },
    {
      id: "s4",
      intent: "load the headers echo endpoint",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: "https://httpbingo.org/headers" },
    },
    {
      id: "s5",
      intent: "capture the echoed headers",
      kind: "do",
      verb: "read",
      on: css("body", "echoed JSON body"),
      saveAs: "hdrBody",
    },
    {
      id: "s6",
      intent: "the tagged header rode along",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "hdrBody" },
        predicate: "contains",
        value: "X-Qa-Suite",
      },
    },
    {
      id: "s7",
      intent: "and its value survived",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "hdrBody" },
        predicate: "contains",
        value: "golden",
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

function run(name: string, cmd: string[]) {
  const p = Bun.spawnSync(cmd, {
    env: {
      ...process.env,
      AGENT_QA_SCENARIOS_DIR: scenariosRoot,
      AGENT_QA_RECORD_DIR: recordRoot,
    },
  });
  results.push({
    name,
    command: cmd,
    exitCode: p.exitCode ?? -1,
    stdout: p.stdout.toString(),
    stderr: p.stderr.toString(),
  });
}

run("scenario check", [agentQa, "scenario", "check", scenarioFile]);
run("replay", [agentQa, "replay", sid, "--session", `${session}-replay`]);

const check = results[0];
const replay = results[1];
// `replay` exits nonzero on any failed step — the exit code is the verdict.
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
