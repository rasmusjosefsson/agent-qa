#!/usr/bin/env bun
/**
 * Docs-site goldens accept — replay each committed scenario, then
 * `shot-accept` its run so the committed `baselines/` are re-minted. Run this
 * after an intentional docs-site change and commit the updated
 * `scenarios/<sid>/baselines/` — or let `/docs-goldens accept` do it on the
 * PR head, since baselines are environment-bound.
 */
import { accept, bootPreview, env, replay, report, scenarioIds } from "./lib.ts";

const e = env();
const { stop } = await bootPreview();
const rows: { sid: string; ok: boolean; error?: string }[] = [];
try {
  for (const sid of scenarioIds()) {
    try {
      // Replay is expected to fail while baselines drift — accept mints from
      // the run's screenshots regardless of claim outcomes.
      await replay(sid, e).catch(() => {});
      await accept(sid, e);
      rows.push({ sid, ok: true });
    } catch (err) {
      rows.push({ sid, ok: false, error: String(err) });
    }
  }
} finally {
  stop();
}
report("docsite-accept", rows);
for (const r of rows) console.log(`${r.ok ? "MINTED" : "FAIL"} ${r.sid}`);
process.exit(rows.every((r) => r.ok) && rows.length > 0 ? 0 : 1);
