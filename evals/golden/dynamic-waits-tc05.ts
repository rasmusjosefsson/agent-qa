#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc05", "Dynamic Waits TC05 spinner disappears before completion", async (golden) => {
  await golden.openPage();
  await golden.domClickSelector('[data-testid="btn-start-spinner"]', "start spinner");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const done = document.querySelector("[data-testid=spinner-done]"); const spinner = document.querySelector("[data-testid=spinner], #spinner, .animate-spin"); if (done && !spinner && (done.textContent || "").includes("Done! Spinner gone.")) resolve(true); else if (Date.now() - started > 7000) reject(new Error("spinner did not disappear with completion text")); else setTimeout(tick, 100); }; tick(); }))()',
    "spinner disappears and completion text appears",
  );
  await golden.waitSelectorAbsent('[data-testid="spinner"], #spinner, .animate-spin', "spinner is no longer visible");
  await golden.waitSelector('[data-testid="spinner-done"]', "spinner done message is visible");
  await golden.waitSelectorText('[data-testid="spinner-done"]', "Done! Spinner gone.", "spinner done message has expected text");
});
