/**
 * Golden PR comments include a structural-diff block and a diff-viewer link
 * when the latest run wrote domshots-diff/<step>.diff.txt.
 *
 * Run: `bun test evals/golden/domshot-pr-comment.test.ts`
 */
import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { $ } from "bun";

function fixture(): string {
  const root = mkdtempSync(join(tmpdir(), "aq-domshot-prc-"));
  mkdirSync(join(root, "results"), { recursive: true });
  writeFileSync(
    join(root, "results", "latest.json"),
    JSON.stringify({
      rows: [
        { sid: "settings", ok: false },
        { sid: "runs", ok: true },
      ],
    }),
  );
  const runDir = join(root, "scenarios", "settings", "replays", "run-9");
  mkdirSync(join(runDir, "domshots-diff"), { recursive: true });
  writeFileSync(join(root, "scenarios", "settings", "replays", "latest.txt"), "run-9\n");
  writeFileSync(
    join(runDir, "domshots-diff", "s3.diff.txt"),
    '--- baselines/s3.snap.txt\n+++ snapshots/s3.txt\n- heading "Old"\n+ heading "New"\n',
  );
  return root;
}

async function render(script: string, root: string): Promise<string> {
  return await $`bun ${script} --root ${root} --run-url https://example.test/run --diff-base https://raw.githubusercontent.com/acme/agent-qa/goldens-diffs/pr-7`.text();
}

describe("domshot lines in golden PR comments", () => {
  test("docs comment embeds the hunk and a dom=1 viewer link", async () => {
    const root = fixture();
    const md = await render(new URL("../docsite/pr-comment.ts", import.meta.url).pathname, root);
    expect(md).toContain("structural diff");
    expect(md).toContain('heading "New"');
    expect(md).toContain("diff-viewer.html?");
    expect(md).toContain("dom=1");
    expect(md).toContain("s3.baseline.snap.txt");
    expect(md).toContain("s3.current.snap.txt");
    expect(md).toContain("side-by-side line diff");
    expect(md).not.toContain("No shot or domshot diffs were captured");
  });

  test("ui comment embeds the hunk and a dom=1 viewer link", async () => {
    const root = fixture();
    const md = await render(new URL("../selftest/pr-comment.ts", import.meta.url).pathname, root);
    expect(md).toContain("### agent-qa UI goldens");
    expect(md).toContain("structural diff");
    expect(md).toContain("dom=1");
    expect(md).toContain("side-by-side line diff");
  });
});
