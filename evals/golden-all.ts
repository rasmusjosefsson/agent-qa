/**
 * Runs every `golden:*` script in evals/package.json sequentially and prints
 * a rollup. The suite self-enumerates: adding a `golden:*` script joins it
 * automatically. Sequential because each case spawns its own agent-browser
 * sessions; parallel runs on small CI runners only add flakes.
 *
 * Suite-level aggregators (a script that itself runs other golden cases)
 * must NOT use the `golden:` prefix — each case already runs once here, so
 * an aggregator would double-run its cases inside one GOLDEN_TIMEOUT_MS
 * budget and always die mid-suite. Name them `eval:*` instead
 * (e.g. `eval:automation-exercise-all`).
 *
 * Env:
 *   GOLDEN_TIMEOUT_MS — per-case cap in milliseconds (default 300000)
 *   GOLDEN_ONLY       — comma-separated substring filters on script name;
 *                       a case runs when it matches ANY entry (local
 *                       debugging + CI subsets, e.g. fixture-only gates)
 *   GOLDEN_SHARD      — "i/n" runs every n-th case of the sorted list
 *                       starting at index i-1 (CI matrix fan-out; the
 *                       full suite is hours long — one job can never
 *                       finish it in a single runner slot)
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

const allNames = Object.keys(pkg.scripts)
  .filter((n) => n.startsWith("golden:") && n !== "golden:all")
  .filter((n) => only.length === 0 || only.some((o) => n.includes(o)))
  .sort();

let shardSpec = "";
let names = allNames;
const shardMatch = (process.env.GOLDEN_SHARD || "").match(/^(\d+)\s*\/\s*(\d+)$/);
if (shardMatch) {
  const i = Number(shardMatch[1]);
  const n = Number(shardMatch[2]);
  if (i < 1 || n < 1 || i > n) {
    console.error(`golden-all: invalid GOLDEN_SHARD=${process.env.GOLDEN_SHARD} — want "i/n" with 1<=i<=n`);
    process.exit(2);
  }
  shardSpec = `${i}/${n}`;
  names = allNames.filter((_, idx) => idx % n === i - 1);
}

if (names.length === 0) {
  console.error(`golden-all: no golden:* scripts matched${only.length ? ` (GOLDEN_ONLY=${only.join(",")})` : ""}${shardSpec ? ` (GOLDEN_SHARD=${shardSpec})` : ""}`);
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
      shard: shardSpec || undefined,
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
