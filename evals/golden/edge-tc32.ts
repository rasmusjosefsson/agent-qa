import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: challenging DOM — the canvas answer changes on every click and
// button ids are random per render; stable handles are the button classes.
await runEdgeGolden(
  "tc32",
  "challenging dom: stable class handles over random ids",
  "/challenging_dom",
  "a.button",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("table tbody tr:nth-of-type(10)", "10-row table renders");
    await b.assertElementPresent("canvas#canvas", "canvas present");
    await b.clickSelector("a.button.alert", "click the alert button");
    await b.waitSelector("a.button", "page re-rendered with new ids");
    await b.assertElementPresent("table tbody tr:nth-of-type(10)", "table still renders after re-render");
    await b.clickSelector("a.button.success", "click the success button");
    await b.assertElementPresent("a.button", "buttons re-rendered again");
  },
);
