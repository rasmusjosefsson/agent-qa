#!/usr/bin/env bun
// crawl-tc01 — `agent-qa crawl` against a live multi-link page: the draft
// scenario validates (schema + lint), carries a shot + console-error claim
// per covered route, and replays green immediately off --mint-baselines.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const evalsRoot = resolve(__dirname, "..");
const repoRoot = resolve(evalsRoot, "..");

const runId = `golden-crawl-tc01-${new Date().toISOString().replace(/[:.]/g, "-")}`;
const resultRoot = resolve(evalsRoot, "results", runId);
const scenariosRoot = resolve(resultRoot, "scenarios");
const session = `golden-crawl-tc01-${Math.random().toString(16).slice(2, 8)}`;
const agentQa = existsSync(resolve(repoRoot, "cli/target/debug/agent-qa"))
  ? resolve(repoRoot, "cli/target/debug/agent-qa")
  : "agent-qa";
const agentBrowser = process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN || "agent-browser";
// the-internet's index is the target: ~44 same-origin links, static HTML,
// no XHR — deterministic surface for a bounded crawl.
const entryUrl = "https://the-internet.herokuapp.com/";
const sid = "crawl-tc01";
const MAX_LINKS = "3";

mkdirSync(scenariosRoot, { recursive: true });

const env: Record<string, string> = {
  ...(process.env as Record<string, string>),
  AGENT_QA_SCENARIOS_DIR: scenariosRoot,
  AGENT_BROWSER_BIN: agentBrowser,
  AGENT_QA_REPO_ROOT: repoRoot,
  AGENT_QA_AGENT_BROWSER_TIMEOUT_MS: process.env.AGENT_QA_AGENT_BROWSER_TIMEOUT_MS || "15000",
  NO_COLOR: "1",
};

const results: { name: string; exitCode: number; stdout: string; stderr: string }[] = [];

async function run(name: string, command: string[]): Promise<string> {
  console.error(`[golden] ${name}`);
  const proc = Bun.spawn(command, { cwd: repoRoot, env, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  results.push({ name, exitCode, stdout, stderr });
  if (exitCode !== 0) {
    throw new Error(`${name} failed (${exitCode})\n${command.join(" ")}\n${stdout}\n${stderr}`);
  }
  return stdout;
}

let pass = 0;
let fail = 0;
function check(label: string, ok: boolean, detail = "") {
  if (ok) {
    pass += 1;
    console.error(`  PASS ${label}`);
  } else {
    fail += 1;
    console.error(`  FAIL ${label} ${detail}`);
  }
}

try {
  await run("crawl", [
    agentQa,
    "crawl",
    entryUrl,
    "--session",
    session,
    "--sid",
    sid,
    "--out",
    resolve(scenariosRoot, sid),
    "--max",
    MAX_LINKS,
    "--depth",
    "1",
    "--mint-baselines",
  ]);

  const scenarioPath = resolve(scenariosRoot, sid, "scenario.json");
  check("scenario.json written", existsSync(scenarioPath));
  const scenario = JSON.parse(readFileSync(scenarioPath, "utf8"));
  const steps = scenario.steps as any[];

  const gotos = steps.filter((s) => s.verb === "goto");
  const shots = steps.filter(
    (s) => s.claim && s.claim.subject && s.claim.subject.shot !== undefined,
  );
  const consoleClaims = steps.filter(
    (s) => s.claim && s.claim.subject && s.claim.subject.console !== undefined,
  );
  const netClaims = steps.filter(
    (s) => s.claim && s.claim.subject && s.claim.subject.network !== undefined,
  );
  const routes = gotos.length;
  check("routes covered >= 2 (entry + links)", routes >= 2, `got ${routes}`);
  check("routes bounded by --max", routes <= 1 + Number(MAX_LINKS), `got ${routes}`);
  check("one shot claim per route", shots.length === routes, `${shots.length} vs ${routes}`);
  check("one console claim per route", consoleClaims.length === routes, `${consoleClaims.length} vs ${routes}`);
  check(
    "baselines minted per shot",
    shots.every((s) =>
      existsSync(resolve(scenariosRoot, sid, "baselines", `${s.claim.subject.shot}.png`)),
    ),
  );
  // The entry page fires an optimizely beacon — telemetry must never
  // become a fired claim (its URL is per-visitor and flakes on replay).
  check(
    "no telemetry claims",
    netClaims.every((s) => !/optimizely|google-analytics|sentry|segment/.test(JSON.stringify(s))),
    `${netClaims.length} net claims`,
  );

  await run("scenario check", [agentQa, "scenario", "check", scenarioPath]);

  const replayOut = await run("replay", [agentQa, "replay", sid]);
  check("draft replay exits 0", true);
  const auditDir = resolve(scenariosRoot, sid, "replays");
  check("run audit produced", existsSync(auditDir));
  const auditLine = replayOut.split("\n").find((l) => l.includes("pass"));
  console.error(`  replay: ${auditLine ?? replayOut.trim().split("\n").pop()}`);
} finally {
  await run("close session", [agentBrowser, "--session", session, "close"]).catch(() => "");
  mkdirSync(resultRoot, { recursive: true });
  writeFileSync(
    resolve(resultRoot, "steps.json"),
    JSON.stringify({ runId, pass, fail, results }, null, 2),
  );
  console.error(`[golden] crawl-tc01: ${pass} passed, ${fail} failed`);
}

if (fail > 0) process.exit(1);
