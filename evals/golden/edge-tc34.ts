import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: a bare input[type=number] — type digits, assert the live
// value; the page is otherwise empty so this is the minimal form case.
await runEdgeGolden(
  "tc34",
  "number input accepts typed digits and exposes the value",
  "/inputs",
  "input[type=number]",
  async (b) => {
    await b.openPage();
    await b.fillSelector("input[type=number]", "42", "type 42");
    await b.assertElementAttribute(
      "input[type=number]",
      "value",
      "equals",
      "42",
      "input holds 42",
    );
    await b.fillSelector("input[type=number]", "137", "retype 137");
    await b.assertElementAttribute(
      "input[type=number]",
      "value",
      "equals",
      "137",
      "input holds 137",
    );
  },
);
