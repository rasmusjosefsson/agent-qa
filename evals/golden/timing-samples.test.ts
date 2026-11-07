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

  test("exits 2 when the rollup is missing", async () => {
    const root = mkdtempSync(join(tmpdir(), "aq-ts-empty-"));
    const out = join(root, "samples.json");
    const r = await $`bun ${script} --results-dir ${root} --out ${out}`.nothrow().quiet();
    expect(r.exitCode).toBe(2);
  });
});
