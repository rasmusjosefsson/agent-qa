import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: dynamic controls — Enable/Disable and Remove/Add buttons run an
// ajax round-trip that disables the control mid-flight, then flips state.
// Covers waits on text flips and element removal/return.
await runEdgeGolden(
  "tc16",
  "dynamic controls toggle input disablement and checkbox presence",
  "/dynamic_controls",
  "#input-example button",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#input-example button", "enable the input");
    await b.waitSelectorText("#message", "It's enabled!", "enable finished");
    await b.assertElementText("#input-example button", "Disable", "button flips to Disable");
    await b.clickSelector("#input-example button", "disable the input again");
    await b.waitSelectorText("#message", "It's disabled!", "disable finished");
    await b.clickSelector("#checkbox-example button", "remove the checkbox");
    await b.waitSelectorAbsent("#checkbox", "checkbox removed from DOM");
    await b.assertElementText("#message", "It's gone!", "removal banner shown");
    await b.clickSelector("#checkbox-example button", "add the checkbox back");
    await b.waitSelector("#checkbox", "checkbox re-added");
    await b.assertElementText("#message", "It's back!", "re-add banner shown");
  },
);
