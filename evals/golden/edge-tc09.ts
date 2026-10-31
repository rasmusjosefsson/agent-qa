#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc09", "Edge — checkbox toggle",
  "/checkboxes", "#checkboxes",
  async (b) => {
    await b.openPage();
    await b.checkSelector("#checkboxes input:nth-child(1)", "check the first box");
    await b.assertElementAttribute("#checkboxes input:nth-child(1)", "checked", "equals", "true", "first box checked");
    // Second checkbox starts checked (`nth-child(3)` skips the `<br>`).
    // `checkSelector` is idempotent on an already-checked box — uncheck it.
    await b.uncheckSelector("#checkboxes input:nth-child(3)", "uncheck the second box");
    await b.assertElementAttribute("#checkboxes input:nth-child(3)", "checked", "equals", "false", "second box unchecked");
  },
);
