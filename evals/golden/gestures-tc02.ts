// gestures-tc02 — authored golden for the `pinch` verb: two-finger
// pinch gestures on evals/fixtures/gestures.html (touch path plus the
// ctrl-wheel desktop trackpad convention). Writes the scenario/2 doc,
// runs `scenario check` + `replay`, emits golden-report.json.

import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "gestures-tc02";
const intent = "pinch in/out gestures reach the page's pinch handlers";
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
      intent: "pinch out on the zone (fingers apart → zoom in)",
      kind: "do",
      verb: "pinch",
      on: { raw: { kind: "css", value: "#pinchZone" }, reason: "pinch target" },
      params: { direction: "out", distance: 200 },
    },
    {
      id: "s2",
      intent: "capture the pinch touch outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#pinchState" }, reason: "pinch state div" },
      saveAs: "pinchOut",
    },
    {
      id: "s3",
      intent: "pinch-out registered on the two-touch span delta",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "pinchOut" },
        predicate: "matches",
        value: "^out:[0-9]+$",
      },
    },
    {
      id: "s4",
      intent: "capture the ctrl-wheel pinch outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#pinchWheel" }, reason: "pinch wheel state" },
      saveAs: "pinchWheelOut",
    },
    {
      id: "s5",
      intent: "ctrl-wheel convention also fired (desktop trackpad pinch)",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "pinchWheelOut" },
        predicate: "equals",
        value: "wheel-out",
      },
    },
    {
      id: "s6",
      intent: "pinch in on the zone (fingers together → zoom out)",
      kind: "do",
      verb: "pinch",
      on: { raw: { kind: "css", value: "#pinchZone" }, reason: "pinch target" },
      params: { direction: "in", distance: 160 },
    },
    {
      id: "s7",
      intent: "capture the pinch touch outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#pinchState" }, reason: "pinch state div" },
      saveAs: "pinchIn",
    },
    {
      id: "s8",
      intent: "pinch-in registered on the two-touch span delta",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "pinchIn" },
        predicate: "matches",
        value: "^in:[0-9]+$",
      },
    },
    {
      id: "s9",
      intent: "capture the ctrl-wheel pinch-in outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#pinchWheel" }, reason: "pinch wheel state" },
      saveAs: "pinchWheelIn",
    },
    {
      id: "s10",
      intent: "ctrl-wheel pinch-in also fired",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "pinchWheelIn" },
        predicate: "equals",
        value: "wheel-in",
      },
    },
    {
      id: "s11",
      intent: "viewport-origin pinch in (no `on` locator)",
      kind: "do",
      verb: "pinch",
      params: { direction: "in" },
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
const report = {
  tc,
  intent,
  passed: check.exitCode === 0 && replay.exitCode === 0,
  steps: results,
};
writeFileSync(resolve(resultRoot, "golden-report.json"), JSON.stringify(report, null, 2));

console.log(`=== ${tc}: ${intent}`);
for (const r of results) {
  console.log(`--- ${r.name}: exit ${r.exitCode}`);
  if (r.exitCode !== 0) {
    console.log(r.stdout.trim());
    console.log(r.stderr.trim());
  }
}
if (!report.passed) process.exit(1);
if (!existsSync(resolve(resultRoot, "golden-report.json"))) {
  console.error("golden-report.json missing");
  process.exit(1);
}
console.log("golden-report.json written:", resolve(resultRoot, "golden-report.json"));
