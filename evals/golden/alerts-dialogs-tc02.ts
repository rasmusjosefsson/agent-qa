#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc02", "Alerts Dialogs TC02 read alert text", async (golden) => {
  await golden.openFixture();
  await golden.clickSelector('[data-testid="btn-simple-alert"]', "open simple alert");
  await golden.assertDialogText("Welcome to QA PlayGround!", "alert text matches before accepting");
  await golden.dialogAccept("accept the alert after reading");
});
