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
// Overridable for tests — production reads evals/results.
const resultsRoot = flag("--results-dir") || resultsDir;

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

const rollups = readdirSync(resultsRoot)
  .filter((f) => /^golden-all-.*\.json$/.test(f))
  .sort();
if (rollups.length === 0) {
  console.error(`no golden-all-*.json rollup under ${resultsRoot}`);
  process.exit(2);
}
const latest = JSON.parse(
  readFileSync(resolve(resultsRoot, rollups[rollups.length - 1]), "utf8"),
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
// For a failed case, walk its newest results dir and build a mini digest of
// the run: failing steps (sid · step id · intent · error) from events.jsonl,
// page console errors from console.json, failed/pending requests from
// network.json, the heal count from heal.jsonl, and whether shot diffs were
// written. Reviewers see WHAT broke without downloading the artifact.
interface StepEvent {
  id?: string;
  idx?: number;
  intent?: string;
  status?: string;
  error?: string;
}
interface Digest {
  steps: string[];
  consoleErrors: string[];
  networkFails: string[];
  heals: number;
  shotDiffs: string[];
}
function readJsonl(file: string): unknown[] {
  try {
    return readFileSync(file, "utf8")
      .split("\n")
      .filter(Boolean)
      .map((l) => {
        try {
          return JSON.parse(l);
        } catch {
          return null;
        }
      })
      .filter((v) => v != null);
  } catch {
    return [];
  }
}
function caseDigest(caseName: string): Digest {
  const d: Digest = { steps: [], consoleErrors: [], networkFails: [], heals: 0, shotDiffs: [] };
  try {
    const prefix = `golden-${short(caseName).replace(/[^a-z0-9]+/gi, "-")}-`;
    const dirs = readdirSync(resultsRoot)
      .filter((x) => x.startsWith(prefix))
      .sort();
    const newest = dirs[dirs.length - 1];
    if (!newest) return d;
    const scenRoot = resolve(resultsRoot, newest, "scenarios");
    for (const sidDir of readdirSync(scenRoot)) {
      const replaysDir = resolve(scenRoot, sidDir, "replays");
      let runs: string[] = [];
      try {
        runs = readdirSync(replaysDir).sort();
      } catch {
        continue;
      }
      const runId = runs[runs.length - 1];
      const runDir = resolve(replaysDir, runId);
      for (const ev of readJsonl(resolve(runDir, "events.jsonl")) as StepEvent[]) {
        if (ev.status === "fail") {
          const id = ev.id ?? `s${ev.idx ?? "?"}`;
          const err = (ev.error ?? "").split("\n")[0].slice(0, 120);
          d.steps.push(`\`${sidDir}\` ${id} ${ev.intent ?? ""} — ${err}`);
          if (d.steps.length >= 5) break;
        }
      }
      try {
        const con = JSON.parse(readFileSync(resolve(runDir, "console.json"), "utf8")) as {
          messages?: { type?: string; text?: string }[];
        };
        for (const m of con.messages ?? []) {
          if (m.type === "error") {
            d.consoleErrors.push((m.text ?? "").split("\n")[0].slice(0, 140));
            if (d.consoleErrors.length >= 3) break;
          }
        }
      } catch {
        /* no console.json */
      }
      try {
        const net = JSON.parse(readFileSync(resolve(runDir, "network.json"), "utf8")) as {
          requests?: { method?: string; url?: string; status?: number | null }[];
        };
        for (const r of net.requests ?? []) {
          if (r.status == null || r.status >= 400) {
            const tail = (r.url ?? "").split("/").pop()?.split("?")[0] ?? "";
            d.networkFails.push(
              `${r.method ?? "GET"} …/${tail.slice(0, 80)} — ${r.status == null ? "no response" : r.status}`,
            );
            if (d.networkFails.length >= 3) break;
          }
        }
      } catch {
        /* no network.json */
      }
      d.heals += readJsonl(resolve(runDir, "heal.jsonl")).length;
      try {
        for (const f of readdirSync(resolve(runDir, "shots-diff"))) {
          if (f.endsWith(".png")) d.shotDiffs.push(`\`${sidDir}\` …/shots-diff/${f}`);
        }
      } catch {
        /* no shot diffs */
      }
    }
    return d;
  } catch {
    return d;
  }
}

if (!ok) {
  for (const r of latest.results) {
    if (r.pass) continue;
    const d = caseDigest(r.name);
    const parts =
      d.steps.length + d.consoleErrors.length + d.networkFails.length + d.shotDiffs.length > 0 ||
      d.heals > 0;
    if (parts) {
      lines.push(`<details><summary><code>${short(r.name)}</code> — failure digest</summary>`);
      lines.push("");
      for (const s of d.steps) lines.push(`- ${s}`);
      if (d.consoleErrors.length) {
        lines.push(`- console errors (${d.consoleErrors.length}):`);
        for (const e of d.consoleErrors) lines.push(`  - \`${e}\``);
      }
      if (d.networkFails.length) {
        lines.push(`- network failures (${d.networkFails.length}):`);
        for (const e of d.networkFails) lines.push(`  - \`${e}\``);
      }
      if (d.heals > 0) lines.push(`- ${d.heals} self-heal(s) — see heal.jsonl in the run artifact`);
      for (const s of d.shotDiffs) lines.push(`- shot diff: ${s}`);
      lines.push("");
      lines.push("</details>");
      lines.push("");
    }
  }
  lines.push(
    "Download the `qa-gate` artifact on the run for each case's replay dir — `replays/<runId>/` has the screenshot, ARIA snapshot, step events, console log, network log, and any shot-diff images at the failing step.",
  );
  lines.push("");
}
lines.push(
  "<sub>Fixture-backed subset only (downloads + dialogs) — deterministic, no live-site flake. Full suite runs nightly.</sub>",
);

const body = lines.join("\n");
if (out) writeFileSync(out, body);
else console.log(body);
