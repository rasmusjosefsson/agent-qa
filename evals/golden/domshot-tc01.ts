// domshot-tc01 — text-golden ARIA-snapshot claims end to end on
// the-internet's /login page: an authored scenario whose checks claim
// {"domshot"} baselines. The first replay must bail with the
// domshot-accept mint hint (no baselines yet — proves the failure path
// and the lint warning); domshot-accept mints from the run's snapshots
// sidecars; the second replay passes. Covers claim dispatch, baseline
// minting, the missing-baseline error, and normalization (@eN refs differ
// between the two replays by construction).
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const tc = "domshot-tc01";
const runId = `golden-domshot-${tc}-${new Date().toISOString().replace(/[:.]/g, "-")}`;
const resultRoot = resolve(evalsRoot, "results", runId);
const scenariosRoot = resolve(resultRoot, "scenarios");
const recordRoot = resolve(resultRoot, "record");
mkdirSync(scenariosRoot, { recursive: true });
mkdirSync(recordRoot, { recursive: true });

const agentQa = existsSync(resolve(repoRoot, "cli/target/debug/agent-qa"))
  ? resolve(repoRoot, "cli/target/debug/agent-qa")
  : "agent-qa";
const session = `golden-domshot-${tc}-${Math.random().toString(16).slice(2, 8)}`;
const env = {
  ...(process.env as Record<string, string>),
  AGENT_QA_SCENARIOS_DIR: scenariosRoot,
  AGENT_QA_RECORD_DIR: recordRoot,
  AGENT_QA_REPO_ROOT: repoRoot,
  AGENT_QA_RECORD_SKIP_SIDECARS: "1",
  AGENT_QA_AGENT_BROWSER_TIMEOUT_MS: process.env.AGENT_QA_AGENT_BROWSER_TIMEOUT_MS || "10000",
  NO_COLOR: "1",
};

interface StepResult { name: string; exitCode: number; stdout: string; stderr: string }
const results: StepResult[] = [];

async function run(name: string, command: string[]): Promise<StepResult> {
  console.error(`[golden] ${name}`);
  const proc = Bun.spawn(command, { cwd: repoRoot, env, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  const r = { name, exitCode, stdout, stderr };
  results.push(r);
  return r;
}

const css = (value: string) => ({
  raw: { kind: "css", value },
  reason: "authored domshot golden",
});
const lit = (literal: unknown) => ({ from: "literal", literal });

const sid = `s-${tc}-authored`;
const doc = {
  schema: "scenario/2",
  id: sid,
  intent: "domshot — ARIA-snapshot text baselines minted + matched",
  env: { open: [{ kind: "fresh" }] },
  producedBy: { producer: "llm-author", producedAt: new Date().toISOString() },
  steps: [
    {
      id: "s0",
      intent: "open the login page",
      kind: "do",
      verb: "goto",
      value: lit("https://the-internet.herokuapp.com/login"),
    },
    {
      id: "s1",
      intent: "login page structure matches the golden",
      kind: "check",
      claim: { subject: { domshot: "s0" }, predicate: "matches" },
    },
    {
      id: "s2",
      intent: "submit empty form → deterministic flash error",
      kind: "do",
      verb: "click",
      on: css("button[type='submit']"),
    },
    {
      id: "s3",
      intent: "post-submit structure matches the golden",
      kind: "check",
      claim: { subject: { domshot: "s2" }, predicate: "matches" },
    },
  ],
};
const sdir = resolve(scenariosRoot, sid);
mkdirSync(sdir, { recursive: true });
writeFileSync(resolve(sdir, "scenario.json"), JSON.stringify(doc, null, 2));

let pass = false;
let error = "";

try {
  // 1. lint should already warn about the missing domshot baselines
  //    (check's count-only path doesn't emit rule codes; lint does).
  const check = await run("check", [agentQa, "scenario", "check", resolve(sdir, "scenario.json")]);
  if (check.exitCode !== 0) {
    throw new Error(`scenario check failed\n${check.stdout}\n${check.stderr}`);
  }
  const lint = await run("lint", [agentQa, "scenario", "lint", resolve(sdir, "scenario.json")]);
  if (!(lint.stdout + lint.stderr).includes("domshot-without-baseline")) {
    throw new Error(`expected domshot-without-baseline lint warning\n${lint.stdout}\n${lint.stderr}`);
  }

  // 2. First replay bails on the missing baseline with the mint hint.
  //    --keep-going keeps the run alive past that failure so every step's
  //    snapshot sidecar lands for domshot-accept to mint from.
  const first = await run("replay (no baselines)", [agentQa, "replay", sid, "--keep-going", "--session", `${session}-replay`]);
  if (first.exitCode === 0) {
    throw new Error("replay unexpectedly passed without domshot baselines");
  }
  if (!(first.stdout + first.stderr).includes("domshot-accept")) {
    throw new Error(`missing-baseline bail lacks the domshot-accept hint\n${first.stdout}\n${first.stderr}`);
  }

  // 3. Mint baselines from that run's snapshot sidecars.
  const accept = await run("domshot-accept", [agentQa, "domshot-accept", sid]);
  if (accept.exitCode !== 0) {
    throw new Error(`domshot-accept failed\n${accept.stdout}\n${accept.stderr}`);
  }
  if (!existsSync(resolve(sdir, "baselines/s0.snap.txt"))) {
    throw new Error("domshot-accept did not write baselines/s0.snap.txt");
  }

  // 4. Re-replay: refs renumber between runs, so this also proves the
  //    @eN → @e normalization holds on real output.
  await run("replay (baselines minted)", [agentQa, "replay", sid, "--session", `${session}-replay2`]);
  pass = true;
} catch (err) {
  error = err instanceof Error ? err.message : String(err);
}

const report = { pass, runId, sid, resultRoot, scenariosRoot, session, error, results };
writeFileSync(resolve(resultRoot, "golden-report.json"), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
process.exit(pass ? 0 : 1);
