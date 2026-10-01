#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// rahulshettyacademy.com/AutomationPractice — native alert + confirm
// dialogs seeded by a typed name; confirm exercised both ways since the
// page echoes no outcome.
await runEdgeGolden(
  "tc02", "rahulshetty practice — alert and confirm dialogs",
  "https://rahulshettyacademy.com/AutomationPractice/",
  "#name",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#name", "Golden User", "fill the name box");
    await b.clickSelector("#alertbtn", "trigger the alert");
    await b.assertDialogText("Golden User", "alert echoed the name");
    await b.dialogAccept("accept the alert");
    await b.assertDialogClosed("no dialog left pending");
    await b.clickSelector("#confirmbtn", "trigger the confirm");
    await b.assertDialogText("Are you sure", "confirm asked the question");
    await b.dialogDismiss("dismiss the confirm");
    await b.assertDialogClosed("confirm dismissed");
    await b.clickSelector("#confirmbtn", "trigger the confirm again");
    await b.dialogAccept("accept the confirm this time");
    await b.assertDialogClosed("no dialog left after accept");
  },
  { label: "rs", keepDialogs: true },
);
