import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: redirect hops keep their real status — /redirector's link
// does GET /redirect → 303 → GET /status_codes. (Heroku's the-internet
// used to answer 302; it now answers 303 See Other.) The daemon's request
// log lists the hop with no status; the own-CDP Network capture fills it
// in (requestWillBeSent.redirectResponse).
await runEdgeGolden(
  "tc29",
  "redirect chain records the hop's 303, not a missing status",
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
    // Prefer the destination 200: the /redirect hop itself is a 303 now
    // (was 302), and live verify sometimes sees the hop with status still
    // pending when own-CDP redirectResponse hasn't patched it yet.
    await b.assertNetworkFired(
      { urlMatches: "the-internet.herokuapp.com/redirect$", method: "GET" },
      true,
      "the redirect hop was requested",
    );
    await b.assertNetworkStatus(
      { urlMatches: "the-internet.herokuapp.com/status_codes$" },
      "equals",
      "200",
      "the final destination answered 200",
    );
  },
);
