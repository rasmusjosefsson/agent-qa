import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze-tc02 — add a product to the cart and remove it. The "Add to
// cart" click raises a native alert which the golden records as dialogText
// + dialogAccept steps; the cart row id is a volatile uuid so the delete
// locator anchors on the row itself.
await runEdgeGolden(
  "db-tc02",
  "add to cart — alert claim, cart row renders, delete removes it",
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
    await g.assertNetworkFired(
      { method: "POST", urlMatches: "addtocart" },
      true,
      "addtocart POST fired",
    );
    await g.clickSelector("#cartur", "open cart");
    await g.waitSelector("#tbodyid .success", "cart row renders");
    await g.assertElementPresent("#tbodyid .success td", "cart item present");
    await g.clickSelector("#tbodyid .success td a", "delete the item");
    await g.waitSelectorAbsent("#tbodyid .success", "cart row removed");
    await g.assertNetworkFired(
      { method: "POST", urlMatches: "deleteitem" },
      true,
      "deleteitem POST fired",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { keepDialogs: true },
);
