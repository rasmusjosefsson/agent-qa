#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// jqueryui.com/sortable — reorder a list via mouse-drag inside the demo
// frame; DOM-order claims prove the sort actually landed.
await runEdgeGolden(
  "tc02", "jqueryui — sortable list reorder by drag",
  "https://jqueryui.com/sortable/",
  "iframe.demo-frame",
  async (b) => {
    await b.openPage();
    await b.enterFrame("iframe.demo-frame", "into the sortable demo frame");
    await b.assertElementText("#sortable li:first-child", "Item 1", "initial order precondition");
    await b.assertElementCount("#sortable li", 7, "seven sortable items");
    await b.dragSelector("#sortable li:nth-child(7)", "#sortable li:nth-child(1)", "drag Item 7 to the top");
    await b.waitMs(500, "sortable settle");
    await b.assertElementText("#sortable li:first-child", "Item 7", "Item 7 reordered to front");
    await b.assertElementText("#sortable li:last-child", "Item 6", "rest of the list shifted down");
    await b.exitFrame("back to the wrapper page");
    await b.assertElementText("h1", "Sortable", "wrapper page heading");
  },
  { label: "jqui" },
);
