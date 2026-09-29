#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze.com — the classic storefront: card grid → product detail →
// add-to-cart native alert → cart page row. Dialog auto-accept stays off
// so the alert text and its dismissal are both asserted.
await runEdgeGolden(
  "tc01", "demoblaze — browse to cart",
  "https://www.demoblaze.com/", ".card-block",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("#cat", "category rail rendered");
    await b.clickSelector('.card-title a[href="prod.html?idp_=1"]', "open the first product");
    await b.waitSelector(".name", "product name rendered");
    await b.assertElementAttribute(".name", "text", "equals", "Samsung galaxy s6", "product page shows the picked card");
    await b.clickSelector("a.btn-success", "add the product to cart");
    await b.assertDialogText("Product added", "add-to-cart fired its alert");
    await b.dialogAccept("accept the product-added alert");
    await b.assertDialogClosed("no dialog left pending");
    await b.clickSelector("#cartur", "open the cart");
    await b.waitSelectorText("#tbodyid", "Samsung galaxy s6", "cart lists the added product");
    await b.assertElementAttribute("#tbodyid", "text", "contains", "360", "cart row carries the price");
  },
  { label: "demoblaze", keepDialogs: true },
);
