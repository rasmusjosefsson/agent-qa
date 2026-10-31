#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// Front page → comments link → item page with the (possibly empty)
// comment tree container. The last subtext anchor is always "N comments".
await runEdgeGolden(
  "tc03",
  "hn item page — comments link navigates to the thread",
  "https://news.ycombinator.com/",
  ".athing.submission",
  async (b) => {
    await b.openPage();
    // HN soft-gates rapid hops from cloud egress. Cool down, then try the
    // comments link a couple of times until the item chrome appears.
    let opened = false;
    for (let attempt = 1; attempt <= 3; attempt++) {
      await b.waitMs(3000 * attempt, `cool down before item fetch (attempt ${attempt})`);
      await b.clickSelector(
        ".athing.submission + tr td.subtext a[href^='item?id=']:last-of-type",
        "open the first story's comment thread",
      );
      try {
        await b.waitSelector("table.fatitem", "item header table rendered");
        opened = true;
        break;
      } catch {
        // Soft-gate or miss — reopen the front page and retry.
        await b.gotoUrl("https://news.ycombinator.com/", "return to front page after soft-gate");
        await b.waitSelector(".athing.submission", "front page listings restored");
      }
    }
    if (!opened) {
      throw new Error("hn-tc03: never reached an item page (HN soft-gate?)");
    }
    await b.assertUrlContains("item?id=", "landed on an item page");
    await b.assertElementPresent(
      ".fatitem .titleline a",
      "item title link rendered",
    );
    await b.assertElementPresent("table.comment-tree", "comment tree present");
  },
  { label: "hn" },
);
