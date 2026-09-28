#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

await runEdgeGolden(
  "tc05", "Edge — native JS alert: observe text, accept, verify result",
  "/javascript_alerts", 'button[onclick="jsAlert()"]',
  async (b) => {
    await b.openPage();
    await b.clickSelector('button[onclick="jsAlert()"]', "open the JS alert");
    await b.assertDialogText("I am a JS Alert", "alert message readable while pending");
    await b.dialogAccept("accept the alert");
    await b.assertDialogClosed("alert is dismissed");
    await b.assertElementText("#result", "You successfully clicked an alert", "page echoes the accept");
  },
  { keepDialogs: true },
);
