#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// Front page → More → page 2: a live pagination round-trip.
await runEdgeGolden(
  "tc02",
  "hn pagination — More link walks to page 2",
  "https://news.ycombinator.com/",
  ".athing.submission",
  async (b) => {
    await b.openPage();
    await b.clickSelector("a.morelink", "open page 2");
    await b.assertUrlContains("p=2", "url advanced to page 2");
    await b.assertElementPresent(".athing.submission", "page 2 lists stories");
    await b.assertElementPresent("a.morelink", "page 2 still paginates");
  },
  { label: "hn" },
);
