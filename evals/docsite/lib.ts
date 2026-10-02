/**
 * Docs-site goldens lib — builds `site/` (astro build → dist/) and serves it
 * through `astro preview`, then replays the committed scenarios under
 * evals/docsite/scenarios against the rendered pages, so the docs site's own
 * UI is covered by the same shot-claim golden loop as the workbench.
 *
 *   bun evals/docsite/run.ts            — verify: replay every scenario
 *   bun evals/docsite/accept.ts         — re-mint baselines (after an
 *                                          intentional UI change; commit the
 *                                          updated scenarios/<sid>/baselines/)
 *   bun evals/docsite/gen.ts            — regenerate the committed
 *                                          scenario.json from dist/*.html
 *
 * The committed scenario dirs hold scenario.json + baselines/<step>.png;
 * replays/ output under them is gitignored. Baselines are environment-bound —
 * mint them in CI (`/docs-goldens accept` or the workflow_dispatch job), not
 * on a dev machine.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, extname, resolve } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
export const docsiteRoot = resolve(__dirname);
export const scenariosRoot = resolve(docsiteRoot, "scenarios");
export const repoRoot = resolve(docsiteRoot, "..", "..");
export const siteRoot = resolve(repoRoot, "site");
export const previewPort = Number(process.env.DOCSITE_PORT || 4399);
export const previewBase = `http://127.0.0.1:${previewPort}/agent-qa`;

export const agentQa = existsSync(resolve(repoRoot, "cli/target/debug/agent-qa"))
  ? resolve(repoRoot, "cli/target/debug/agent-qa")
  : "agent-qa";
export const agentBrowser =
  process.env.AGENT_QA_EVAL_AGENT_BROWSER_BIN || "agent-browser";

export function env(extra: Record<string, string> = {}): Record<string, string> {
  return {
    ...(process.env as Record<string, string>),
    AGENT_QA_SCENARIOS_DIR: scenariosRoot,
    AGENT_QA_BINARY_PATH: agentQa,
    AGENT_BROWSER_BIN: agentBrowser,
    NO_COLOR: "1",
    ...extra,
  };
}

async function sh(cmd: string[], e: Record<string, string>, name: string, cwd = repoRoot): Promise<string> {
  console.error(`[docsite] ${name}`);
  const proc = Bun.spawn(cmd, { cwd, env: e, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  if (exitCode !== 0) {
    throw new Error(`${name} failed (${exitCode})\n${cmd.join(" ")}\n${stdout}\n${stderr}`);
  }
  return stdout;
}

/** Install site deps if needed and (re)build dist/ — the goldens gate on the
 *  page the PR actually produces, so always build fresh. */
export async function buildSite(e: Record<string, string>): Promise<void> {
  if (!existsSync(resolve(siteRoot, "node_modules"))) {
    await sh(["npm", "ci", "--no-audit", "--no-fund"], e, "install site deps", siteRoot);
  }
  await sh(["npm", "run", "build"], e, "build site", siteRoot);
}

const MIME: Record<string, string> = {
  ".html": "text/html",
  ".css": "text/css",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".map": "application/json",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webp": "image/webp",
  ".avif": "image/avif",
  ".ico": "image/x-icon",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
  ".xml": "application/xml",
  ".txt": "text/plain",
};

/** Serve site/dist in-process — `astro preview` is a singleton that refuses a
 *  second instance, so the suite can't rely on it. Mirrors its routing: the
 *  `/agent-qa` base prefix, trailing-slash dirs → index.html, 404 page. */
export function serveDist(): { base: string; stop: () => void } {
  const dist = resolve(siteRoot, "dist");
  const notFound = () => {
    const p = resolve(dist, "404.html");
    return new Response(Bun.file(p), { status: 404, headers: { "content-type": "text/html" } });
  };
  const server = Bun.serve({
    port: previewPort,
    hostname: "127.0.0.1",
    fetch(req) {
      let path: string;
      try {
        path = decodeURIComponent(new URL(req.url).pathname);
      } catch {
        return notFound();
      }
      if (!path.startsWith("/agent-qa/") && path !== "/agent-qa") return notFound();
      path = path.slice("/agent-qa".length) || "/";
      if (path.endsWith("/")) path += "index.html";
      const file = resolve(dist, `.${path}`);
      if (!file.startsWith(dist) || !existsSync(file)) return notFound();
      return new Response(Bun.file(file), {
        headers: { "content-type": MIME[extname(file)] ?? "application/octet-stream" },
      });
    },
  });
  return { base: previewBase, stop: () => server.stop(true) };
}

