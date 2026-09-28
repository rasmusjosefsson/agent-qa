#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc08", "Edge — native select dropdown",
  "/dropdown", "#dropdown",
  async (b) => {
    await b.openPage();
    await b.selectOption("#dropdown", "Option 1", "select Option 1");
    await b.assertElementAttribute("#dropdown", "value", "equals", "1", "dropdown value stuck");
  },
);
