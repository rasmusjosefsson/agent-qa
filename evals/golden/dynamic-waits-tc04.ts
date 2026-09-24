#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc04", "Dynamic Waits TC04 loading text reaches loaded state", async (golden) => {
  await golden.openPage();
  await golden.clickSelector('[data-testid="dw-fetch-btn"]', "start data fetch");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const el = document.querySelector("[id=result-s07]"); if (el && (el.textContent || "").includes("Fetched")) resolve(true); else if (Date.now() - started > 8000) reject(new Error("fetched result did not appear")); else setTimeout(tick, 100); }; tick(); }))()',
    "fetched result appears",
  );
  await golden.waitSelectorText('[id="result-s07"]', "Fetched", "result contains fetched data");
});
