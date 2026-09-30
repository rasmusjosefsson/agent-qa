#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// testautomationpractice.blogspot.com — jQuery UI datepicker, dblclick copy,
// and the Wikipedia search widget (live network round-trip on submit).
await runEdgeGolden(
  "tc03", "TAP — datepicker + dblclick + wiki search",
  "https://testautomationpractice.blogspot.com/", "#datepicker",
  async (b) => {
    await b.openPage();
    // jQuery UI calendar: open, pick the "15" day link, field keeps the date.
    await b.clickSelector("#datepicker", "open the jQuery UI datepicker");
    await b.waitSelector(".ui-datepicker-calendar td a", "the calendar grid rendered");
    await b.clickSelector(".ui-datepicker-calendar td:nth-child(5) a", "pick a mid-month day");
    await b.assertElementAttribute("#datepicker", "value", "matches", "\\d{2}/\\d{2}/\\d{4}", "datepicker holds an mm/dd/yyyy date");
    // "Copy Text" button: ondblclick copies #field1's value into #field2.
    await b.dblclickSelector("button[ondblclick]", "double-click the Copy Text button");
    await b.assertElementAttribute("#field2", "value", "equals", "Hello World!", "dblclick copied field1 into field2");
    // Wikipedia search widget — submits via its own button, results render inline.
    await b.fillSelector("#Wikipedia1_wikipedia-search-input", "playwright", "type into the wiki search box");
    await b.clickSelector(".wikipedia-search-button", "submit the wiki search");
    await b.waitSelectorText("#Wikipedia1_wikipedia-search-results", "Playwright", "wiki results rendered inline");
  },
  { label: "tap" },
);
