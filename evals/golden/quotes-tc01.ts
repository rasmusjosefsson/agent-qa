import { runEdgeGolden } from "./edge-pages-lib";

// quotes.toscrape.com/scroll — the XHR-fed variant: the page shell loads bare
// and every quote arrives via GET /api/quotes?page=N. Real-site coverage of
// the network claim subjects (fired / status / responseJsonPath).
await runEdgeGolden(
  "tc01",
  "scroll feed: first page renders via /api/quotes XHR",
  "https://quotes.toscrape.com/scroll",
  ".quote",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent(".quote:nth-of-type(10)", "ten quotes rendered");
    await b.assertElementAbsent(".quote:nth-of-type(11)", "exactly one page of quotes");
    await b.assertNetworkFired(
      { urlMatches: "api/quotes", method: "GET" },
      "quotes feed request fired",
    );
    await b.assertNetworkStatus(
      { urlMatches: "api/quotes\\?page=1", method: "GET" },
      "equals",
      "200",
      "first feed page is a 200",
    );
    await b.assertNetworkJson(
      { urlMatches: "api/quotes" },
      "$.has_next",
      "equals",
      true,
      "API reports a next page",
    );
    await b.assertNetworkJson(
      { urlMatches: "api/quotes" },
      "$.quotes[0].author.name",
      "equals",
      "Albert Einstein",
      "first quote author is Einstein",
    );
    await b.assertElementText(
      ".quote:nth-of-type(1) small.author",
      "Albert Einstein",
      "rendered quote matches the API payload",
    );
  },
  { label: "quotes" },
);
