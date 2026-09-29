import { runEdgeGolden } from "./edge-pages-lib";

// quotes.toscrape.com — an author's (about) link navigates to the
// server-rendered /author/<slug> bio page.
await runEdgeGolden(
  "tc06",
  "author (about) link navigates to the bio page",
  "https://quotes.toscrape.com/",
  ".quote",
  async (b) => {
    await b.openPage();
    await b.clickSelector("a[href='/author/Albert-Einstein']", "open Einstein's bio");
    await b.waitSelector(".author-title", "author page rendered");
    await b.assertUrlContains("/author/Albert-Einstein", "url moved to the author page");
    await b.assertElementText(".author-title", "Albert Einstein", "author heading");
    await b.assertElementPresent(".author-description", "bio text rendered");
    await b.assertNetworkStatus(
      { urlMatches: "/author/Albert-Einstein/$" },
      "equals",
      "200",
      "author document is a 200 (after the 308 to the canonical slash)",
    );
  },
  { label: "quotes" },
);
