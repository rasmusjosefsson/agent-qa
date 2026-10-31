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
    // Pause before More — HN soft-gates ("not able to serve your requests
    // this quickly") when the pagination fetch follows the front-page load
    // too tightly from cloud egress.
    await b.waitMs(3000, "cool down before pagination fetch");
    await b.clickSelector("a.morelink", "open page 2");
    await b.waitSelector(".athing.submission", "page 2 lists stories");
    await b.assertUrlContains("p=2", "url advanced to page 2");
    await b.assertElementPresent("a.morelink", "page 2 still paginates");
  },
  { label: "hn" },
);
