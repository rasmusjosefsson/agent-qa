#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// rahulshettyacademy.com/AutomationPractice — display toggling through
// computed-style claims, a CSS :hover menu, the fixed-header table,
// and an iframe entered and exited through the frame verb.
await runEdgeGolden(
  "tc04", "rahulshetty practice — hide/show, hover menu, table, iframe",
  "https://rahulshettyacademy.com/AutomationPractice/",
  "#displayed-text",
  async (b) => {
    await b.openPage();
    // Hide/Show flips display on a DOM-resident input — the element is
    // never absent, so computed style is the honest claim. It's an
    // input: its visible computed display is inline-block.
    await b.assertStyle("#displayed-text", "display", "inline-block", "text visible at start");
    await b.clickSelector("#hide-textbox", "hide the text field");
    await b.assertStyle("#displayed-text", "display", "none", "text hidden after Hide");
    await b.clickSelector("#show-textbox", "show the text field again");
    await b.assertStyle("#displayed-text", "display", "block", "text back visible");
    // CSS :hover menu — real hover reveals it; Top jumps to #top.
    await b.scrollToSelector(".mouse-hover", "bring the hover block into view");
    await b.hoverSelector("#mousehover", "open the hover menu");
    await b.clickSelector(".mouse-hover-content a[href='#top']", "click Top in the hover menu");
    await b.assertUrlContains("#top", "page jumped to the top anchor");
    // Fixed-header table + footer total.
    await b.assertElementCount(".tableFixHead tbody tr", 9, "fixed table holds nine rows");
    await b.assertElementText(".totalAmount", "Total Amount Collected: 296", "footer totals 296");
    // Iframe round-trip through the frame verb. The frame is
    // cross-origin — eval-based claims can't pierce it, so the
    // presence claim goes through a role locator read off the a11y
    // snapshot, which does reach inside.
    await b.enterFrame("#courses-iframe", "into the courses iframe");
    await b.assertRolePresent("link", "Courses", "courses nav link inside the frame");
    await b.exitFrame("back to the top document");
    await b.assertElementPresent("#mousehover", "outer page intact");
  },
  { label: "rs" },
);
