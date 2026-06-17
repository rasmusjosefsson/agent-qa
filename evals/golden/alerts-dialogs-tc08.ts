#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc08", "Alerts Dialogs TC08 close sweet alert modal", async (golden) => {
  await golden.openPage();
  await golden.domClickSelector('[data-testid="btn-modal-alert"]', "open sweet alert modal");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const dialog = document.querySelector("[role=alertdialog]"); if (dialog && (dialog.textContent || "").includes("Modern Alert")) resolve(true); else if (Date.now() - started > 5000) reject(new Error("sweet alert modal did not open")); else setTimeout(tick, 100); }; tick(); }))()',
    "sweet alert modal opens",
  );
  await golden.waitSelector('[role="alertdialog"]', "sweet alert modal is visible");
  await golden.clickSelector('[data-testid="btn-modal-cancel"]', "close sweet alert with cancel button");
  await golden.waitSelectorAbsent('[role="alertdialog"]', "sweet alert modal is dismissed");
});
