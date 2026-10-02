#!/usr/bin/env bun
/**
 * Docs-site goldens generator — rebuilds `site/dist` and rewrites the
 * committed `scenarios/docs-pages/scenario.json` so every built page gets a
 * shot claim in BOTH themes. Run this whenever a docs page is added or
 * removed; CI regenerates and diffs so the scenario can't silently drift
 * from the real page list.
 *
 * Step shape (two passes — `starlight-theme` is read from localStorage at
 * page boot, so `state` before `goto` renders the whole pass in one theme):
 *
 *   viewport 1280x800 → goto / (establish origin) → state light
 *   → per page: goto → isVisible gate (`.mermaid svg` when the page has a
 *     client-rendered diagram, `main` otherwise) → shot claim on that step
 *   → state dark → same per page again.
 */
import { mkdirSync, writeFileSync } from "fs";
import { dirname } from "path";
import { resolve } from "path";
import { buildSite, env, previewPages, scenariosRoot } from "./lib.ts";

await buildSite(env());
const pages = previewPages();
if (!pages.length) throw new Error("no pages found under site/dist — build output empty?");

const steps: unknown[] = [
  {
    id: "s0",
    kind: "do",
    verb: "viewport",
    params: { width: 1280, height: 800 },
    intent: "pin desktop viewport",
  },
  {
    id: "s1",
    kind: "do",
    verb: "goto",
    value: { from: "literal", literal: (pages.find((p) => p.slug === "index") ?? pages[0]!).url },
    intent: "land on origin so `state` can seed localStorage",
  },
];

let n = 1;
const pass = (theme: "light" | "dark") => {
  steps.push({
    id: `s${++n}`,
    kind: "do",
    verb: "state",
    params: { localStorage: { "starlight-theme": theme } },
    intent: `theme → ${theme} (read by the theme bootstrap on next nav)`,
  });
  for (const p of pages) {
    const gate = `s${++n}`;
    steps.push({
      id: gate,
      kind: "do",
      verb: "goto",
      value: { from: "literal", literal: p.url },
      intent: `${p.slug} (${theme})`,
    });
    steps.push({
      id: `s${++n}`,
      kind: "check",
      claim: {
        subject: {
          element: {
            raw: { kind: "css", value: p.mermaid ? ".mermaid svg" : "main" },
            reason: "docsite",
          },
        },
        predicate: "isVisible",
      },
      intent: p.mermaid ? "content + diagram rendered" : "content rendered",
    });
    steps.push({
      id: `s${++n}`,
      kind: "check",
      claim: {
        subject: { shot: `s${n - 1}` },
        predicate: "matches",
        tolerance: { pixels: 0.05 },
      },
      intent: `${p.slug} golden (${theme})`,
    });
  }
};
pass("light");
pass("dark");

const scenario = {
  schema: "scenario/2",
  id: "docs-pages",
  intent: "Docs site renders every page identically to baseline in light + dark",
  env: { open: [{ kind: "fresh" }] },
  steps,
};

const out = resolve(scenariosRoot, "docs-pages", "scenario.json");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(scenario, null, 2) + "\n");
console.log(`wrote ${out} — ${pages.length} pages x 2 themes, ${steps.length} steps`);
