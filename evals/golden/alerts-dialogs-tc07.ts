#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc07", "Alerts Dialogs TC07 toast notification appears", async (golden) => {
  await golden.openPage();
  await golden.domClickSelector('[data-testid="btn-toast-alert"]', "trigger toast alert");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const toast = document.querySelector("[data-sonner-toast]"); if (toast && (toast.textContent || "").includes("This is simple toast")) resolve(true); else if (Date.now() - started > 3000) reject(new Error("toast did not appear with expected text")); else setTimeout(tick, 25); }; tick(); }))()',
    "toast appears with expected text",
  );
  await golden.waitSelector('[data-sonner-toast]', "toast notification appears");
});
