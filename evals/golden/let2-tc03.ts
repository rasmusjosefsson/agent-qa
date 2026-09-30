#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /window — window.open spawns a second tab; the `tab` verb
// switches the session between them by positional ref (t1 = opener,
// t2 = opened).
await runEdgeGolden(
  "tc03",
  "letcode window — window.open + tab round-trip",
  "https://letcode.in/window",
  "#home",
  async (b) => {
    await b.openPage();
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.clickSelector("#home", "open home page in a new tab");
    await b.waitMs(1000, "let the new tab open");
    await b.tabCommand("t2", "switch to the opened tab");
    await b.assertUrlContains("letcode.in", "child tab is on the home page");
    await b.assertElementPresent("h1", "home heading rendered in the child tab");
    await b.tabCommand("t1", "switch back to the windows page");
    await b.assertElementPresent("#multi", "back on the windows page");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
