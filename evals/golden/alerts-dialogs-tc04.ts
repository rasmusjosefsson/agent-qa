#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc04", "Alerts Dialogs TC04 dismiss confirm", async (golden) => {
  await golden.openFixture();
  await golden.clickSelector('[data-testid="btn-confirm-alert"]', "open confirm dialog");
  await golden.dialogDismiss("dismiss the confirm dialog");
  await golden.waitSelectorText('[data-testid="result-confirm"]', "Dismissed", "result shows Dismissed");
});
