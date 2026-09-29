#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /advancedtable — a searchable, paginated university table:
// a filter input narrows rows client-side ("No matching records found"
// row appears for misses) and numbered page buttons swap the page's rows.
// Pagination buttons expose proper button roles, so role locators drive them.
await runEdgeGolden(
  "tc03",
  "letcode advanced table — search filter + role-driven pagination",
  "https://letcode.in/advancedtable",
  "input[placeholder='Search table...']",
  async (b) => {
    await b.openPage();
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.assertElementText("#advancedtable tbody tr:nth-child(1) td:nth-child(2)", "University of Aberdeen", "page 1 starts with University of Aberdeen");
    await b.fillSelector("input[placeholder='Search table...']", "Aberdeen", "filter the table for Aberdeen");
    await b.assertElementText("#advancedtable tbody tr:nth-child(1) td:nth-child(2)", "University of Aberdeen", "filter keeps the Aberdeen row");
    await b.assertElementAbsent("#advancedtable tbody tr:nth-child(2)", "filter leaves exactly one row");
    await b.fillSelector("input[placeholder='Search table...']", "zzz", "filter with a query that matches nothing");
    await b.assertElementText("#advancedtable tbody tr td", "No matching records found", "miss shows the empty-state row");
    // Keystroke clear: an empty fill leaves the controlled input stale (the
    // filter still thinks "zzz"), so the pager never remounts.
    await b.clearSelector("input[placeholder='Search table...']", "clear the search filter");
    await b.waitSelectorText("#advancedtable tbody tr:nth-child(1) td:nth-child(2)", "University of Aberdeen", "unfiltered rows and pagination return");
    // The React pager unmounts while the cleared filter re-renders — wait for
    // the numbered-button strip to remount before driving it by role+name.
    await b.waitSelector("div.flex.items-center.gap-1", "pagination buttons remounted after clearing the filter");
    await b.clickRole("button", "4", "jump to page 4 of the table");
    await b.assertElementText("#advancedtable tbody tr:nth-child(1) td:nth-child(2)", "University of Wales, Newport", "page 4 starts with University of Wales, Newport");
    await b.clickRole("button", "Previous", "paginate back with Previous");
    await b.assertElementText("#advancedtable tbody tr:nth-child(1) td:nth-child(2)", "Middlesex University", "Previous lands on page 3");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
