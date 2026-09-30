// gestures-tc03 — authored golden for the `rotate` verb: a two-finger
// rotate gesture on evals/fixtures/gestures.html's #rotateZone, which
// measures the delta between the two-touch line at touchstart and
// touchend. Writes the scenario/2 doc, runs `scenario check` + `replay`,
// emits golden-report.json.

import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "gestures-tc03";
const intent = "rotate gestures reach the page's two-touch angle handler";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-gestures-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;
const fixtureUrl = pathToFileURL(resolve(evalsRoot, "fixtures", "gestures.html")).href;

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
      intent: "open the gestures fixture",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: fixtureUrl },
    },
    {
      id: "s1",
      intent: "rotate 90deg clockwise on the zone",
      kind: "do",
      verb: "rotate",
      on: { raw: { kind: "css", value: "#rotateZone" }, reason: "rotate target" },
      params: { degrees: 90, radius: 60 },
    },
    {
      id: "s2",
      intent: "capture the rotate outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#rotateState" }, reason: "rotate state div" },
      saveAs: "rotCw",
    },
    {
      id: "s3",
      intent: "clockwise ~90deg registered on the two-touch angle delta",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "rotCw" },
        predicate: "matches",
        value: "^cw:(8[5-9]|9[0-5])$",
      },
    },
    {
      id: "s4",
      intent: "rotate 45deg counter-clockwise (negative degrees)",
      kind: "do",
      verb: "rotate",
      on: { raw: { kind: "css", value: "#rotateZone" }, reason: "rotate target" },
      params: { degrees: -45, radius: 60 },
    },
    {
      id: "s5",
      intent: "capture the counter-clockwise outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#rotateState" }, reason: "rotate state div" },
      saveAs: "rotCcw",
    },
    {
      id: "s6",
      intent: "counter-clockwise ~45deg registered",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "rotCcw" },
        predicate: "matches",
        value: "^ccw:4[0-9]$",
      },
    },
    {
      id: "s7",
      intent: "viewport-origin rotate (no `on` locator)",
      kind: "do",
      verb: "rotate",
      params: { degrees: 180 },
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
