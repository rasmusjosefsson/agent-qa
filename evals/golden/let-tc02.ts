import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /alert — all three native dialog kinds in one case: alert accept,
// confirm accept, and prompt with a response value the page echoes into
// #myName.
await runEdgeGolden(
  "let-tc02",
  "letcode alert page — alert/confirm/prompt round-trip",
  "https://letcode.in/alert",
  "#accept",
  async (b) => {
    await b.openPage();

    await b.clickSelector("#accept", "open the simple alert");
    await b.assertDialogText("Welcome to LetCode", "alert text");
    await b.dialogAccept("accept the alert");
    await b.assertDialogClosed("alert closed");

    await b.clickSelector("#confirm", "open the confirm dialog");
    await b.assertDialogText("happy with LetCode", "confirm text");
    await b.dialogAccept("accept the confirm");
    await b.assertDialogClosed("confirm closed");

    await b.clickSelector("#prompt", "open the prompt dialog");
    await b.assertDialogText("Enter your name", "prompt text");
    await b.dialogAccept("answer the prompt", "Koushik");
    await b.assertDialogClosed("prompt closed");
    await b.assertElementAttribute(
      "#myName",
      "text",
      "contains",
      "Koushik",
      "prompt value echoed",
    );
  },
  { label: "let", keepDialogs: true },
);
