#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// New site: selectorshub.com xpath-practice-page — open + nested shadow
// DOM. The "user name" and "pizza" inputs live inside shadow roots, so a
// plain css selector cannot reach them: role locators resolve through the
// a11y snapshot (record + replay), and the value claim reads the snapshot
// line back. Telemetry beacons on this page are nondeterministic — flush
// skips auto-network claims here.
await runEdgeGolden(
  "tc01",
  "selectorshub — fill + assert inputs inside open/nested shadow DOM",
  "https://selectorshub.com/xpath-practice-page/",
  "#userName",
  async (b) => {
    await b.openPage();
    await b.typeRole("textbox", "user name field", "Shadow User", "fill username inside open shadow root");
    await b.typeRole("textbox", "Enter pizza name", "Margherita", "fill pizza inside nested shadow root");
    await b.assertRoleAttribute("textbox", "user name field", "value", "Shadow User", "shadow input holds the typed value");
    await b.assertRoleAttribute("textbox", "Enter pizza name", "value", "Margherita", "nested shadow input holds the typed value");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
  { label: "shub", flushArgs: ["--no-auto-network"] },
);
