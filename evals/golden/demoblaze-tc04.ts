import { runEdgeGolden } from "./edge-pages-lib";

// demoblaze-tc04 — full purchase flow: add to cart, Place Order modal,
// fill the payment form, confirm the SweetAlert receipt.
await runEdgeGolden(
  "db-tc04",
  "purchase flow — place-order modal, receipt, deletecart POST",
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
    await g.clickSelector("#cartur", "open cart");
    await g.waitSelector("#tbodyid .success", "cart row renders");
    await g.clickSelector("button.btn-success", "open Place Order modal");
    await g.waitSelector("#orderModal.show #name", "order modal open");
    await g.fillSelector("#name", "Qa Golden", "name");
    await g.fillSelector("#country", "Sweden", "country");
    await g.fillSelector("#city", "Stockholm", "city");
    await g.fillSelector("#card", "4111111111111111", "card");
    await g.fillSelector("#month", "09", "month");
    await g.fillSelector("#year", "2030", "year");
    await g.clickSelector("#orderModal button[onclick='purchaseOrder()']", "submit purchase");
    await g.waitSelector(".sweet-alert h2", "receipt renders");
    await g.waitSelectorText(".sweet-alert h2", "Thank you", "receipt title");
    await g.assertNetworkFired(
      { method: "POST", urlMatches: "deletecart" },
      true,
      "cart emptied server-side",
    );
    await g.clickSelector(".sweet-alert .confirm", "dismiss receipt");
    await g.waitSelectorAbsent(".sweet-alert", "receipt dismissed");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
  { keepDialogs: true },
);
