#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// /newest — a second list view (submit queue, no rank numbers).
await runEdgeGolden(
  "tc04",
  "hn newest — submit queue renders fresh submissions",
  "https://news.ycombinator.com/newest",
  ".athing.submission",
  async (b) => {
    await b.openPage();
    await b.assertUrlContains("/newest", "on the newest view");
    await b.assertElementPresent(".athing.submission", "submissions listed");
    await b.assertElementPresent(".subtext .age", "timestamps rendered");
    await b.assertElementPresent("a.morelink", "newest paginates");
  },
  { label: "hn" },
);
