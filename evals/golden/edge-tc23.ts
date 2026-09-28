import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: javascript_error — the page intentionally throws an uncaught
// exception on load (window.onerror channel, not console.*). Covers the
// pageError claim: assert the exception was captured, and that no error
// contains a marker string we invent.
await runEdgeGolden(
  "tc23",
  "page throws on load — pageError claim sees the uncaught exception",
  "/javascript_error",
  "p",
  async (b) => {
    await b.openPage();
    await b.assertPageError(
      { text: "Cannot read" },
      "exists",
      undefined,
      "uncaught TypeError captured",
    );
    await b.assertPageError(
      { text: "QA_MARKER_NOT_PRESENT" },
      "notExists",
      undefined,
      "no error mentions our marker",
    );
  },
);
