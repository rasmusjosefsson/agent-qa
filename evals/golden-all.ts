/**
 * Runs every `golden:*` script in evals/package.json sequentially and prints
 * a rollup. The suite self-enumerates: adding a `golden:*` script joins it
 * automatically. Sequential because each case spawns its own agent-browser
 * sessions; parallel runs on small CI runners only add flakes.
 *
 * Env:
 *   GOLDEN_TIMEOUT_MS — per-case cap in milliseconds (default 300000)
 *   GOLDEN_ONLY       — comma-separated substring filters on script name;
 *                       a case runs when it matches ANY entry (local
 *                       debugging + CI subsets, e.g. fixture-only gates)
 *
 * Exit: 0 when every case passed, 1 otherwise. A JSON rollup lands in
 * results/golden-all-<ts>.json next to each case's own golden-report.json.
 */

import { mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(resolve(__dirname, "package.json"), "utf8")) as {
  scripts: Record<string, string>;
};

const timeoutMs = Number(process.env.GOLDEN_TIMEOUT_MS || 300_000);
const only = (process.env.GOLDEN_ONLY || "")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);

const names = Object.keys(pkg.scripts)
  .filter((n) => n.startsWith("golden:") && n !== "golden:all")
  .filter((n) => only.length === 0 || only.some((o) => n.includes(o)))
  .sort();

if (names.length === 0) {
  console.error(`golden-all: no golden:* scripts matched${only.length ? ` (GOLDEN_ONLY=${only.join(",")})` : ""}`);
  process.exit(2);
}

interface CaseResult {
  name: string;
  pass: boolean;
  exitCode: number | null;
  timedOut: boolean;
  durMs: number;
}

const results: CaseResult[] = [];
const started = Date.now();

for (const name of names) {
  const t0 = Date.now();
  const proc = Bun.spawnSync(["bun", "run", name], {
    cwd: __dirname,
    stdout: "inherit",
    stderr: "inherit",
    timeout: timeoutMs,
  });
  const durMs = Date.now() - t0;
  const exitCode = proc.exitCode;
  const timedOut = exitCode == null && proc.signalCode != null;
  const pass = exitCode === 0;
  results.push({ name, pass, exitCode, timedOut, durMs });
  console.log(`${pass ? "PASS" : "FAIL"} ${name} (${(durMs / 1000).toFixed(1)}s${timedOut ? ", timed out" : ""})`);
}

const passed = results.filter((r) => r.pass);
const failed = results.filter((r) => !r.pass);
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const outDir = resolve(__dirname, "results");
mkdirSync(outDir, { recursive: true });
const outFile = resolve(outDir, `golden-all-${stamp}.json`);
writeFileSync(
  outFile,
  JSON.stringify(
    {
      startedAt: new Date(started).toISOString(),
      durMs: Date.now() - started,
      timeoutMs,
      total: results.length,
      passed: passed.length,
      failed: failed.length,
      results,
      filter: only.length ? only : undefined,
    },
    null,
    2,
  ),
);

console.log(`\n${passed.length}/${results.length} golden cases passed in ${((Date.now() - started) / 60000).toFixed(1)}m`);
if (failed.length) {
  console.log("failures:");
  for (const f of failed) console.log(`  ${f.name} (exit ${f.exitCode ?? "timeout"})`);
}
console.log(`rollup: ${outFile}`);
process.exit(failed.length ? 1 : 0);
