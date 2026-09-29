import { runEdgeGolden } from "./edge-pages-lib";

// quotes.toscrape.com/js — the client-rendered variant embeds the data inline
// (`var data = [...]`), so it must NOT fire /api/quotes: a real-site
// networkFired=false (silent) claim — data came from the document, not XHR.
await runEdgeGolden(
  "tc03",
  "js variant renders inline data with no feed request",
  "https://quotes.toscrape.com/js/",
  ".quote",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent(".quote:nth-of-type(10)", "ten quotes render client-side");
    await b.assertElementText(
      ".quote:nth-of-type(10) small.author",
      "Eleanor Roosevelt",
      "tenth quote rendered from inline data",
    );
    await b.assertNetworkSilent(
      { urlMatches: "api/quotes" },
      "no /api/quotes XHR — data was inline",
    );
    await b.assertElementText(
      "small.author",
      "Albert Einstein",
      "first quote is Einstein",
    );
  },
  { label: "quotes" },
);
