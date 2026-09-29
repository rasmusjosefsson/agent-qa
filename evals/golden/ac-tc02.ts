import { runEdgeGolden } from "./edge-pages-lib";

// ac-tc02 — frames.html has a same-origin iframe (#frame1 → frame1.html)
// with its own button; the frame verb switches context mid-scenario.
await runEdgeGolden(
  "ac-tc02",
  "iframe — click inside #frame1, then assert back on the top document",
  "https://play1.automationcamp.ir/frames.html",
  "#frame1",
  async (g) => {
    await g.openPage();
    await g.assertElementPresent("#frame1", "iframe rendered");
    await g.enterFrame("#frame1", "switch into frame1");
    await g.waitSelector("button#click_me_1", "frame button ready");
    await g.clickSelector("button#click_me_1", "Click Me inside the frame");
    await g.waitSelectorText("body", "Clicked", "frame reports the click");
    await g.exitFrame("back to the top document");
    await g.assertElementAbsent("button#click_me_1", "frame button absent at top level");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
