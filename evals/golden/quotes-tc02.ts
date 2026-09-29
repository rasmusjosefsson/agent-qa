import { runEdgeGolden } from "./edge-pages-lib";

// quotes.toscrape.com/scroll — infinite scroll: hitting the bottom fires the
// next /api/quotes?page=2 and appends ten more quotes. Covers `wait url`
// (networkRequest) plus count growth.
await runEdgeGolden(
  "tc02",
  "scroll feed: infinite scroll fetches page 2",
  "https://quotes.toscrape.com/scroll",
  ".quote",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent(".quote:nth-of-type(10)", "page 1 rendered");
    await b.scrollBottom("scroll to the bottom of page 1");
    await b.waitRequest("*/api/quotes?page=2*", "page 2 request completes");
    await b.waitSelector(".quote:nth-of-type(20)", "page 2 quotes appended");
    await b.assertElementAbsent(".quote:nth-of-type(21)", "exactly twenty quotes after page 2");
    await b.assertNetworkStatus(
      { urlMatches: "api/quotes\\?page=2", method: "GET" },
      "equals",
      "200",
      "page 2 request is a 200",
    );
  },
  { label: "quotes" },
);
