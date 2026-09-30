#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /slider — an input[type=range] (min 1, max 50) drives a React
// state readout ("Word limit : N") and a "Get Countries" button that renders
// N dash-separated country names into a teal panel. The range input is a
// controlled React component: a plain .value set is ignored, so a real key
// press (End) drives it to the max instead.
await runEdgeGolden(
  "tc04",
  "letcode slider — key-driven range input + generated output",
  "https://letcode.in/slider",
  "input[type=range]#generate",
  async (b) => {
    await b.openPage();
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.assertElementAttribute("#generate", "value", "equals", "10", "slider defaults to 10");
    await b.pressOn("#generate", "End", "press End on the slider to reach the max");
    await b.assertElementAttribute("#generate", "value", "equals", "50", "End key set the slider to its max");
    await b.assertElementText("h1.font-bold", "Word limit : 50", "heading reflects the max word limit");
    await b.clickRole("button", "Get Countries", "generate the country list");
    await b.assertElementPresent("p.leading-relaxed", "country list rendered into the teal panel");
    await b.waitMs(300, "let the generated list settle");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
