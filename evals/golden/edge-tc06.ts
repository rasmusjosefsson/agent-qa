#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc06", "Edge — native JS confirm: dismiss path",
  "/javascript_alerts", 'button[onclick="jsConfirm()"]',
  async (b) => {
    await b.openPage();
    await b.clickSelector('button[onclick="jsConfirm()"]', "open the JS confirm");
    await b.assertDialogText("I am a JS Confirm", "confirm message readable while pending");
    await b.dialogDismiss("dismiss the confirm");
    await b.assertDialogClosed("confirm is dismissed");
    await b.assertElementText("#result", "You clicked: Cancel", "page echoes the cancel");
  },
  { keepDialogs: true },
);
