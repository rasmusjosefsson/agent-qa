// bonigarcia-tc02 — shadow DOM: the greeting lives inside #content's open
// shadowRoot, unreachable to css locators. The text wait resolves through
// the a11y snapshot, which pierces open roots — this is the record-path
// claim that proves it end to end.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc02",
  "Bonigarcia — read text inside an open shadow root",
  "https://bonigarcia.dev/selenium-webdriver-java/shadow-dom.html",
  "#content",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("#content", "shadow host rendered");
    await b.waitText("Hello Shadow DOM", "shadow-root paragraph is visible");
    await b.assertElementAbsent("#content > p", "no light-DOM paragraph under the host (it is inside the root)");
  },
);
