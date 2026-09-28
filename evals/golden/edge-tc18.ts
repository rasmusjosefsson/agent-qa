import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: new-window link — target=_blank opens a second tab; the `tab`
// verb switches the session between them.
await runEdgeGolden(
  "tc18",
  "click opens a new tab; tab verb switches between them",
  "/windows",
  ".example a",
  async (b) => {
    await b.openPage();
    await b.clickSelector(".example a", "open the new-window link");
    await b.tabCommand("t2", "switch to the new tab");
    await b.assertUrlContains("/windows/new", "new tab on the child URL");
    await b.assertElementText("h3", "New Window", "new window content rendered");
    await b.tabCommand("t1", "switch back to the opener");
    await b.assertUrlContains("/windows", "back on the opener");
  },
);
