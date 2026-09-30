#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// testautomationpractice.blogspot.com — file upload status + jQuery UI
// slider driven by keyboard, and the popup window as a second tab.
await runEdgeGolden(
  "tc04", "TAP — upload + slider + popup tab",
  "https://testautomationpractice.blogspot.com/", "#singleFileInput",
  async (b) => {
    await b.openPage();
    // Single-file upload: input → submit → status paragraph echoes the file.
    await b.upload("#singleFileInput", "evals/fixtures/upload-valid.txt", "attach the upload-valid fixture");
    await b.clickSelector("#singleFileForm button[type=submit]", "submit the single-file form");
    await b.waitSelectorText("#singleFileStatus", "upload-valid.txt", "upload status shows the file name");
    // jQuery UI price slider: focus the left handle, ArrowRight raises $min.
    await b.clickSelector("#slider-range .ui-slider-handle", "focus the left slider handle");
    await b.pressKey("ArrowRight", "nudge the slider right");
    await b.pressKey("ArrowRight", "nudge the slider right again");
    await b.assertElementAttribute("#amount", "value", "contains", "$", "price range still renders a dollar span");
    // Popup button opens a new window (selenium.dev); switch, assert, close.
    await b.clickSelector("#PopUp", "open the popup window");
    await b.waitMs(1500, "give the popup window a beat to open");
    await b.tabCommand("t2", "switch to the popup tab");
    await b.assertUrlContains("selenium", "popup tab navigated to selenium.dev");
    await b.tabCommand("close t2", "close the popup tab");
    await b.tabCommand("t1", "switch back to the practice page");
    await b.assertElementPresent("#singleFileInput", "main page intact after popup round-trip");
  },
  { label: "tap" },
);
