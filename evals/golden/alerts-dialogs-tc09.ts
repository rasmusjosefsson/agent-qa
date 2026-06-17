#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc09", "Alerts Dialogs TC09 close advanced share dialog", async (golden) => {
  await golden.openPage();
  await golden.domClickSelector('[data-testid="btn-dialog-share"]', "open share dialog");
  await golden.assertLiveCondition(
    '(() => new Promise((resolve, reject) => { const started = Date.now(); const tick = () => { const input = document.querySelector("[data-testid=input-share-link]"); if (input && input.value.includes("qaplayground.com/practice/alerts-dialogs")) resolve(true); else if (Date.now() - started > 5000) reject(new Error("share link input did not contain expected value")); else setTimeout(tick, 100); }; tick(); }))()',
    "share link input has expected value",
  );
  await golden.waitSelector('[role="dialog"] [data-testid="input-share-link"]', "share link input is visible");
  await golden.clickSelector('[data-testid="btn-dialog-close"]', "close share dialog");
  await golden.waitSelectorAbsent('[role="dialog"] [data-testid="input-share-link"]', "share dialog is dismissed");
});
