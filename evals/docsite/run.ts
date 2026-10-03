#!/usr/bin/env bun
/**
 * Docs-site goldens verify — replay every committed scenario under
 * evals/docsite/scenarios against a fresh astro build + preview and report
 * per-scenario pass/fail. Baselines must be minted first:
 * `bun evals/docsite/accept.ts` (or `/docs-goldens accept` on the PR).
 */
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { bootPreview, closeSessions, env, lint, replay, report, scenarioIds, scenariosRoot } from "./lib.ts";

// DOCS_ONLY (comma-separated page slugs like "docs/verbs,docs/overview") —
// replay only the step groups for those pages. A "group" is a `goto` step
// plus the check steps that follow it up to the next goto/state; every other
// step (viewport, landing goto, theme state) is kept so the environment sets
// up exactly as a full run. Step ids are preserved, so shot baselines still
// line up. Used by CI to skip unrelated pages on docs-only PRs.
const only = (process.env.DOCS_ONLY || "")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);

function pageSlug(step: Record<string, unknown>): string | null {
  if (step.verb !== "goto") return null;
  const literal = (step.value as Record<string, unknown> | undefined)?.literal;
  if (typeof literal !== "string") return null;
  const path = new URL(literal).pathname.replace(/\/+$/, "");
  if (path === "/agent-qa") return "index";
  return path.startsWith("/agent-qa/") ? path.slice("/agent-qa/".length) : null;
}

function filterScenario(sid: string): () => void {
  const file = resolve(scenariosRoot, sid, "scenario.json");
  const original = readFileSync(file, "utf8");
  const scenario = JSON.parse(original) as { steps: Record<string, unknown>[] };
  let sawPageGoto = false;
  let lastGotoKept = true;
  const steps = scenario.steps.filter((step) => {
    if (step.verb === "goto") {
      const slug = pageSlug(step);
      // The landing goto (first page goto before any theme state) sets up the
      // origin for `state` — always keep it; page gotos keep only their group.
      if (!sawPageGoto) {
        sawPageGoto = true;
        lastGotoKept = true;
        return true;
      }
      lastGotoKept = slug !== null && only.includes(slug);
      return lastGotoKept;
    }
    if (step.kind === "check") return lastGotoKept;
    return true;
  });
  writeFileSync(file, JSON.stringify({ ...scenario, steps }, null, 2) + "\n");
  console.log(`[docsite] filtered ${sid}: ${steps.length}/${scenario.steps.length} steps (DOCS_ONLY=${only.join(",")})`);
  return () => writeFileSync(file, original);
}

const e = env();
const { stop } = await bootPreview();
const rows: { sid: string; ok: boolean; error?: string }[] = [];
const sids = scenarioIds();
console.log(`[docsite] replaying ${sids.length} scenario(s)`);
try {
  for (const [i, sid] of sids.entries()) {
    console.log(`[docsite] ${i + 1}/${sids.length} lint+replay ${sid} ...`);
    // Lint first: a malformed scenario fails with the lint finding rather
    // than an opaque mid-replay error.
    const lintErr = await lint(sid, e);
    if (lintErr) {
      rows.push({ sid, ok: false, error: lintErr });
      console.log(`[docsite] ${i + 1}/${sids.length} ${sid} LINT FAIL`);
      continue;
    }
    const restore = only.length > 0 ? filterScenario(sid) : () => {};
    try {
      await replay(sid, e);
      rows.push({ sid, ok: true });
      console.log(`[docsite] ${i + 1}/${sids.length} ${sid} PASS`);
    } catch (err) {
      rows.push({ sid, ok: false, error: String(err) });
      console.log(`[docsite] ${i + 1}/${sids.length} ${sid} FAIL`);
    } finally {
      restore();
    }
  }
} finally {
  stop();
  await closeSessions(e);
}
report("docsite", rows);
for (const r of rows) console.log(`${r.ok ? "PASS" : "FAIL"} ${r.sid}${r.error ? `\n${r.error.slice(0, 400)}` : ""}`);
process.exit(rows.every((r) => r.ok) && rows.length > 0 ? 0 : 1);
