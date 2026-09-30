import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze-tc03 — cart persists across a full page reload (the server keys
// the cart on the anonymous `user` cookie). Exercises session persistence,
// a mid-scenario reload, and delete cleanup.
await runEdgeGolden(
  "db-tc03",
  "cart survives reload — anonymous session cookie, row still listed",
  "https://www.demoblaze.com/index.html",
  ".card-title a",
  async (g) => {
    await g.openPage();
    await g.clickSelector(".card-title a", "open first product");
    await g.waitSelector(".btn-success", "product page renders");
    await g.clickSelector("a.btn-success", "add to cart");
    await g.waitMs(2000, "addtocart POST settles before the alert");
    await g.assertDialogText("Product added", "add-to-cart alert text");
    await g.dialogAccept("accept product-added alert");
    await g.reload("reload mid-session");
    await g.clickSelector("#cartur", "open cart after reload");
    await g.waitSelector("#tbodyid .success", "cart row survives the reload");
    await g.clickSelector("#tbodyid .success td a", "delete the item");
    await g.waitSelectorAbsent("#tbodyid .success", "cart empty after delete");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { keepDialogs: true },
);
