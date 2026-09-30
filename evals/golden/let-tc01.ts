import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /edit — input interactions and element-state claims: fill fields,
// read a preset value, assert disabled and readonly attributes.
await runEdgeGolden(
  "let-tc01",
  "letcode edit page — fills and element-state claims",
  "https://letcode.in/edit",
  "#fullName",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#fullName", "Ada Lovelace", "full name");
    await b.fillSelector("#join", " plus tooling", "join field");
    await b.assertElementAttribute(
      "#getMe",
      "value",
      "contains",
      "ortonikc",
      "preset value is readable",
    );
    await b.assertElementAttribute(
      "#noEdit",
      "disabled",
      "equals",
      "true",
      "noEdit is disabled",
    );
    await b.assertElementAttribute(
      "#dontwrite",
      "value",
      "contains",
      "readonly",
      "dontwrite shows its readonly value",
    );
    await b.assertElementAttribute(
      "#dontwrite",
      "readOnly",
      "equals",
      "true",
      "dontwrite is readonly",
    );
  },
  { label: "let" },
);
