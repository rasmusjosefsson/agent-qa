#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc02", "Dynamic Waits TC02 hidden element becomes visible", async (golden) => {
  await golden.openPage();
  await golden.clickSelector('[data-testid="dw-trigger-delayed"]', "show delayed element");
  await golden.assertLiveSelectorText('[data-testid="dw-delayed-result"]', "appeared after a 2-second delay", "delayed element text is visible");
  await golden.waitSelector('[data-testid="dw-delayed-result"]', "delayed element is visible");
  await golden.waitSelectorText('[data-testid="dw-delayed-result"]', "appeared after a 2-second delay", "delayed element has expected text");
});
