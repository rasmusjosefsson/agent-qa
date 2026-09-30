// bonigarcia-tc04 — native dialogs + a Bootstrap modal: alert/confirm/
// prompt all record their text + the chosen answer flows back into the
// page. keepDialogs because the page's own handlers must see real dialogs.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc04",
  "Bonigarcia — accept/dismiss/type into native dialogs, then close the modal",
  "https://bonigarcia.dev/selenium-webdriver-java/dialog-boxes.html",
  "#my-alert",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#my-alert", "open the alert");
    await b.assertDialogText("Hello world!", "alert carried its message");
    await b.dialogAccept("accept the alert");

    await b.clickSelector("#my-confirm", "open the confirm");
    await b.assertDialogText("Is this correct?", "confirm carried its message");
    await b.dialogDismiss("dismiss the confirm");
    await b.waitSelectorText("#confirm-text", "You chose: false", "page shows the dismissed result");

    await b.clickSelector("#my-prompt", "open the prompt");
    await b.assertDialogText("Please enter your name", "prompt carried its message");
    await b.dialogAccept("answer the prompt", "Devin");
    await b.waitSelectorText("#prompt-text", "You typed: Devin", "page shows the typed answer");

    await b.clickSelector("#my-modal", "open the modal");
    await b.waitSelector("#example-modal.show", "modal visible");
    await b.clickSelector("#example-modal .btn-primary", "press Save changes");
    await b.waitSelectorText("#modal-text", "You chose: Save", "page shows the modal choice");
  },
  { keepDialogs: true },
);
