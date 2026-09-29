import { runEdgeGolden } from "./edge-pages-lib";

// ac-tc04 — expected_conditions.html dialogs: "Show Alert" fires alert()
// and "Show Prompt" actually fires confirm() (accept → #confirm_ok,
// dismiss → #confirm_cancelled). keepDialogs leaves them pending so the
// recorded dialog checks + do/dialog pair replay them honestly.
await runEdgeGolden(
  "ac-tc04",
  "dialogs — alert accepted and confirm dismissed, markers shown",
  "https://play1.automationcamp.ir/expected_conditions.html",
  "#alert_trigger",
  async (g) => {
    await g.openPage();
    await g.fillSelector("#min_wait", "0", "min wait 0s");
    await g.fillSelector("#max_wait", "1", "max wait 1s");
    await g.clickSelector("#alert_trigger", "show alert");
    await g.waitMs(1500, "alert fires after the trigger delay");
    await g.assertDialogText("I am alerting you!", "alert text");
    await g.dialogAccept("accept the alert");
    await g.assertDialogClosed("alert cleared");
    await g.waitSelector("#alert_handled", "alert-handled marker shows");
    await g.clickSelector("#prompt_trigger", "show confirm");
    await g.waitMs(1500, "confirm fires after the trigger delay");
    await g.assertDialogText("Choose wisely", "confirm text");
    await g.dialogDismiss("dismiss the confirm");
    await g.waitSelector("#confirm_cancelled", "cancelled marker shows");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { keepDialogs: true },
);
