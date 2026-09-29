/**
 * The qa-gate sticky comment generator: with a fabricated results dir it must
 * render the verdict table plus, for a failed case, the failure digest — the
 * failing step, console errors, network failures, heal count, and shot-diff
 * pointers the reviewer would otherwise download artifacts to see.
 *
 * Run: `bun test evals/golden/pr-comment.test.ts`
 */
import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { $ } from "bun";

function fixture(withDigest = true): string {
  const root = mkdtempSync(join(tmpdir(), "aq-prc-"));
  writeFileSync(
    join(root, "golden-all-2026-01-01T00-00-00.json"),
    JSON.stringify({
      startedAt: "2026-01-01T00:00:00Z",
      durMs: 61000,
      total: 2,
      passed: 1,
      failed: 1,
      results: [
        { name: "golden:file-upload:download", pass: true, exitCode: 0, timedOut: false, durMs: 30000 },
        { name: "golden:alerts-dialogs:tc01", pass: false, exitCode: 1, timedOut: false, durMs: 31000 },
      ],
    }),
  );
  if (!withDigest) return root;
  const runDir = join(
    root,
    "golden-alerts-dialogs-tc01-2026-01-01T00-00-01",
    "scenarios",
    "alerts-tc01",
    "replays",
    "run-1",
  );
  mkdirSync(join(runDir, "shots-diff"), { recursive: true });
  writeFileSync(
    join(runDir, "events.jsonl"),
    [
      JSON.stringify({ id: "s0", intent: "open page", status: "pass" }),
      JSON.stringify({ id: "s3", intent: "confirm deletes", status: "fail", error: "locator not found: button Delete\nframe two" }),
    ].join("\n"),
  );
  writeFileSync(
    join(runDir, "console.json"),
    JSON.stringify({
      count: 2,
      messages: [
        { type: "error", text: "TypeError: x is not a function" },
        { type: "warn", text: "deprecation" },
      ],
      session: "s",
    }),
  );
  writeFileSync(
    join(runDir, "network.json"),
    JSON.stringify({
      requestCount: 3,
      requests: [
        { method: "GET", url: "https://app.example/api/items?p=1", status: 200 },
        { method: "POST", url: "https://app.example/api/save", status: 500 },
        { method: "GET", url: "https://app.example/api/hang" },
      ],
    }),
  );
  writeFileSync(
    join(runDir, "heal.jsonl"),
    [JSON.stringify({ schema: "heal-row/v1", stepId: "s2", mode: "locator-correction" })].join("\n"),
  );
  writeFileSync(join(runDir, "shots-diff", "s4.png"), "png-bytes");
  return root;
}

async function render(root: string): Promise<string> {
  const r =
    await $`bun ${new URL("./pr-comment.ts", import.meta.url).pathname} --results-dir ${root}`.text();
  return r;
}

describe("pr-comment", () => {
  test("failed case renders the failure digest, not just the table", async () => {
    const body = await render(fixture());
    expect(body).toContain("<!-- qa-gate -->");
    expect(body).toContain("1 failure");
    expect(body).toContain("`alerts-dialogs:tc01` | FAIL (exit 1)");
    // the digest block
    expect(body).toContain("failure digest");
    expect(body).toContain("alerts-tc01` s3 confirm deletes — locator not found: button Delete");
    expect(body).toContain("console errors (1)");
    expect(body).toContain("TypeError: x is not a function");
    expect(body).not.toContain("deprecation"); // warn rows are not errors
    expect(body).toContain("network failures (2)");
    expect(body).toContain("POST …/save — 500");
    expect(body).toContain("GET …/hang — no response");
    expect(body).toContain("1 self-heal(s)");
    expect(body).toContain("shot diff: `alerts-tc01` …/shots-diff/s4.png");
  });

  test("a failed case with sparse artifacts still renders", async () => {
    const body = await render(fixture(false));
    expect(body).toContain("1 failure");
    expect(body).not.toContain("failure digest");
  });

  test("exits non-zero when no rollup exists", async () => {
    const root = mkdtempSync(join(tmpdir(), "aq-prc-empty-"));
    const r = await $`bun ${new URL("./pr-comment.ts", import.meta.url).pathname} --results-dir ${root}`
      .nothrow()
      .quiet();
    expect(r.exitCode).toBe(2);
  });
});