/** Build the site and serve dist/ until stop() — the eval-suite entrypoint. */
export async function bootPreview(): Promise<{ base: string; stop: () => void }> {
  await buildSite(env());
  mkdirSync(scenariosRoot, { recursive: true });
  return serveDist();
}

/** Close the `docsite-<sid>` agent-browser sessions the replays opened —
 *  the daemon + Chrome tree stay alive (and hold ~1.4 GB) long after the
 *  run ends otherwise. Best-effort: a failed close shouldn't fail the suite. */
export async function closeSessions(e: Record<string, string>): Promise<void> {
  for (const sid of scenarioIds()) {
    await sh(
      [agentBrowser, "close", "--session", `docsite-${sid}`],
      e,
      `close docsite-${sid}`,
    ).catch(() => {});
  }
}

export async function replay(sid: string, e: Record<string, string>): Promise<void> {
  await sh(
    // --keep-going: a missing/stale baseline fails the first shot claim, but
    // every later page still needs its screenshot captured (accept) and its
    // diff reported (verify) — stop-at-first-fail hides the full picture.
    [agentQa, "replay", sid, "--session", `docsite-${sid}`, "--keep-going"],
    e,
    `replay ${sid}`,
  );
}

/** Lint a scenario (schema + rules). Returns the failure output or null. */
export async function lint(sid: string, e: Record<string, string>): Promise<string | null> {
  const scenarioPath = resolve(scenariosRoot, sid, "scenario.json");
  try {
    await sh([agentQa, "scenario", "check", scenarioPath], e, `lint ${sid}`);
    return null;
  } catch (err) {
    return String(err);
  }
}

/** Re-mint every golden baseline for a scenario from its latest run. */
export async function accept(sid: string, e: Record<string, string>): Promise<void> {
  await sh([agentQa, "shot-accept", sid, "--json"], e, `shot-accept ${sid}`);
}

export function scenarioIds(): string[] {
  const { readdirSync } = require("fs");
  return readdirSync(scenariosRoot, { withFileTypes: true })
    .filter((d) => d.isDirectory() && existsSync(resolve(scenariosRoot, d.name, "scenario.json")))
    .map((d) => d.name)
    .sort();
}

export function report(name: string, rows: { sid: string; ok: boolean; error?: string }[]): void {
  const resultsDir = resolve(docsiteRoot, "results");
  mkdirSync(resultsDir, { recursive: true });
  const stamped = { name, at: new Date().toISOString(), rows };
  writeFileSync(
    resolve(resultsDir, `${name}-${stamped.at.replace(/[:.]/g, "-")}.json`),
    JSON.stringify(stamped, null, 2),
  );
  // Stable path for CI: the artifact upload + PR comment read this.
  writeFileSync(resolve(resultsDir, "latest.json"), JSON.stringify(stamped, null, 2));
}

/** Pages the committed scenario covers — every *.html page under dist/
 *  (except the error page), as an absolute preview URL. The generator and the
 *  drift check share this list. */
export function previewPages(): { slug: string; url: string; mermaid: boolean }[] {
  const { readdirSync, readFileSync: rf } = require("fs");
  const dist = resolve(siteRoot, "dist");
  const pages: { slug: string; url: string; mermaid: boolean }[] = [];
  const walk = (dir: string) => {
    for (const d of readdirSync(dir, { withFileTypes: true })) {
      const p = resolve(dir, d.name);
      if (d.isDirectory()) walk(p);
      else if (d.name === "index.html") {
        const rel = p.slice(dist.length).replace(/\/index\.html$/, "/") || "/";
        if (rel === "/404/") continue;
        const html = rf(p, "utf8");
        pages.push({
          slug: rel === "/" ? "index" : rel.replace(/^\//, "").replace(/\/$/, ""),
          url: previewBase + rel,
          mermaid: /class="mermaid"/.test(html),
        });
      }
    }
  };
  walk(dist);
  return pages.sort((a, b) => a.slug.localeCompare(b.slug));
}
