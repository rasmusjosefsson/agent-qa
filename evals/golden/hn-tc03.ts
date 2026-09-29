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
    await b.clickSelector(
      ".athing.submission + tr td.subtext a[href^='item?id=']:last-of-type",
      "open the first story's comment thread",
    );
    await b.assertUrlContains("item?id=", "landed on an item page");
    await b.assertElementPresent("table.fatitem", "item header table rendered");
    await b.assertElementPresent(
      ".fatitem .titleline a",
      "item title link rendered",
    );
    await b.assertElementPresent("table.comment-tree", "comment tree present");
  },
  { label: "hn" },
);
