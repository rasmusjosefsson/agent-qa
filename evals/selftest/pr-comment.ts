#!/usr/bin/env bun
/**
 * Render the ui-goldens PR comment from evals/selftest/results/latest.json.
 * Writes markdown to stdout (or --out <file>). The comment carries the
 * per-scenario verdict + which steps' shots missed their baseline — the
 * diff maps themselves land in the workflow artifact this comment links to.
 *
 *   bun evals/selftest/pr-comment.ts --run-url <url> --artifact <name> --out /tmp/comment.md
 */
import { existsSync, readFileSync, writeFileSync } from "fs";
import { resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

function arg(name: string): string | null {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] || null : null;
}

const latestPath = resolve(__dirname, "results", "latest.json");
const runUrl = arg("run-url") || "";
const artifactName = arg("artifact") || "ui-goldens";

let lines: string[];
if (!existsSync(latestPath)) {
  lines = [
    "### agent-qa UI goldens",
    "",
    `⚠️ selftest produced no report — the suite failed to run. See the [workflow log](${runUrl}).`,
  ];
} else {
  const data = JSON.parse(readFileSync(latestPath, "utf8"));
  const rows = data.rows || [];
  const failed = rows.filter((r: { ok: boolean }) => !r.ok);
  lines = ["### agent-qa UI goldens", ""];
  lines.push(
    failed.length === 0
      ? `✅ ${rows.length}/${rows.length} workbench goldens pass against this build.`
      : `❌ ${failed.length}/${rows.length} workbench goldens differ from baseline.`,
  );
  lines.push("");
  lines.push("| scenario | result |");
  lines.push("| --- | --- |");
  for (const r of rows) lines.push(`| \`${r.sid}\` | ${r.ok ? "PASS" : "FAIL"} |`);
  if (failed.length) {
    lines.push("");
    lines.push(
      "Diff maps (`shots-diff/*.diff.png`, red = changed pixels) and the new screenshots are in the " +
        `[\`${artifactName}\` artifact](${runUrl}) of this run.`,
    );
    lines.push("");
    lines.push(
      "If the change is intentional, re-mint baselines: `bun evals/selftest/accept.ts`, then commit the updated `evals/selftest/scenarios/*/baselines/`.",
    );
  }
}

const out = arg("out");
const md = lines.join("\n") + "\n";
if (out) writeFileSync(out, md);
else process.stdout.write(md);
