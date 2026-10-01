#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// jqueryui.com/accordion + /slider — two demos in one scenario (gotoUrl
// navigation). Accordion section expansion claims via the active-class
// marker and aria-expanded; slider handle position via its inline style.
await runEdgeGolden(
  "tc03", "jqueryui — accordion expand + slider drag",
  "https://jqueryui.com/accordion/",
  "iframe.demo-frame",
  async (b) => {
    await b.openPage();
    await b.enterFrame("iframe.demo-frame", "into the accordion demo frame");
    await b.assertElementCount("#accordion .ui-accordion-content-active", 1, "one panel open initially");
    await b.clickSelector("#ui-id-3", "open Section 2");
    await b.waitMs(600, "accordion animate");
    await b.assertElementAttribute("#ui-id-3", "aria-expanded", "equals", "true", "Section 2 header expanded");
    await b.assertElementAttribute("#ui-id-4", "class", "contains", "ui-accordion-content-active", "Section 2 panel active");
    await b.assertElementAttribute("#ui-id-1", "aria-expanded", "equals", "false", "Section 1 collapsed");
    await b.exitFrame("back to the wrapper page");
    await b.assertElementText("h1", "Accordion", "wrapper page heading");

    await b.gotoUrl("https://jqueryui.com/slider/", "open the slider demo");
    await b.enterFrame("iframe.demo-frame", "into the slider demo frame");
    await b.assertElementAttribute("#slider .ui-slider-handle", "style", "contains", "left: 0%", "handle starts at 0%");
    await b.dragSelector("#slider .ui-slider-handle", "#slider", "drag handle to track center");
    await b.waitMs(400, "slider settle");
    await b.assertElementAttribute("#slider .ui-slider-handle", "style", "contains", "left: 50%", "handle landed at 50%");
    await b.exitFrame("back to the wrapper page");
    await b.assertElementText("h1", "Slider", "wrapper page heading");
  },
  { label: "jqui" },
);
