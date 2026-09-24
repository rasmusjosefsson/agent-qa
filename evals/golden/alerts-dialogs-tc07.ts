#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc07", "Alerts Dialogs TC07 notification dialog appears", async (golden) => {
  await golden.openPage();
  await golden.domClickSelector('[data-testid="open-notification-dialog"]', "open notification dialog");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const dlg = [...document.querySelectorAll("[class*=dialogBackdrop]")].pop(); if (dlg && (dlg.textContent || "").includes("Maintenance Window")) resolve(true); else if (Date.now() - started > 5000) reject(new Error("notification dialog did not appear")); else setTimeout(tick, 100); }; tick(); }))()',
    "notification dialog appears with expected text",
  );
  await golden.waitSelector('[data-testid="notif-ack-btn"]', "notification acknowledge button is visible");
});
