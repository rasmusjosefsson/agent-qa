// a11y-tc01 — authored golden for the `a11y` claim subject (#240): axe-core
// violation gating against a fixture page with two planted violations
// (image-alt = critical, color-contrast = serious) and one clean subtree.
// Writes the scenario/2 doc, runs `scenario check` + `replay`, emits
// golden-report.json under results/.

import { mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "a11y-tc01";
const intent = "a11y claims gate on planted axe violations";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-a11y-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;
const fixture = `file://${resolve(evalsRoot, "fixtures", "a11y.html")}`;

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
      intent: "open the a11y fixture",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: fixture },
    },
    {
      id: "s1",
      intent: "the page has violations at all",
      kind: "check",
      claim: { subject: { a11y: true }, predicate: "exists" },
    },
    {
      id: "s2",
      intent: "rule filter: the planted img has no alt",
      kind: "check",
      claim: { subject: { a11y: { rule: "image-alt" } }, predicate: "exists" },
    },
    {
      id: "s3",
      intent: "impact floor counts critical+serious (image-alt, color-contrast)",
      kind: "check",
      claim: { subject: { a11y: { impact: "serious" } }, predicate: "countEquals", value: 2 },
    },
    {
      id: "s4",
      intent: "critical floor counts only image-alt",
      kind: "check",
      claim: { subject: { a11y: { impact: "critical" } }, predicate: "countEquals", value: 1 },
    },
    {
      id: "s5",
      intent: "the clean subtree passes",
      kind: "check",
      claim: { subject: { a11y: { within: "#good" } }, predicate: "notExists" },
    },
    {
      id: "s6",
      intent: "the violating subtree scopes correctly",
      kind: "check",
      claim: { subject: { a11y: { within: "#bad" } }, predicate: "exists" },
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
