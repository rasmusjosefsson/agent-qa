// idb-tc01 — authored golden for the `indexeddb` claim subject + the
// `do/state` indexeddb seed key: seed two object stores (keyPath records +
// out-of-line {key,value}), assert records, paths, and store presence, then
// prove persistence across a reload. Runs `scenario check` + `replay`,
// emits golden-report.json under results/.

import { mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "idb-tc01";
const intent = "do/state seeds IndexedDB; indexeddb claims assert records across a reload";
const runId = new Date().toISOString().replace(/[:.]/g, "-") + "Z";
const resultRoot = resolve(evalsRoot, "results", `golden-idb-${tc}-${runId}`);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = process.env.AGENT_QA_BIN ?? resolve(repoRoot, "cli/target/debug/agent-qa");
const session = `golden-${tc}`;
const page = "https://example.com/";

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
      intent: "open a page to host the seed",
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: page },
    },
    {
      id: "s1",
      intent: "seed a keyPath store and an out-of-line-key store",
      kind: "do",
      verb: "state",
      params: {
        indexeddb: [
          {
            db: "aq-cart",
            store: "items",
            keyPath: "sku",
            put: [
              { sku: "sku-1", qty: 2, title: "red mug" },
              { sku: "sku-2", qty: 1 },
            ],
          },
          {
            db: "aq-flags",
            store: "out",
            put: [{ key: "onboarded", value: { done: true, step: 3 } }],
          },
        ],
      },
    },
    {
      id: "s2",
      intent: "record exists in the keyPath store",
      kind: "check",
      claim: {
        subject: { indexeddb: { db: "aq-cart", store: "items", key: "sku-1" } },
        predicate: "exists",
      },
    },
    {
      id: "s3",
      intent: "path walks the stored record",
      kind: "check",
      claim: {
        subject: {
          indexeddb: { db: "aq-cart", store: "items", key: "sku-1" },
          path: "$.qty",
        },
        predicate: "equals",
        value: 2,
      },
    },
    {
      id: "s4",
      intent: "string predicate on a field",
      kind: "check",
      claim: {
        subject: {
          indexeddb: { db: "aq-cart", store: "items", key: "sku-1" },
          path: "$.title",
        },
        predicate: "contains",
        value: "mug",
      },
    },
    {
      id: "s5",
      intent: "out-of-line key reads back",
      kind: "check",
      claim: {
        subject: {
          indexeddb: { db: "aq-flags", store: "out", key: "onboarded" },
          path: "$.step",
        },
        predicate: "equals",
        value: 3,
      },
    },
    {
      id: "s6",
      intent: "store itself exists (no key in the matcher)",
      kind: "check",
      claim: {
        subject: { indexeddb: { db: "aq-flags", store: "out" } },
        predicate: "exists",
      },
    },
    {
      id: "s7",
      intent: "missing record reports absent",
      kind: "check",
      claim: {
        subject: { indexeddb: { db: "aq-cart", store: "items", key: "sku-404" } },
        predicate: "notExists",
      },
    },
    {
      id: "s8",
      intent: "missing db reports absent — the probe never creates it",
      kind: "check",
      claim: {
        subject: { indexeddb: { db: "aq-ghost", store: "items" } },
        predicate: "notExists",
      },
    },
    {
      id: "s9",
      intent: "reload the page",
      kind: "do",
      verb: "reload",
    },
    {
      id: "s10",
      intent: "records survive the reload",
      kind: "check",
      claim: {
        subject: {
          indexeddb: { db: "aq-cart", store: "items", key: "sku-2" },
          path: "$.qty",
        },
        predicate: "equals",
        value: 1,
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
