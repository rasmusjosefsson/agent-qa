#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc03", "Edge — key presses land on a focused input",
  "/key_presses", "#target",
  async (b) => {
    await b.openPage();
    await b.pressOn("#target", "a", "press 'a' in the target field");
    await b.assertElementText("#result", "You entered: A", "key echo rendered");
  },
);
