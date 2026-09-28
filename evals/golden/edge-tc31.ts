import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: content that randomizes every load — assertions pin only the
// stable shape (three rows, each with an image + a non-empty paragraph),
// never the random payload itself.
await runEdgeGolden(
  "tc31",
  "dynamic content randomizes but keeps its shape",
  "/dynamic_content",
  ".large-10 img",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent(
      ".large-10 .row:nth-of-type(3) img",
      "three content rows render",
    );
    await b.assertElementAbsent(
      ".large-10 .row:nth-of-type(4)",
      "no fourth row",
    );
    await b.assertElementAttribute(
      ".large-10 .row:nth-of-type(1) .large-10.columns",
      "text",
      "matches",
      ".{20,}",
      "first row has real text (not the empty shell)",
    );
    await b.reload("reload for a fresh draw");
    await b.waitSelector(".large-10 img", "rows re-rendered after reload");
    await b.assertElementPresent(
      ".large-10 .row:nth-of-type(3) img",
      "shape stable across reloads",
    );
  },
);
