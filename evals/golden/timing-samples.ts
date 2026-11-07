#!/usr/bin/env bun
/**
 * Turns the newest golden-all rollup into a timing-samples file for
 * `agent-qa timings check --samples`. Only passing cases are included:
 * a failure's duration is the timeout, not the cost.
 *
 *   bun golden/timing-samples.ts --results-dir evals/results --out /tmp/samples.json
 *   [--builds /tmp/timing-builds.txt]
 *
 * `--builds` is optional lines of `id=ms` (kind `build`), for example
 * `build:cli=96000`. A missing file is ignored. A failed build is simply
 * absent: its wall clock is not a duration.
 *
 * Exit 0 when a rollup was read (even if every case failed). Exit 2 when
 * no golden-all-*.json rollup exists.
 */
import { readFileSync, readdirSync, writeFileSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const args = process.argv.slice(2);
const flag = (name: string): string | undefined => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};

const resultsRoot = flag("--results-dir") || resolve(__dirname, "..", "results");
const buildsFile = flag("--builds");
const out = flag("--out");
if (!out) {
  console.error("usage: timing-samples.ts --out <file> [--results-dir <dir>]");
  process.exit(2);
}

interface CaseResult {
  name: string;
  pass: boolean;
  durMs: number;
}

let rollups: string[] = [];
try {
  rollups = readdirSync(resultsRoot)
    .filter((f) => /^golden-all-.*\.json$/.test(f))
    .sort();
} catch {
  rollups = [];
}
if (rollups.length === 0) {
  console.error(`no golden-all-*.json rollup under ${resultsRoot}`);
  process.exit(2);
}

const latest = JSON.parse(readFileSync(resolve(resultsRoot, rollups[rollups.length - 1]), "utf8")) as {
  results?: CaseResult[];
};

const entries = (latest.results ?? [])
  .filter((r) => r.pass && Number.isFinite(r.durMs) && r.durMs >= 0)
  .map((r) => ({
    id: `case:${r.name.replace(/^golden:/, "")}`,
    kind: "case",
    ms: Math.round(r.durMs),
  }));

if (buildsFile) {
  let text = "";
  try {
    text = readFileSync(buildsFile, "utf8");
  } catch {
    text = "";
  }
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    const id = trimmed.slice(0, eq).trim();
    const ms = Number(trimmed.slice(eq + 1).trim());
    if (!id || !Number.isFinite(ms) || ms < 0) continue;
    entries.push({ id, kind: "build", ms: Math.round(ms) });
  }
}

writeFileSync(
  out,
  JSON.stringify(
    {
      schema: "agent-qa-timing-samples/v1",
      entries,
    },
    null,
    2,
  ) + "\n",
);
console.error(`wrote ${entries.length} passing duration(s) to ${out}`);
