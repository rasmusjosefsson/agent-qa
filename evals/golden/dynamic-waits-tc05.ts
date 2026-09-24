#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc05", "Dynamic Waits TC05 spinner disappears before completion", async (golden) => {
  await golden.openPage();
  await golden.clickSelector('[data-testid="dw-trigger-spinner"]', "start spinner");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const done = document.querySelector("[data-testid=dw-spinner-content]"); if (done && (done.textContent || "").includes("Content loaded successfully")) resolve(true); else if (Date.now() - started > 8000) reject(new Error("spinner did not reach loaded content")); else setTimeout(tick, 100); }; tick(); }))()',
    "spinner disappears and content loads",
  );
  await golden.waitSelector('[data-testid="dw-spinner-content"]', "spinner content resolves");
  await golden.waitSelectorText('[data-testid="dw-spinner-content"]', "Content loaded successfully", "spinner done message has expected text");
});
