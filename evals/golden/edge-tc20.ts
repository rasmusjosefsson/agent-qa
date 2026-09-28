import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: typos — the paragraph randomly contains a misspelled variant on
// each load; only the stable substring is claimable.
await runEdgeGolden(
  "tc20",
  "typos page renders its randomly-misspelled paragraph",
  "/typos",
  ".example p",
  async (b) => {
    await b.openPage();
    await b.assertElementAttribute(".example p", "text", "contains", "typo", "paragraph mentions its own typo");
  },
);
