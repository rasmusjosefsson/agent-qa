#!/usr/bin/env bun
/**
 * Docs-site goldens verify — replay every committed scenario under
 * evals/docsite/scenarios against a fresh astro build + preview and report
 * per-scenario pass/fail.
 *
 * A missing baseline, and a shot whose page source changed in this pull
 * request (`DOCS_CHANGED_FILES`), is re-minted from the replay. A shot whose
 * source did not change still fails the run when its pixels drift. Shared
 * chrome is not re-minted here — that stays `/docs-goldens accept`.
 *
 * DOCS_ONLY (comma-separated page slugs like "docs/verbs,docs/overview")
 * replays only those page groups. Viewport, the landing goto, and theme
 * state steps stay, and step ids are preserved so baselines still line up.
 */
import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  accept,
  bootPreview,
  closeSessions,
  docsiteRoot,
  env,
  lintFindings,
  replay,
  report,
  scenarioIds,
  scenariosRoot,
} from "./lib.ts";
import {
  blockingFindings,
  changedFilesFromEnv,
  pageSlug,
  refreshShotIds,
  refreshShotsForReplay,
  shotSlugs,
  slugsForChangedFiles,
  terminalFailures,
  unexpectedFailures,
  type ShotStep,
  type StepEvent,
} from "./refresh.ts";

const only = (process.env.DOCS_ONLY || "")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);
const changedSlugs = slugsForChangedFiles(changedFilesFromEnv(process.env.DOCS_CHANGED_FILES));

function filterScenario(sid: string): () => void {
  const file = resolve(scenariosRoot, sid, "scenario.json");
  const original = readFileSync(file, "utf8");
  const scenario = JSON.parse(original) as { steps: ShotStep[] };
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

function readFailures(sid: string): Map<string, string> | null {
  try {
    const latest = readFileSync(resolve(scenariosRoot, sid, "replays", "latest.txt"), "utf8").trim();
    const text = readFileSync(resolve(scenariosRoot, sid, "replays", latest, "events.jsonl"), "utf8");
    const events = text
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line) as StepEvent);
    return terminalFailures(events);
  } catch {
    return null;
  }
}

function publishMinted(sid: string, ids: readonly string[]): void {
  const dest = resolve(docsiteRoot, "results", "minted", sid);
  mkdirSync(dest, { recursive: true });
  for (const id of ids) {
    copyFileSync(resolve(scenariosRoot, sid, "baselines", `${id}.png`), resolve(dest, `${id}.png`));
  }
  writeFileSync(resolve(dest, "ids.json"), JSON.stringify(ids));
}

type Row = { sid: string; ok: boolean; error?: string; refreshed?: string[] };

const e = env();
const { stop } = await bootPreview();
const rows: Row[] = [];
const sids = scenarioIds();
console.log(`[docsite] replaying ${sids.length} scenario(s)`);
if (changedSlugs.length) console.log(`[docsite] page source changed: ${changedSlugs.join(", ")}`);
try {
  for (const [i, sid] of sids.entries()) {
    console.log(`[docsite] ${i + 1}/${sids.length} lint+replay ${sid} ...`);
    const scenarioPath = resolve(scenariosRoot, sid, "scenario.json");
    const scenario = JSON.parse(readFileSync(scenarioPath, "utf8")) as { steps: ShotStep[] };
    const baselineNames = new Set<string>();
    try {
      for (const name of readdirSync(resolve(scenariosRoot, sid, "baselines"))) {
        if (name.endsWith(".png")) baselineNames.add(name);
      }
    } catch {
      // No baselines directory yet — every shot is missing.
    }
    const slugs = shotSlugs(scenario.steps);
    const refresh = refreshShotsForReplay(
      refreshShotIds(scenario.steps, changedSlugs, baselineNames),
      slugs,
      only,
    );
    if (refresh.length) console.log(`[docsite] will refresh ${refresh.join(", ")}`);

    const linted = await lintFindings(sid, e);
    if (!linted.parsed) {
      rows.push({ sid, ok: false, error: linted.raw });
      console.log(`[docsite] ${i + 1}/${sids.length} ${sid} LINT FAIL`);
      continue;
    }
    const blocking = blockingFindings(linted.findings);
    if (blocking.length) {
      const error = blocking.map((f) => `${f.code}: ${f.message}`).join("\n");
      rows.push({ sid, ok: false, error });
      console.log(`[docsite] ${i + 1}/${sids.length} ${sid} LINT FAIL`);
      continue;
    }

    const restore = only.length > 0 ? filterScenario(sid) : () => {};
    try {
      let replayError: string | null = null;
      try {
        await replay(sid, e);
      } catch (err) {
        replayError = String(err);
      }
      const failed = readFailures(sid);
      if (!failed) {
        rows.push({ sid, ok: false, error: replayError ?? "replay produced no step events" });
        console.log(`[docsite] ${i + 1}/${sids.length} ${sid} FAIL`);
        continue;
      }
      const unexpected = unexpectedFailures(scenario.steps, failed, new Set(refresh));
      if (unexpected.length) {
        const error = unexpected.map((row) => `${row.id}: ${row.error}`).join("\n");
        rows.push({ sid, ok: false, error });
        console.log(`[docsite] ${i + 1}/${sids.length} ${sid} FAIL`);
        continue;
      }
      if (refresh.length) {
        await accept(sid, e, refresh);
        publishMinted(sid, refresh);
      }
      rows.push({ sid, ok: true, refreshed: refresh });
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
for (const r of rows) {
  const refreshed = r.refreshed?.length ? ` refreshed ${r.refreshed.join(",")}` : "";
  const detail = r.error ? `\n${r.error.slice(0, 800)}` : refreshed;
  console.log(`${r.ok ? "PASS" : "FAIL"} ${r.sid}${detail}`);
}
process.exit(rows.every((r) => r.ok) && rows.length > 0 ? 0 : 1);
