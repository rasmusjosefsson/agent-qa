#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// hn.algolia.com — a real React SPA on the official Algolia backend. Typing
// debounces a POST to *.algolia.net/1/indexes/…/query whose body carries the
// query string — the network claims prove url, method, status AND post body.
await runEdgeGolden(
  "tc05",
  "hn algolia — SPA search posts the query and renders results",
  "https://hn.algolia.com/",
  "input[type='search']",
  async (b) => {
    await b.openPage();
    await b.fillSelector(
      "input[type='search']",
      "typescript",
      "type a search query",
    );
    await b.waitMs(1500, "let the debounced search fire");
    await b.assertNetworkFired(
      { urlMatches: "algolia\\.net/1/indexes/", method: "POST" },
      true,
      "search API request captured",
    );
    await b.assertNetworkFired(
      {
        urlMatches: "algolia\\.net/1/indexes/",
        postDataContains: "typescript",
      },
      true,
      "search POST body carries the typed query",
    );
    await b.assertNetworkStatus(
      { urlMatches: "algolia\\.net/1/indexes/" },
      "equals",
      "200",
      "search API returned 200",
    );
    await b.waitSelector(".Story", "result stories rendered");
    await b.assertElementPresent(".Story a", "result links rendered");
  },
  { label: "hn" },
);
