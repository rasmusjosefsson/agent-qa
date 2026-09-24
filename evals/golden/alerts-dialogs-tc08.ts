#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc08", "Alerts Dialogs TC08 confirm dialog cancelled", async (golden) => {
  await golden.openPage();
  await golden.domClickSelector('[data-testid="open-confirm-dialog"]', "open confirm dialog");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const dlg = [...document.querySelectorAll("[class*=dialogBackdrop]")].pop(); if (dlg && (dlg.textContent || "").includes("Confirm Submission")) resolve(true); else if (Date.now() - started > 5000) reject(new Error("confirm dialog did not open")); else setTimeout(tick, 100); }; tick(); }))()',
    "confirm dialog opens",
  );
  await golden.waitSelector('[data-testid="confirm-cancel-btn"]', "confirm dialog cancel is visible");
  await golden.clickSelector('[data-testid="confirm-cancel-btn"]', "cancel the confirm dialog");
  await golden.waitSelectorAbsent('[data-testid="confirm-cancel-btn"]', "confirm dialog is dismissed");
});
