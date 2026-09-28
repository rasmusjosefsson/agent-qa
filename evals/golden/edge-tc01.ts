#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc01", "Edge — dynamic loading: element appears after async work",
  "/dynamic_loading/2", "#start button",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#start button", "start the dynamic load");
    await b.waitSelectorText("#finish", "Hello World!", "async content rendered");
    await b.assertElementText("#finish h4", "Hello World!", "greeting visible after load");
  },
);
