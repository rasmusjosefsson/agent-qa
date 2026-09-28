import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: hover captions — mousing over a figure reveals its caption via
// CSS :hover, no click needed. Covers the hover verb + text claims.
await runEdgeGolden(
  "tc22",
  "hovering a figure reveals its user caption",
  "/hovers",
  ".figure",
  async (b) => {
    await b.openPage();
    await b.hoverSelector(".figure:nth-child(3)", "hover the first figure");
    await b.waitSelectorText(
      ".figure:nth-child(3) .figcaption h5",
      "name: user1",
      "caption revealed on hover",
    );
    await b.hoverSelector(".figure:nth-child(5)", "hover the last figure");
    await b.waitSelectorText(
      ".figure:nth-child(5) .figcaption h5",
      "name: user3",
      "third caption revealed",
    );
  },
);
