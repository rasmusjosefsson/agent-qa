#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// jqueryui.com/droppable — the demo widget lives inside a class-only
// iframe (no id/name), so the frame verb exercises the selector→ref
// fallback; the drag itself proves mouse-tracked droppable works.
await runEdgeGolden(
  "tc01", "jqueryui — drag into droppable inside demo iframe",
  "https://jqueryui.com/droppable/",
  "iframe.demo-frame",
  async (b) => {
    await b.openPage();
    await b.enterFrame("iframe.demo-frame", "into the droppable demo frame");
    await b.dragSelector("#draggable", "#droppable", "drag me to my target");
    await b.waitMs(500, "droppable settle");
    await b.assertElementText("#droppable p", "Dropped!", "droppable accepted the drag");
    await b.assertElementAttribute("#droppable", "class", "contains", "ui-state-highlight", "droppable highlight class");
    await b.exitFrame("back to the wrapper page");
    await b.assertElementText("h1", "Droppable", "wrapper page heading");
  },
  { label: "jqui" },
);
