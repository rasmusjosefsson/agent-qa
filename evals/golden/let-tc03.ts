import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /frame — the frame verb against a real same-origin practice
// iframe (/frameui): enter, fill name fields, verify the values, leave back to
// the top document.
await runEdgeGolden(
  "let-tc03",
  "letcode frame page — drive inputs inside an iframe",
  "https://letcode.in/frame",
  "iframe[src*='frameui']",
  async (b) => {
    await b.openPage();
    await b.frameInto("iframe[src*='frameui']", "enter the practice frame");
    await b.fillInFrame(
      "iframe[src*='frameui']",
      "input[name='fname']",
      "Ada",
      "frame first name",
    );
    await b.fillInFrame(
      "iframe[src*='frameui']",
      "input[name='lname']",
      "Lovelace",
      "frame last name",
    );
    await b.assertElementAttribute(
      "input[name='fname']",
      "value",
      "equals",
      "Ada",
      "frame value claim",
    );
    await b.frameMain("back to the top document");
    await b.assertElementPresent("h1", "top doc renders again");
  },
  { label: "let" },
);
