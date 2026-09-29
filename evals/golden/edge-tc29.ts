import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: redirect hops keep their real status — /redirector's link
// does GET /redirect → 302 → GET /status_codes. The daemon's request log
// lists the hop with no status; the own-CDP Network capture fills it in
// (requestWillBeSent.redirectResponse).
await runEdgeGolden(
  "tc29",
  "redirect chain records the hop's 302, not a missing status",
  "/redirector",
  ".example a[href='redirect']",
  async (b) => {
    await b.openPage();
    await b.clickSelector(
      ".example a[href='redirect']",
      "follow the redirect link",
    );
    await b.assertElementText(
      "h3",
      "Status Codes",
      "landed on the status-code index",
    );
    await b.assertNetworkStatus(
      { urlMatches: "the-internet.herokuapp.com/redirect$", method: "GET" },
      "equals",
      "302",
      "the redirect hop itself answered 302",
    );
    await b.assertNetworkStatus(
      { urlMatches: "the-internet.herokuapp.com/status_codes$" },
      "equals",
      "200",
      "the final destination answered 200",
    );
  },
);
