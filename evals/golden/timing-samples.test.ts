/**
 * Passing cases become timing samples. Failed cases stay out, because a
 * timeout is not the case's cost.
 *
 * Run: `bun test evals/golden/timing-samples.test.ts`
 */
import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { $ } from "bun";

const script = new URL("./timing-samples.ts", import.meta.url).pathname;

describe("timing-samples", () => {
  test("keeps passing cases and drops failures", async () => {
    const root = mkdtempSync(join(tmpdir(), "aq-ts-"));
    writeFileSync(
      join(root, "golden-all-2026-01-01T00-00-00.json"),
      JSON.stringify({
        results: [
          { name: "golden:alerts-dialogs:tc01", pass: true, durMs: 23200.4 },
          { name: "golden:file-upload:download:tc02", pass: false, durMs: 300000 },
        ],
      }),
    );
    const out = join(root, "samples.json");
    await $`bun ${script} --results-dir ${root} --out ${out}`.quiet();
    const body = JSON.parse(readFileSync(out, "utf8")) as {
      schema: string;
      entries: { id: string; kind: string; ms: number }[];
    };
    expect(body.schema).toBe("agent-qa-timing-samples/v1");
    expect(body.entries).toEqual([
      { id: "case:alerts-dialogs:tc01", kind: "case", ms: 23200 },
    ]);
  });

  test("merges a build timing line and skips a missing builds file", async () => {
    const root = mkdtempSync(join(tmpdir(), "aq-ts-build-"));
    writeFileSync(
      join(root, "golden-all-2026-01-01T00-00-00.json"),
      JSON.stringify({
        results: [{ name: "golden:alerts-dialogs:tc01", pass: true, durMs: 23200 }],
      }),
    );
    const builds = join(root, "builds.txt");
    writeFileSync(builds, "build:cli=96000\n# note\nnot a line\n");
    const out = join(root, "samples.json");
    await $`bun ${script} --results-dir ${root} --out ${out} --builds ${builds}`.quiet();
    const body = JSON.parse(readFileSync(out, "utf8")) as {
      entries: { id: string; kind: string; ms: number }[];
    };
    expect(body.entries).toEqual([
      { id: "case:alerts-dialogs:tc01", kind: "case", ms: 23200 },
      { id: "build:cli", kind: "build", ms: 96000 },
    ]);
    const out2 = join(root, "samples-2.json");
    await $`bun ${script} --results-dir ${root} --out ${out2} --builds ${join(root, "nope.txt")}`.quiet();
    const body2 = JSON.parse(readFileSync(out2, "utf8")) as { entries: { kind: string }[] };
    expect(body2.entries.map((e) => e.kind)).toEqual(["case"]);
  });

  test("exits 2 when the rollup is missing", async () => {
    const root = mkdtempSync(join(tmpdir(), "aq-ts-empty-"));
    const out = join(root, "samples.json");
    const r = await $`bun ${script} --results-dir ${root} --out ${out}`.nothrow().quiet();
    expect(r.exitCode).toBe(2);
  });
});
