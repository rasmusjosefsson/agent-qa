import { runEdgeGolden } from "./edge-pages-lib";

// style:<prop> claims — #finish sits display:none until Start is clicked,
// then flips to block. Computed-style assertions can't be expressed by
// getAttribute or IDL properties.
await runEdgeGolden(
  "tc35",
  "dynamic_loading: #finish flips from display:none to block",
  "/dynamic_loading/1",
  "#start button",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("#finish", "finish div exists but hidden");
    await b.assertStyle("#finish", "display", "none", "hidden pre-click");
    await b.clickSelector("#start button", "start the load");
    await b.waitSelectorText("#finish", "Hello World!", "finish text appeared");
    await b.assertStyle("#finish", "display", "block", "visible post-load");
    await b.assertElementText("#finish", "Hello World!", "finish text correct");
  },
);
