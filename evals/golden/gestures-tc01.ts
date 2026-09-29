// gestures-tc01 — authored golden for the `hold` and `swipe` verbs:
// synthesized pointer/touch gestures on evals/fixtures/gestures.html.
// Self-contained (this branch predates edge-pages-lib): writes the
// scenario/2 doc, runs `scenario check` + `replay`, emits
// golden-report.json under results/.

import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "gestures-tc01";
const intent = "hold and swipe gestures reach the page's touch handlers";
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
      intent: "hold the button for 600ms",
      kind: "do",
      verb: "hold",
      on: { raw: { kind: "css", value: "#holdBtn" }, reason: "hold target" },
      params: { ms: 600 },
    },
    {
      id: "s2",
      intent: "capture the hold outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#holdState" }, reason: "hold state div" },
      saveAs: "holdText",
    },
    {
      id: "s3",
      intent: "a 600ms hold registers HELD and never CLICK (no click event on release)",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "holdText" },
        predicate: "equals",
        value: "HELD",
      },
    },
    {
      id: "s4",
      intent: "swipe left across the zone",
      kind: "do",
      verb: "swipe",
      on: { raw: { kind: "css", value: "#swipeZone" }, reason: "swipe strip" },
      params: { direction: "left", distance: 160 },
    },
    {
      id: "s5",
      intent: "capture the swipe outcome",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#swipeState" }, reason: "swipe state div" },
      saveAs: "swipeText",
    },
    {
      id: "s6",
      intent: "swipe registered left at ~160px",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "swipeText" },
        predicate: "matches",
        value: "^left:1[0-9]{2}$",
      },
    },
    {
      id: "s7",
      intent: "viewport-origin swipe up (no `on` locator)",
      kind: "do",
      verb: "swipe",
      params: { direction: "up", distance: 120 },
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
