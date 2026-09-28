import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: floating menu — the nav bar JS-follows the viewport as the page
// scrolls. Covers scroll-to-bottom plus presence claims on floating chrome.
await runEdgeGolden(
  "tc25",
  "floating menu stays present after scrolling to the bottom",
  "/floating_menu",
  "#menu",
  async (b) => {
    await b.openPage();
    await b.scrollBottom("scroll to the end of the page");
    await b.waitSelector("#menu", "menu still rendered after scroll");
    await b.assertElementPresent("#menu a[href='#home']", "Home link still reachable");
    await b.assertElementPresent("#menu a[href='#contact']", "Contact link still reachable");
  },
);
