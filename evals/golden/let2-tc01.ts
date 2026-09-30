#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /dropdowns — select verb on three <select>s: single-value
// fruits (option values are indexes), language codes, and a multi-select
// (superheros). Value claims assert the DOM state after each selection.
await runEdgeGolden(
  "tc01",
  "letcode dropdowns — select verb + value claims",
  "https://letcode.in/dropdowns",
  "select#fruits",
  async (b) => {
    await b.openPage();
    // letcode mounts Google's fundingchoices consent wall lazily — dismiss
    // it so a late-mounting overlay can't intercept a later select.
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.selectOption("select#fruits", "1", "choose the first fruit");
    await b.assertElementAttribute("select#fruits", "value", "equals", "1", "fruit selection sticks");
    await b.selectOption("select#lang", "py", "choose python");
    await b.assertElementAttribute("select#lang", "value", "equals", "py", "language selection sticks");
    await b.selectOption("select#country", "Brazil", "choose a country");
    await b.assertElementAttribute("select#country", "value", "equals", "Brazil", "country selection sticks");
    await b.selectOption("select#superheros", "aq", "choose a superhero in the multi-select");
    await b.assertElementAttribute("select#superheros", "value", "equals", "aq", "multi-select holds the pick");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
