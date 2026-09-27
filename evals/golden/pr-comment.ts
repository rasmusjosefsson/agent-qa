#!/usr/bin/env bun
/**
 * Reads the newest results/golden-all-*.json rollup and renders the PR
 * comment body the qa-gate workflow posts. Usage:
 *
 *   bun golden/pr-comment.ts --run-url <url> --out /tmp/qa-gate.md
 */
import { readFileSync, readdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const resultsDir = resolve(__dirname, "..", "results");

const args = process.argv.slice(2);
const flag = (name: string): string | undefined => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const runUrl = flag("--run-url") || "";
const out = flag("--out");

interface CaseResult {
  name: string;
  pass: boolean;
  exitCode: number | null;
  timedOut: boolean;
  durMs: number;
}
interface Rollup {
  startedAt: string;
  durMs: number;
  total: number;
  passed: number;
  failed: number;
  results: CaseResult[];
}

const rollups = readdirSync(resultsDir)
  .filter((f) => /^golden-all-.*\.json$/.test(f))
  .sort();
if (rollups.length === 0) {
  console.error("no golden-all-*.json rollup under evals/results/");
  process.exit(2);
}
const latest = JSON.parse(
  readFileSync(resolve(resultsDir, rollups[rollups.length - 1]), "utf8"),
) as Rollup;

const short = (name: string) => name.replace(/^golden:/, "");
const ok = latest.failed === 0;
const lines: string[] = [];
lines.push(`<!-- qa-gate -->`);
lines.push(
  `### QA gate — ${ok ? "all clear" : `${latest.failed} failure${latest.failed === 1 ? "" : "s"}`}`,
);
lines.push("");
lines.push(
  `${latest.passed}/${latest.total} fixture-backed golden cases passed in ${(latest.durMs / 60000).toFixed(1)}m${runUrl ? ` — [run](${runUrl})` : ""}`,
);
lines.push("");
lines.push("| Case | Result | Time |");
lines.push("| --- | --- | --- |");
for (const r of latest.results) {
  lines.push(
    `| \`${short(r.name)}\` | ${r.pass ? "PASS" : r.timedOut ? "TIMEOUT" : `FAIL (exit ${r.exitCode})`} | ${(r.durMs / 1000).toFixed(1)}s |`,
  );
}
lines.push("");
// For a failed case, walk its newest results dir and list the failing
// steps (sid · step id · intent · error) from each run's events.jsonl —
// reviewers see WHAT broke without downloading the artifact.
interface StepEvent {
  id?: string;
  idx?: number;
  intent?: string;
  status?: string;
  error?: string;
}
function failingSteps(caseName: string): string[] {
  try {
    const prefix = `golden-${short(caseName).replace(/[^a-z0-9]+/gi, "-")}-`;
    const dirs = readdirSync(resultsDir)
      .filter((d) => d.startsWith(prefix))
      .sort();
    const newest = dirs[dirs.length - 1];
    if (!newest) return [];
    const scenRoot = resolve(resultsDir, newest, "scenarios");
    const out: string[] = [];
    for (const sidDir of readdirSync(scenRoot)) {
      const replaysDir = resolve(scenRoot, sidDir, "replays");
      let runs: string[] = [];
      try {
        runs = readdirSync(replaysDir).sort();
      } catch {
        continue;
      }
      const runId = runs[runs.length - 1];
      const eventsFile = resolve(replaysDir, runId, "events.jsonl");
      let evLines: string[] = [];
      try {
        evLines = readFileSync(eventsFile, "utf8").split("\n").filter(Boolean);
      } catch {
        continue;
      }
      for (const line of evLines) {
        try {
          const ev = JSON.parse(line) as StepEvent;
          if (ev.status === "fail") {
            const id = ev.id ?? `s${ev.idx ?? "?"}`;
            const err = (ev.error ?? "").split("\n")[0].slice(0, 120);
            out.push(`\`${sidDir}\` ${id} ${ev.intent ?? ""} — ${err}`);
            if (out.length >= 5) return out;
          }
        } catch {
          /* skip malformed rows */
        }
      }
    }
    return out;
  } catch {
    return [];
  }
}

if (!ok) {
  for (const r of latest.results) {
    if (r.pass) continue;
    const steps = failingSteps(r.name);
    if (steps.length) {
      lines.push(`<details><summary><code>${short(r.name)}</code> — failing steps</summary>`);
      lines.push("");
      for (const s of steps) lines.push(`- ${s}`);
      lines.push("");
      lines.push("</details>");
      lines.push("");
    }
  }
  lines.push(
    "Download the `qa-gate` artifact on the run for each case's replay dir — `replays/<runId>/` has the screenshot, ARIA snapshot, and step events at the failing step.",
  );
  lines.push("");
}
lines.push(
  "<sub>Fixture-backed subset only (downloads + dialogs) — deterministic, no live-site flake. Full suite runs nightly.</sub>",
);

const body = lines.join("\n");
if (out) writeFileSync(out, body);
else console.log(body);
