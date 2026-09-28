/**
 * Selftest lib — boots the real workbench (`agent-qa web`) and replays the
 * committed scenarios under evals/selftest/scenarios against it, so the
 * workbench's own UI is covered by the same shot-claim golden loop we ship.
 *
 *   bun evals/selftest/run.ts            — verify: replay every scenario
 *   bun evals/selftest/accept.ts         — re-mint baselines (after an
 *                                          intentional UI change; commit the
 *                                          updated scenarios/<sid>/baselines/)
 *
 * The committed scenario dirs hold scenario.json + baselines/<step>.png;
 * replays/ output under them is gitignored.
 */

import { existsSync, mkdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
export const selftestRoot = resolve(__dirname);
export const scenariosRoot = resolve(selftestRoot, "scenarios");
export const repoRoot = resolve(selftestRoot, "..", "..");
export const workbenchPort = Number(process.env.SELFTEST_PORT || 4899);
export const workbenchBase = `http://127.0.0.1:${workbenchPort}`;

export const agentQa = existsSync(resolve(repoRoot, "cli/target/debug/agent-qa"))
  ? resolve(repoRoot, "cli/target/debug/agent-qa")
  : "agent-qa";
export const agentBrowser =
  process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN || "agent-browser";

export function env(extra: Record<string, string> = {}): Record<string, string> {
  return {
    ...(process.env as Record<string, string>),
    AGENT_QA_SCENARIOS_DIR: scenariosRoot,
    AGENT_QA_BINARY_PATH: agentQa,
    AGENT_BROWSER_BIN: agentBrowser,
    NO_COLOR: "1",
    ...extra,
  };
}

async function sh(cmd: string[], e: Record<string, string>, name: string): Promise<string> {
  console.error(`[selftest] ${name}`);
  const proc = Bun.spawn(cmd, { cwd: repoRoot, env: e, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (exitCode !== 0) {
    throw new Error(`${name} failed (${exitCode})\n${cmd.join(" ")}\n${stdout}\n${stderr}`);
  }
  return stdout;
}

/** Build the workbench bundle if it's missing (first run / CI). */
async function ensureWebBundle(e: Record<string, string>): Promise<void> {
  const index = resolve(repoRoot, "npm/agent-qa/lib/public/index.html");
  if (!existsSync(index)) {
    await sh(
      ["npm", "--prefix", "npm/agent-qa", "run", "build:web"],
      e,
      "build web bundle",
    );
  }
}

/** Spawn `agent-qa web` bound to the selftest scenarios root. */
export async function bootWorkbench(): Promise<{ base: string; stop: () => void }> {
  const e = env();
  await ensureWebBundle(e);
  mkdirSync(scenariosRoot, { recursive: true });
  const proc = Bun.spawn(
    [
      "node",
      "npm/agent-qa/bin/agent-qa.js",
      "web",
      "--port",
      String(workbenchPort),
      "--root",
      scenariosRoot,
      "--no-open",
    ],
    { cwd: repoRoot, env: e, stdout: "pipe", stderr: "pipe" },
  );
  const deadline = Date.now() + 20_000;
  for (;;) {
    try {
      const r = await fetch(workbenchBase + "/", { signal: AbortSignal.timeout(1000) });
      if (r.ok) break;
    } catch {
      /* not up yet */
    }
    if (Date.now() > deadline) {
      proc.kill();
      throw new Error(`workbench did not come up on ${workbenchBase}`);
    }
    await Bun.sleep(250);
  }
  return { base: workbenchBase, stop: () => proc.kill() };
}

export async function replay(sid: string, e: Record<string, string>): Promise<void> {
  await sh(
    [agentQa, "replay", sid, "--session", `selftest-${sid}`],
    e,
    `replay ${sid}`,
  );
}

/** Lint a scenario (schema + rules). Returns the failure output or null. */
export async function lint(sid: string, e: Record<string, string>): Promise<string | null> {
  const scenarioPath = resolve(scenariosRoot, sid, "scenario.json");
  try {
    await sh([agentQa, "scenario", "check", scenarioPath], e, `lint ${sid}`);
    return null;
  } catch (err) {
    return String(err);
  }
}

/** Re-mint every shot baseline for a scenario from its latest run. */
export async function accept(sid: string, e: Record<string, string>): Promise<void> {
  await sh([agentQa, "shot-accept", sid, "--json"], e, `shot-accept ${sid}`);
}

export function scenarioIds(): string[] {
  const { readdirSync } = require("fs");
  return readdirSync(scenariosRoot, { withFileTypes: true })
    .filter((d) => d.isDirectory() && existsSync(resolve(scenariosRoot, d.name, "scenario.json")))
    .map((d) => d.name)
    .sort();
}

export function report(name: string, rows: { sid: string; ok: boolean; error?: string }[]): void {
  const resultsDir = resolve(selftestRoot, "results");
  mkdirSync(resultsDir, { recursive: true });
  const stamped = { name, at: new Date().toISOString(), rows };
  writeFileSync(
    resolve(resultsDir, `${name}-${stamped.at.replace(/[:.]/g, "-")}.json`),
    JSON.stringify(stamped, null, 2),
  );
  // Stable path for CI: the artifact upload + PR comment read this.
  writeFileSync(resolve(resultsDir, "latest.json"), JSON.stringify(stamped, null, 2));
}

/** shot-diff maps + run screenshots produced by a scenario's latest replay —
 *  the before/after evidence CI uploads and the PR comment links to. */
export function runArtifacts(sid: string): { runId: string; shots: string[]; diffs: string[] } | null {
  const { readdirSync } = require("fs");
  const replaysDir = resolve(scenariosRoot, sid, "replays");
  if (!existsSync(replaysDir)) return null;
  const runId = readdirSync(replaysDir, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort()
    .pop();
  if (!runId) return null;
  const runDir = resolve(replaysDir, runId);
  const ls = (sub: string) =>
    existsSync(resolve(runDir, sub))
      ? readdirSync(resolve(runDir, sub))
          .filter((f: string) => f.endsWith(".png"))
          .map((f: string) => `${sub}/${f}`)
      : [];
  return { runId, shots: ls("screenshots"), diffs: ls("shots-diff") };
}
