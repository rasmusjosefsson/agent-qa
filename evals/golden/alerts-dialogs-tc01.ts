#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc01", "Alerts Dialogs TC01 accept simple alert", async (golden) => {
  await golden.openFixture();
  await golden.clickSelector('[data-testid="btn-simple-alert"]', "open simple alert");
  await golden.assertDialogText("Welcome to QA PlayGround!", "alert message is readable while pending");
  await golden.dialogAccept("accept the alert");
  await golden.assertDialogClosed("alert is dismissed");
});
