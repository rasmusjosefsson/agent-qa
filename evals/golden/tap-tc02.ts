#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// testautomationpractice.blogspot.com — native dialogs: alert, confirm, prompt.
// keepDialogs lets the recorded dialog verbs carry through to the buffer.
await runEdgeGolden(
  "tc02", "TAP — native dialogs",
  "https://testautomationpractice.blogspot.com/", "#alertBtn",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#alertBtn", "trigger the alert button");
    await b.assertDialogText("I am an alert box!", "the alert carried its text");
    await b.dialogAccept("accept the alert");
    await b.assertDialogClosed("no dialog left pending");
    await b.clickSelector("#confirmBtn", "trigger the confirm button");
    await b.assertDialogText("Press a button!", "the confirm carried its text");
    await b.dialogAccept("accept the confirm");
    await b.assertElementAttribute("#demo", "text", "contains", "OK", "confirm accept echoed OK");
    await b.clickSelector("#promptBtn", "trigger the prompt button");
    await b.assertDialogText("Please enter your name", "the prompt carried its text");
    await b.dialogAccept("answer the prompt", "Golden User");
    await b.assertElementAttribute("#demo", "text", "contains", "Golden User", "prompt answer echoed into the page");
  },
  { label: "tap", keepDialogs: true },
);
