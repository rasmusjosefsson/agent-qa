import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: status-code links — clicking a link lands on a page that only
// renders the returned status text. Covers link-click navigation + text claims.
await runEdgeGolden(
  "tc26",
  "status-code links render their returned status text",
  "/status_codes",
  "a[href='status_codes/200']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("a[href='status_codes/200']", "open the 200 link");
    await b.waitSelector(".example p", "status page rendered");
    await b.waitSelectorText(".example p", "200", "page says 200");
    await b.clickSelector("a[href='/status_codes']", "back to the index");
    await b.waitSelector("a[href='status_codes/404']", "index links back");
    await b.clickSelector("a[href='status_codes/404']", "open the 404 link");
    await b.waitSelectorText(".example p", "404", "page says 404");
  },
);
