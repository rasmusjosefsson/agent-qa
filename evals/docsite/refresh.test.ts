/**
 * The docs golden gate may refresh a missing baseline or a page whose
 * source changed. Anything else still fails the run.
 *
 * Run: `bun test evals/docsite/refresh.test.ts`
 */
import { describe, expect, test } from "bun:test";
import {
  blockingFindings,
  changedFilesFromEnv,
  refreshShotIds,
  refreshShotsForReplay,
  shotSlugs,
  slugsForChangedFiles,
  terminalFailures,
  unexpectedFailures,
  type ShotStep,
} from "./refresh.ts";

const steps: ShotStep[] = [
  { id: "s1", verb: "goto", value: { literal: "http://127.0.0.1:4399/agent-qa/" } },
  { id: "s2", verb: "state" },
  { id: "s3", verb: "goto", value: { literal: "http://127.0.0.1:4399/agent-qa/docs/verbs/" } },
  { id: "s4", kind: "check", claim: { subject: {} } },
  { id: "s5", kind: "check", claim: { subject: { shot: "s4" } } },
  { id: "s6", verb: "goto", value: { literal: "http://127.0.0.1:4399/agent-qa/docs/architecture/" } },
  { id: "s7", kind: "check", claim: { subject: {} } },
  { id: "s8", kind: "check", claim: { subject: { shot: "s7" } } },
  { id: "s9", verb: "goto", value: { literal: "http://127.0.0.1:4399/agent-qa/docs/verbs/" } },
  { id: "s10", kind: "check", claim: { subject: {} } },
  { id: "s11", kind: "check", claim: { subject: { shot: "s10" } } },
];

describe("docs golden refresh", () => {
  test("maps a published doc file to its page slug and ignores other paths", () => {
    expect(slugsForChangedFiles(changedFilesFromEnv("docs/verbs.md\nsite/public/.well-known/agent.json"))).toEqual([
      "docs/verbs",
    ]);
    expect(slugsForChangedFiles(["docs/index.md"])).toEqual(["docs/overview"]);
    expect(slugsForChangedFiles(["site/src/pages/index.astro"])).toEqual(["index"]);
    expect(slugsForChangedFiles(["docs/gap-map.md", "docs/specs/notes.md"])).toEqual([]);
  });

  test("refreshes a changed page in both themes and a missing baseline", () => {
    const present = new Set(["s4.png", "s7.png", "s10.png"]);
    expect(refreshShotIds(steps, ["docs/verbs"], present)).toEqual(["s4", "s10"]);
    expect(refreshShotIds(steps, [], new Set(["s4.png", "s10.png"]))).toEqual(["s7"]);
    expect(refreshShotIds(steps, [], present)).toEqual([]);
  });

  test("a selective replay does not refresh pages it will not open", () => {
    const ids = refreshShotIds(steps, ["docs/verbs"], new Set(["s4.png", "s7.png", "s10.png"]));
    expect(refreshShotsForReplay(ids, shotSlugs(steps), ["docs/architecture"])).toEqual([]);
    expect(refreshShotsForReplay(["s7"], shotSlugs(steps), ["docs/verbs"])).toEqual([]);
    expect(refreshShotsForReplay(ids, shotSlugs(steps), [])).toEqual(["s4", "s10"]);
  });

  test("a missing shot baseline does not block lint, and other errors do", () => {
    const findings = blockingFindings([
      { severity: "error", code: "shot-missing-baseline", message: "baselines/s10.png is missing" },
      { severity: "warning", code: "brittle-locator", message: "css id" },
      { severity: "error", code: "goto-without-url", message: "step s3 has no value" },
    ]);
    expect(findings.map((f) => f.code)).toEqual(["goto-without-url"]);
  });

  test("a shot claim we are refreshing is expected; a visibility failure is not", () => {
    const failed = terminalFailures([
      { id: "s4", status: "running" },
      { id: "s4", status: "pass" },
      { id: "s5", status: "fail", error: "shot drifted" },
      { id: "s7", status: "fail", error: "not visible" },
      { id: "s8", status: "fail", error: "no baseline" },
      { id: "s8", status: "pass" },
    ]);
    expect([...failed.keys()]).toEqual(["s5", "s7"]);
    const unexpected = unexpectedFailures(steps, failed, new Set(["s4"]));
    expect(unexpected.map((row) => row.id)).toEqual(["s7"]);
  });
});
