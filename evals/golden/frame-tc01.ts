// frame-tc01 — authored golden for the `frame` verb: drive a scenario
// inside TinyMCE's iframe on the-internet.herokuapp.com/iframe.
// Self-contained (this branch predates edge-pages-lib): writes the
// scenario/2 doc, runs `scenario check` + `replay`, emits
// golden-report.json under results/.

import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "frame-tc01";
const intent = "drive steps inside an iframe and back out";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-frame-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;

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
      intent: "open the iframe demo page",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: "https://the-internet.herokuapp.com/iframe" },
    },
    {
      id: "s1",
      intent: "editor iframe rendered",
      kind: "check",
      claim: {
        subject: {
          element: { raw: { kind: "css", value: "#mce_0_ifr" }, reason: "tinymce host iframe" },
        },
        predicate: "exists",
      },
    },
    {
      id: "s2",
      intent: "enter the editor iframe",
      kind: "do",
      verb: "frame",
      params: { selector: "#mce_0_ifr" },
    },
    {
      id: "s3",
      intent: "read the editor body inside the frame",
      kind: "do",
      verb: "read",
      on: { raw: { kind: "css", value: "#tinymce" }, reason: "tinymce body" },
      saveAs: "editorText",
    },
    {
      id: "s4",
      intent: "default editor copy is present inside the frame",
      kind: "check",
      claim: {
        subject: { kind: "var", name: "editorText" },
        predicate: "contains",
        value: "Your content goes here.",
      },
    },
    {
      id: "s5",
      intent: "element claims resolve inside the frame",
      kind: "check",
      claim: {
        subject: {
          element: { raw: { kind: "css", value: "#tinymce p" }, reason: "editor paragraph" },
        },
        predicate: "exists",
      },
    },
    {
      id: "s6",
      intent: "back to the top document",
      kind: "do",
      verb: "frame",
      params: { main: true },
    },
    {
      id: "s7",
      intent: "top-document claims work again",
      kind: "check",
      claim: {
        subject: {
          element: {
            raw: { kind: "css", value: ".example h3" },
            reason: "page heading — present on the top doc, absent inside the frame",
          },
        },
        predicate: "isVisible",
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
