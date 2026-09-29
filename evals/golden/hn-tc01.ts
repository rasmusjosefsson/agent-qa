#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// New site: news.ycombinator.com front page — a real live HTML app whose
// content changes daily, so every assert is structural.
await runEdgeGolden(
  "tc01",
  "hn front page — 30 submissions, ranks, subtext metadata",
  "https://news.ycombinator.com/",
  ".athing.submission",
  async (b) => {
    await b.openPage();
    await b.assertElementCount(
      ".athing.submission",
      "equals",
      30,
      "exactly 30 stories on the front page",
    );
    await b.assertElementPresent(".athing .rank", "rank numbers rendered");
    await b.assertElementPresent(".titleline a", "story links rendered");
    await b.assertElementPresent(".subtext .hnuser", "author links rendered");
    await b.assertElementPresent(".subtext .age", "timestamps rendered");
    await b.assertElementPresent("a.morelink", "More pagination footer");
    await b.assertElementPresent(
      'a[href="newest"]',
      "topnav New link present",
    );
    await b.assertUrlContains("news.ycombinator.com", "still on the front page");
  },
  { label: "hn" },
);
