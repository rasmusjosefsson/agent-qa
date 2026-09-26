#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc03", "Alerts Dialogs TC03 accept confirm", async (golden) => {
  await golden.openFixture();
  await golden.clickSelector('[data-testid="btn-confirm-alert"]', "open confirm dialog");
  await golden.dialogAccept("accept the confirm dialog");
  await golden.waitSelectorText('[data-testid="result-confirm"]', "Accepted", "result shows Accepted");
});
