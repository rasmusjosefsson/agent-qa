import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze-tc01 — landing + category filters. The grid refetches via POST
// /bycat, so the suite asserts both the DOM change and the API call.
await runEdgeGolden(
  "db-tc01",
  "landing grid + category filter — cards swap, /bycat POST fires",
  "https://www.demoblaze.com/index.html",
  ".card-title a",
  async (g) => {
    await g.openPage();
    await g.waitSelector("#navbarExample a", "navbar renders");
    await g.assertElementPresent(".card-title a", "product cards rendered");
    await g.clickSelector("a[onclick=\"byCat('monitor')\"]", "filter to Monitors");
    await g.waitSelectorText(".card-title a", "Apple monitor", "monitor list renders");
    await g.assertNetworkFired(
      { method: "POST", urlMatches: "bycat" },
      true,
      "bycat POST fired for the filter",
    );
    await g.assertElementAbsent(
      "a.card-title[href*='prod.html?idp_=1']",
      "phone items filtered out",
    );
    await g.clickSelector("a[onclick=\"byCat('notebook')\"]", "filter to Laptops");
    await g.waitSelectorText(".card-title a", "Sony vaio", "laptop list renders");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
