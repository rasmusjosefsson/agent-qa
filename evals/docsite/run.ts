#!/usr/bin/env bun
/**
 * Docs-site goldens verify — replay every committed scenario under
 * evals/docsite/scenarios against a fresh astro build + preview and report
 * per-scenario pass/fail. Baselines must be minted first:
 * `bun evals/docsite/accept.ts` (or `/docs-goldens accept` on the PR).
 */
import { bootPreview, env, lint, replay, report, scenarioIds } from "./lib.ts";

const e = env();
const { stop } = await bootPreview();
const rows: { sid: string; ok: boolean; error?: string }[] = [];
try {
  for (const sid of scenarioIds()) {
    // Lint first: a malformed scenario fails with the lint finding rather
    // than an opaque mid-replay error.
    const lintErr = await lint(sid, e);
    if (lintErr) {
      rows.push({ sid, ok: false, error: lintErr });
      continue;
    }
    try {
      await replay(sid, e);
      rows.push({ sid, ok: true });
    } catch (err) {
      rows.push({ sid, ok: false, error: String(err) });
    }
  }
} finally {
  stop();
}
report("docsite", rows);
for (const r of rows) console.log(`${r.ok ? "PASS" : "FAIL"} ${r.sid}${r.error ? `\n${r.error.slice(0, 400)}` : ""}`);
process.exit(rows.every((r) => r.ok) && rows.length > 0 ? 0 : 1);
