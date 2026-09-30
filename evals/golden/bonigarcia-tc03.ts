// bonigarcia-tc03 — drag-and-drop via jQuery UI: #draggable is a
// draggable() (mouse-driven), not an HTML5 drag source, so the live drive
// uses a real mousedown→move→up sequence and replay exercises do/drag's
// trusted-mouse path. Success = jQuery UI stamped an inline left offset.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc03",
  "Bonigarcia — drag the panel onto the drop zone",
  "https://bonigarcia.dev/selenium-webdriver-java/drag-and-drop.html",
  "#draggable",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("#target", "drop zone rendered");
    await b.assertElementAttribute("#draggable", "style", "equals", "", "no offset before the drag");
    await b.dragSelectorMouse("#draggable", "#target", "drag the panel onto the zone");
    await b.assertElementAttribute("#draggable", "style", "contains", "left", "jQuery UI stamped a left offset");
  },
);
