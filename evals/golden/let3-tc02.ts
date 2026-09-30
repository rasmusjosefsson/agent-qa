#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /table — a shopping list (item + price columns, Total footer)
// and an employee table whose last column is a Present/Absent checkbox
// toggle. Text claims read cells; the toggle is flipped and re-asserted.
await runEdgeGolden(
  "tc02",
  "letcode tables — cell text, price column, presence-toggle checkbox",
  "https://letcode.in/table",
  "#shopping",
  async (b) => {
    await b.openPage();
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.assertElementText("#shopping tbody tr:nth-child(1) td:nth-child(1)", "Chocolate", "first shopping item is Chocolate");
    await b.assertElementText("#shopping tbody tr:nth-child(4) td:nth-child(1)", "Corn", "last shopping item is Corn");
    await b.assertElementText("#shopping tbody tr:nth-child(4) td:nth-child(2)", "480", "Corn costs 480");
    await b.assertElementText("#simpletable tbody tr:nth-child(1) td:nth-child(1)", "Koushik", "first employee row is Koushik");
    await b.assertElementText("#simpletable tbody tr:nth-child(3) td:nth-child(3)", "man@letcode.in", "third employee email cell");
    await b.checkSelector("#simpletable tbody tr:nth-child(1) td:nth-child(4) input", "mark Koushik present");
    await b.assertElementAttribute("#simpletable tbody tr:nth-child(1) td:nth-child(4) input", "checked", "equals", "true", "presence checkbox stayed checked");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
