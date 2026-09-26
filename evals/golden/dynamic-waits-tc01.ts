#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc01", "Dynamic Waits TC01 delayed trigger does not timeout", async (golden) => {
  await golden.openPage();
  await golden.clickSelector('[data-testid="dw-trigger-delayed"]', "trigger delayed element");
  await golden.waitDuration(2500, "wait for delayed element path to complete");
  await golden.assertLiveCondition(
    '(() => ({ href: location.href, readyState: document.readyState, triggerVisible: !!document.querySelector("[data-testid=dw-trigger-delayed]") }))()',
    "page remains responsive after delayed element",
  );
  await golden.waitSelector('[data-testid="dw-trigger-delayed"]', "delayed trigger remains visible");
});

// tc02: hidden element becomes visible -> S01 delayed result
