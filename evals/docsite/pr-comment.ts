#!/usr/bin/env bun
/**
 * Render the docs-goldens PR comment from evals/docsite/results/latest.json.
 * Writes markdown to stdout (or --out <file>). The comment carries the
 * per-scenario verdict; when --diff-base <raw-url> is passed (the workflow
 * publishes the failing diffs to a scratch branch), each failed shot embeds
 * its baseline / current / delta-map images inline.
 *
 *   bun evals/docsite/pr-comment.ts --run-url <url> --diff-base <url> --out /tmp/comment.md
 */
import { existsSync, readFileSync, readdirSync, writeFileSync } from "fs";
import { basename, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

function arg(name: string): string | null {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] || null : null;
}

const latestPath = resolve(__dirname, "results", "latest.json");
const runUrl = arg("run-url") || "";
const artifactName = arg("artifact") || "docs-goldens";
const diffBase = (arg("diff-base") || "").replace(/\/+$/, "");
// The scratch branch also carries evals/diff-viewer/viewer.html — serve it
// through a raw-HTML preview so the comment can link a live compare UI.
// htmlpreview renders inline (raw.githack shows a click-through interstitial).
const viewerBase = `https://htmlpreview.github.io/?${diffBase}`;

/** Failed shot diffs for one scenario's latest run: `shots-diff/<step>.diff.png`. */
function failedShots(sid: string): string[] {
  const dir = resolve(__dirname, "scenarios", sid);
  let runId = "";
  try {
    runId = readFileSync(resolve(dir, "replays", "latest.txt"), "utf8").trim();
  } catch {
    return [];
  }
  const diffDir = resolve(dir, "replays", runId, "shots-diff");
  if (!existsSync(diffDir)) return [];
  return readdirSync(diffDir)
    .filter((f) => f.endsWith(".diff.png"))
    .map((f) => basename(f, ".diff.png"))
    .sort();
}

function img(url: string, alt: string): string {
  return `![${alt}](${url})`;
}

let lines: string[];
if (!existsSync(latestPath)) {
  lines = [
    "### agent-qa docs goldens",
    "",
    `⚠️ docsite produced no report — the suite failed to run. See the [workflow log](${runUrl}).`,
  ];
} else {
  const data = JSON.parse(readFileSync(latestPath, "utf8"));
  const rows: { sid: string; ok: boolean; error?: string }[] = data.rows || [];
  const failed = rows.filter((r) => !r.ok);
  lines = ["### agent-qa docs goldens", ""];
  lines.push(
    failed.length === 0
      ? `✅ ${rows.length}/${rows.length} docs goldens pass against this build.`
      : `❌ ${failed.length}/${rows.length} docs goldens differ from baseline.`,
  );
  lines.push("");
  lines.push("| scenario | result |");
  lines.push("| --- | --- |");
  for (const r of rows) lines.push(`| \`${r.sid}\` | ${r.ok ? "PASS" : "FAIL"} |`);
  if (failed.length) {
    lines.push("");
    if (diffBase) {
      // Baseline | current | delta — the triplet a reviewer needs to decide
      // "intended change → accept" vs "regression → fix".
      for (const r of failed) {
        const shots = failedShots(r.sid);
        for (const step of shots) {
          lines.push(`<details><summary><code>${r.sid}</code> — <code>${step}</code> differs from baseline</summary>`);
          lines.push("");
          lines.push("| baseline | this PR | diff map |");
          lines.push("| --- | --- | --- |");
          const base = `${diffBase}/${r.sid}/${step}`;
          lines.push(
            `| ${img(`${base}.baseline.png`, "baseline")} | ${img(`${base}.current.png`, "this PR")} | ${img(`${base}.diff.png`, "diff")} |`,
          );
          const viewer = `${viewerBase}/viewer.html?b=${base}.baseline.png&c=${base}.current.png&d=${base}.diff.png&sid=${r.sid}&step=${step}`;
          lines.push(`[open diff viewer](${viewer}) — wipe, blink, zoom, blend`);
          lines.push("");
          lines.push("</details>");
          lines.push("");
        }
      }
      if (failed.every((r) => failedShots(r.sid).length === 0)) {
        lines.push(
          "No shot diffs were captured — the failures are step errors, not pixel drift. " +
            `Full run evidence is in the [\`${artifactName}\` artifact](${runUrl}).`,
        );
        lines.push("");
      }
    } else {
      lines.push(
        "Diff maps (`shots-diff/*.diff.png`, red = changed pixels) and the new screenshots are in the " +
          `[\`${artifactName}\` artifact](${runUrl}) of this run.`,
      );
      lines.push("");
    }
    lines.push(
      "If the change is intentional, re-mint baselines: comment `/docs-goldens accept` on this PR " +
        "(mints on this branch with CI rendering), or run `bun evals/docsite/accept.ts` and commit " +
        "`evals/docsite/scenarios/*/baselines/` — baselines are environment-bound, so CI-minted is preferred.",
    );
  }
}

const out = arg("out");
const md = lines.join("\n") + "\n";
if (out) writeFileSync(out, md);
else process.stdout.write(md);
