import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: infinite scroll — new .jscroll-added blocks stream in when the
// viewport reaches the bottom of the page.
await runEdgeGolden(
  "tc14",
  "infinite scroll loads a new content block at page bottom",
  "/infinite_scroll",
  ".example",
  async (b) => {
    await b.openPage();
    await b.scrollBottom("scroll to the bottom to trigger lazy load");
    await b.waitSelector(".jscroll-added", "lazy block streamed in");
    await b.assertElementPresent(".jscroll-added", "lazy block is visible");
  },
);
