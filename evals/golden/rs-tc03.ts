#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// rahulshettyacademy.com/AutomationPractice — two popup classes on one
// page: a target=_blank anchor (Open Tab) and a window.open button
// (Open Window). Both surface as t2 for the tab round-trip.
await runEdgeGolden(
  "tc03", "rahulshetty practice — open tab + open window round-trips",
  "https://rahulshettyacademy.com/AutomationPractice/",
  "#opentab",
  async (b) => {
    await b.openPage();
    // Anchor target=_blank → new tab.
    await b.clickSelector("#opentab", "open the target=_blank tab");
    await b.waitMs(3000, "give the new tab a beat to open");
    await b.tabCommand("t2", "switch to the new tab");
    await b.assertUrlContains("qaclickacademy", "tab navigated to the academy site");
    await b.tabCommand("close t2", "close the new tab");
    await b.tabCommand("t1", "back to the practice page");
    await b.assertElementPresent("#openwindow", "main page intact after tab round-trip");
    // Button window.open → popup window. Tab ids are monotonic —
    // after closing t2 the next popup registers as t3.
    await b.clickSelector("#openwindow", "open the window popup");
    await b.waitMs(3000, "give the popup window a beat to open");
    await b.tabCommand("t3", "switch to the popup window");
    await b.assertUrlContains("qaclickacademy", "popup window navigated to the academy site");
    await b.tabCommand("close t3", "close the popup window");
    await b.tabCommand("t1", "back to the practice page");
    await b.assertElementPresent("#opentab", "main page intact after both popups");
  },
  { label: "rs" },
);
