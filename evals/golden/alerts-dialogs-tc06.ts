#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc06", "Alerts Dialogs TC06 prompt dismiss", async (golden) => {
  await golden.openFixture();
  await golden.clickSelector('[data-testid="btn-prompt-alert"]', "open prompt dialog");
  await golden.dialogDismiss("dismiss the prompt dialog");
  await golden.assertDialogClosed("prompt dialog is dismissed");
  await golden.assertSelectorTextEquals('[data-testid="result-prompt"]', "", "prompt result stays empty after dismiss");
});
