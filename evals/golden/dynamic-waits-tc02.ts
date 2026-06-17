#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc02", "Dynamic Waits TC02 hidden element becomes visible", async (golden) => {
  await golden.openPage();
  await golden.clickSelector('[data-testid="btn-show-element"]', "show delayed element");
  await golden.assertLiveSelectorText('[data-testid="delayed-element"]', "Element is now visible!", "delayed element text is visible");
  await golden.waitSelector('[data-testid="delayed-element"]', "delayed element is visible");
  await golden.waitSelectorText('[data-testid="delayed-element"]', "Element is now visible!", "delayed element has expected text");
});
