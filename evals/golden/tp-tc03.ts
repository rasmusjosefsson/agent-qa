import { runEdgeGolden } from "./edge-pages-lib.ts";

// testpages.eviltester.com dialogs: alert/confirm/prompt with retval
// spans the page writes after each dialog resolves.
await runEdgeGolden(
  "tp-tc03",
  "alert, confirm and prompt dialogs capture and resolve",
  "https://testpages.eviltester.com/styled/alerts/alert-test.html",
  "#alertexamples",
  async (b) => {
    await b.openPage();
    // Buttons sit below the fold in the 800px-tall replay viewport — a
    // synthetic click on an unscrolled element dispatches to nothing.
    await b.scrollToSelector("#alertexamples", "scroll the alert button up");
    await b.clickSelector("#alertexamples", "raise an alert");
    await b.assertDialogText("I am an alert box!", "alert text captured");
    await b.dialogAccept("accept the alert");
    await b.assertDialogClosed("alert resolved");
    await b.assertElementAttribute(
      "#alertexplanation",
      "text",
      "contains",
      "handled the alert dialog",
      "page confirms the alert was handled",
    );
    await b.scrollToSelector("#confirmexample", "scroll to the confirm");
    await b.clickSelector("#confirmexample", "raise a confirm");
    await b.assertDialogText("I am a confirm alert", "confirm captured");
    await b.dialogDismiss("cancel the confirm");
    await b.assertDialogClosed("confirm resolved");
    await b.assertElementAttribute(
      "#confirmreturn",
      "text",
      "equals",
      "false",
      "confirm returned false",
    );
    await b.scrollToSelector("#promptexample", "scroll to the prompt");
    await b.clickSelector("#promptexample", "raise a prompt");
    await b.assertDialogText("I prompt you", "prompt captured");
    await b.dialogAccept("answer the prompt", "typed by devin");
    await b.assertDialogClosed("prompt resolved");
    await b.assertElementAttribute(
      "#promptreturn",
      "text",
      "equals",
      "typed by devin",
      "prompt value echoed",
    );
  },
  { keepDialogs: true, label: "tp" },
);
