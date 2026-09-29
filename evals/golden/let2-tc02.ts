#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /button — computed-style claim (#color is teal), disabled-state
// claim (#isDisabled), and a real same-tab navigation via #home.
await runEdgeGolden(
  "tc02",
  "letcode buttons — style + disabled claims + nav",
  "https://letcode.in/button",
  "#home",
  async (b) => {
    await b.openPage();
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.assertStyle("#color", "background-color", "rgb(13, 148, 136)", "color button is teal");
    await b.assertElementAttribute("#isDisabled", "disabled", "equals", "true", "disabled button reads disabled");
    await b.clickSelector("#home", "go home");
    await b.assertUrlContains("letcode.in", "navigated to the home page");
    await b.assertElementPresent("h1", "home heading rendered");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
