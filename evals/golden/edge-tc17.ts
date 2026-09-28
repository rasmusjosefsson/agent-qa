import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: HTML5 drag-and-drop — the do/drag verb drives a native
// dragstart→dragover→drop gesture that swaps the two column headers.
await runEdgeGolden(
  "tc17",
  "drag column A onto column B swaps their headers",
  "/drag_and_drop",
  "#column-a",
  async (b) => {
    await b.openPage();
    await b.dragSelector("#column-a", "#column-b", "drag A onto B");
    await b.assertElementText("#column-a header", "B", "column A now shows B");
    await b.assertElementText("#column-b header", "A", "column B now shows A");
  },
);
