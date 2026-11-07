/**
 * Decide which docs-site shot baselines a pull request may refresh.
 *
 * A missing `baselines/<step>.png` fails every docs or site pull request
 * before replay, and a page whose markdown changed fails the pixel compare
 * on purpose. Both are refreshed from the CI replay. A page whose source
 * did not change is never in this set, so a real pixel drift still fails.
 *
 * Shared chrome (styles, layout, sidebar) is not a per-page source edit.
 * Those pulls stay red until someone runs `/docs-goldens accept`.
 */

export type ShotStep = {
  id?: string;
  kind?: string;
  verb?: string;
  value?: { literal?: string };
  claim?: { subject?: { shot?: string } };
};

export type LintFinding = { severity: string; code: string; message: string };

export type StepEvent = { id?: string; status?: string; error?: string };

/** Published page slug for a source file that renders into the docs site.
 *  Repo-only notes (gap map, specs) are absent: editing them changes no shot. */
export const DOC_FILE_SLUGS: Record<string, string> = {
  "docs/index.md": "docs/overview",
  "docs/architecture.md": "docs/architecture",
  "docs/configuration.md": "docs/configuration",
  "docs/extension.md": "docs/extension",
  "docs/flow.md": "docs/flow",
  "docs/github-action.md": "docs/github-action",
  "docs/lint-rules.md": "docs/lint-rules",
  "docs/network.md": "docs/network",
  "docs/plugins.md": "docs/plugins",
  "docs/process-hygiene.md": "docs/process-hygiene",
  "docs/releasing.md": "docs/releasing",
  "docs/surfaces.md": "docs/surfaces",
  "docs/templates.md": "docs/templates",
  "docs/verbs.md": "docs/verbs",
  "docs/visual-testing.md": "docs/visual-testing",
  "docs/workbench.md": "docs/workbench",
  "site/src/content/docs/docs/quickstart.mdx": "docs/quickstart",
  "site/src/pages/index.astro": "index",
};

export function changedFilesFromEnv(raw: string | undefined): string[] {
  return (raw ?? "")
    .split(/[\n,]/)
    .map((s) => s.trim())
    .filter(Boolean);
}

export function slugsForChangedFiles(files: readonly string[]): string[] {
  const slugs = new Set<string>();
  for (const file of files) {
    const slug = DOC_FILE_SLUGS[file];
    if (slug) slugs.add(slug);
  }
  return [...slugs].sort();
}

/** Slug of a `goto` step, matching the docs-site preview path. The landing
 *  `goto` (first one) is not a page under test. */
export function pageSlug(step: ShotStep): string | null {
  if (step.verb !== "goto") return null;
  const literal = step.value?.literal;
  if (typeof literal !== "string") return null;
  const path = new URL(literal).pathname.replace(/\/+$/, "");
  if (path === "/agent-qa") return "index";
  return path.startsWith("/agent-qa/") ? path.slice("/agent-qa/".length) : null;
}

/** Screenshot step id → page slug. The first goto only establishes origin. */
export function shotSlugs(steps: readonly ShotStep[]): Map<string, string | null> {
  let sawGoto = false;
  let slug: string | null = null;
  const map = new Map<string, string | null>();
  for (const step of steps) {
    if (step.verb === "goto") {
      const next = pageSlug(step);
      if (!sawGoto) {
        sawGoto = true;
        continue;
      }
      slug = next;
      continue;
    }
    const shot = step.claim?.subject?.shot;
    if (typeof shot === "string" && shot) map.set(shot, slug);
  }
  return map;
}

/** Shot ids to re-mint: the baseline file is missing, or the page source changed. */
export function refreshShotIds(
  steps: readonly ShotStep[],
  changedSlugs: readonly string[],
  baselineNames: ReadonlySet<string>,
): string[] {
  const changed = new Set(changedSlugs);
  const slugs = shotSlugs(steps);
  const ids: string[] = [];
  for (const [shot, slug] of slugs) {
    const missing = !baselineNames.has(`${shot}.png`);
    const edited = slug !== null && changed.has(slug);
    if (missing || edited) ids.push(shot);
  }
  return ids;
}

/** Drop refresh ids for pages this run will not visit (`DOCS_ONLY`). */
export function refreshShotsForReplay(
  ids: readonly string[],
  slugs: ReadonlyMap<string, string | null>,
  only: readonly string[],
): string[] {
  if (only.length === 0) return [...ids];
  const allow = new Set(only);
  return ids.filter((id) => {
    const slug = slugs.get(id);
    return slug != null && allow.has(slug);
  });
}

/** Lint errors other than a missing shot baseline. Warnings never block. */
export function blockingFindings(findings: readonly LintFinding[]): LintFinding[] {
  return findings.filter((f) => f.severity === "error" && f.code !== "shot-missing-baseline");
}

/** Last terminal status per step. A later pass clears an earlier fail. */
export function terminalFailures(events: readonly StepEvent[]): Map<string, string> {
  const status = new Map<string, string>();
  const error = new Map<string, string>();
  for (const event of events) {
    if (!event.id || event.status === "running" || !event.status) continue;
    status.set(event.id, event.status);
    if (event.status === "fail") error.set(event.id, event.error ?? "");
    if (event.status === "pass") error.delete(event.id);
  }
  const failed = new Map<string, string>();
  for (const [id, state] of status) {
    if (state === "fail") failed.set(id, error.get(id) ?? "");
  }
  return failed;
}

export type StepFailure = { id: string; error: string };

/** Failures that are not a shot claim whose screenshot we intend to refresh. */
export function unexpectedFailures(
  steps: readonly ShotStep[],
  failed: ReadonlyMap<string, string>,
  refresh: ReadonlySet<string>,
): StepFailure[] {
  const byId = new Map(steps.map((step) => [step.id, step]));
  const out: StepFailure[] = [];
  for (const [id, error] of failed) {
    const shot = byId.get(id)?.claim?.subject?.shot;
    if (typeof shot === "string" && refresh.has(shot)) continue;
    out.push({ id, error });
  }
  return out;
}
