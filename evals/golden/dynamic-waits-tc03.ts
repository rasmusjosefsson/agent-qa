#!/usr/bin/env bun
import { runDynamicWaitsGolden } from "./dynamic-waits-lib.ts";

await runDynamicWaitsGolden("tc03", "Dynamic Waits TC03 disabled button becomes enabled", async (golden) => {
  await golden.openPage();
  await golden.assertLiveCondition(
    '(() => { const el = document.querySelector("[data-testid=dw-submit-btn]"); if (!el?.disabled) throw new Error("target button was not initially disabled"); return true; })()',
    "target button is initially disabled",
  );
  await golden.waitSelector('[data-testid="dw-submit-btn"][disabled]', "target button is initially disabled");
  await golden.domClickSelector('[data-testid="dw-arm-enable"]', "arm the enable countdown");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const el = document.querySelector("[data-testid=dw-submit-btn]"); if (el && !el.disabled) resolve(true); else if (Date.now() - started > 6000) reject(new Error("target button did not become enabled")); else setTimeout(tick, 100); }; tick(); }))()',
    "target button becomes enabled",
  );
  await golden.waitSelector('[data-testid="dw-submit-btn"]:not([disabled])', "target button is enabled");
  await golden.clickSelector('[data-testid="dw-submit-btn"]', "submit after enable");
  await golden.waitSelectorText('[id="result-s05"]', "Sub", "result reflects successful submit");
});
