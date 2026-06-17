#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc04", "Dynamic Waits TC04 loading text reaches loaded state", async (golden) => {
  await golden.openPage();
  await golden.clickText("Load Data", "start data load");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const matches = [...document.querySelectorAll("main em")].filter((el) => (el.textContent || "").trim() === "Data Loaded!"); if (matches.length) resolve(true); else if (Date.now() - started > 6000) reject(new Error("Data Loaded! text did not appear in main em")); else setTimeout(tick, 100); }; tick(); }))()',
    "Data Loaded text appears in main content",
  );
  await golden.waitSelectorText("main", "Data Loaded!", "main content contains Data Loaded after load");
});
