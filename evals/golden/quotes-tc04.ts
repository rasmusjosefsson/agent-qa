import { runEdgeGolden } from "./edge-pages-lib";

// quotes.toscrape.com (server-rendered index) — tag click navigates to a
// filtered /tag/<name>/ page. Covers full-document navigation + url claims +
// a silent feed check (server render, no XHR).
await runEdgeGolden(
  "tc04",
  "tag link navigates to the filtered tag page",
  "https://quotes.toscrape.com/",
  ".quote",
  async (b) => {
    await b.openPage();
    await b.clickSelector("a[href='/tag/change/page/1/']", "open the 'change' tag");
    await b.waitSelector(".quote", "tag page rendered");
    await b.assertUrlContains("/tag/change/", "url moved to the tag page");
    await b.assertElementText(
      "h3 a",
      "change",
      "tag page heading shows the filter",
    );
    await b.assertElementPresent(".quote", "filtered quotes rendered");
    await b.assertNetworkSilent(
      { urlMatches: "api/quotes" },
      "server-rendered tag page — no feed XHR",
    );
  },
  { label: "quotes" },
);
