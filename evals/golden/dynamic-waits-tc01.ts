#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc01", "Dynamic Waits TC01 delayed alert does not timeout", async (golden) => {
  await golden.openPage();
  await golden.clickText("Trigger Delayed Alert", "trigger delayed alert");
  await golden.waitDuration(2500, "wait for delayed alert path to complete");
  await golden.assertLiveCondition(
    '(() => ({ href: location.href, readyState: document.readyState, triggerVisible: !!document.querySelector("[data-testid=btn-delayed-alert]") }))()',
    "page remains responsive after delayed alert",
  );
  await golden.waitSelector('[data-testid="btn-delayed-alert"]', "delayed alert trigger remains visible after alert path");
});
