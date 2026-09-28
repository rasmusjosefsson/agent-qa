#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc02", "Edge — shadow DOM: read slotted content through a shadow host",
  "/shadowdom", "my-paragraph",
  async (b) => {
    await b.openPage();
    await b.assertElementAttribute("my-paragraph", "text", "contains", "different text", "slotted text readable through the host");
  },
);
