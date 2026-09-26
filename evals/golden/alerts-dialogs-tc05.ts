#!/usr/bin/env bun
import { runAlertsDialogsGolden } from "./alerts-dialogs-lib.ts";

await runAlertsDialogsGolden("tc05", "Alerts Dialogs TC05 prompt accept with text", async (golden) => {
  await golden.openFixture();
  await golden.clickSelector('[data-testid="btn-prompt-alert"]', "open prompt dialog");
  await golden.dialogAccept("enter name and accept the prompt", "John Doe");
  await golden.waitSelectorText('[data-testid="result-prompt"]', "Your name is - John Doe", "prompt result shows entered name");
});
