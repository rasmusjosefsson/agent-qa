// petstore-tc01 — JPetStore catalog browse: category → product list →
// item detail. URLs carry volatile ;jsessionid= segments, so the url
// claims match on the action + param names only — this doubles as a
// volatility-resilience check for URL assertions.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc01",
  "JPetStore — browse FISH to the Angelfish item page",
  "https://petstore.octoperf.com/actions/Catalog.action",
  "#Sidebar",
  async (b) => {
    await b.openPage();
    await b.clickSelector('a[href*="categoryId=FISH"]', "open the FISH category");
    await b.waitSelector('a[href*="FI-SW-01"]', "fish product list rendered");
    await b.assertUrlContains("categoryId=FISH", "still in the FISH category");
    await b.clickSelector('a[href*="FI-SW-01"]', "open the Angelfish product");
    await b.waitSelector("h2", "product detail rendered");
    await b.assertElementText("h2", "Angelfish", "product page is Angelfish");
    await b.clickSelector('a[href*="itemId=EST-1"]', "open the first item variant");
    await b.assertUrlContains("viewItem=", "landed on the item detail");
    await b.assertUrlContains("itemId=EST-1", "item id in the URL");
    await b.assertElementAttribute("body", "text", "contains", "16.50", "item price rendered");
  },
);
