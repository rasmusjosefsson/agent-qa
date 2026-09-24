#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc09", "Alerts Dialogs TC09 info dialog closes", async (golden) => {
  await golden.openPage();
  await golden.domClickSelector('[data-testid="open-info-dialog"]', "open info dialog");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const dlg = [...document.querySelectorAll("[class*=dialogBackdrop]")].pop(); if (dlg && (dlg.textContent || "").includes("Session Notice")) resolve(true); else if (Date.now() - started > 5000) reject(new Error("info dialog did not open")); else setTimeout(tick, 100); }; tick(); }))()',
    "info dialog opens",
  );
  await golden.waitSelector('[data-testid="info-dialog-ok-btn"]', "info dialog ok button is visible");
  await golden.clickSelector('[data-testid="info-dialog-ok-btn"]', "acknowledge info dialog");
  await golden.waitSelectorAbsent('[data-testid="info-dialog-ok-btn"]', "info dialog is dismissed");
});
