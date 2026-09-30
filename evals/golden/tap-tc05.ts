#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// testautomationpractice.blogspot.com — jQuery UI droppable: mouse-tracking
// widgets ignore synthesized DragEvents, so both the live drive and the
// recorded do/drag go through trusted mouse input (needs #281's drag verb).
await runEdgeGolden(
  "tc05", "TAP — jQuery UI drag and drop",
  "https://testautomationpractice.blogspot.com/", "#draggable",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("#draggable", "the drag source rendered");
    await b.assertElementAttribute("#droppable", "text", "contains", "Drop here", "droppable starts empty");
    await b.dragSelector("#draggable", "#droppable", "drag the box onto the droppable");
    await b.waitSelectorText("#droppable", "Dropped", "droppable accepted the drag");
    await b.assertElementAttribute("#droppable", "class", "contains", "ui-state-highlight", "droppable flipped to highlight state");
  },
  { label: "tap" },
);
