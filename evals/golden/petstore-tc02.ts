// petstore-tc02 — add to cart: item page → Add to Cart link → cart page
// shows the line item and subtotal. Cart.action responses carry the
// ;jsessionid= segment, so the network claim matches on the action name.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc02",
  "JPetStore — add EST-1 to the cart and see the subtotal",
  "https://petstore.octoperf.com/actions/Catalog.action?viewItem=&itemId=EST-1",
  'a[href*="addItemToCart"]',
  async (b) => {
    await b.openPage();
    await b.assertElementAttribute("body", "text", "contains", "Angelfish", "item page shows the product");
    await b.clickSelector('a[href*="addItemToCart"]', "add the item to the cart");
    await b.assertUrlContains("Cart.action", "landed on the cart");
    await b.assertNetworkStatus({ urlMatches: "Cart\\.action.*addItemToCart" }, "equals", "200", "cart-add request returned 200");
    await b.waitSelector("table", "cart table rendered");
    await b.assertElementAttribute("body", "text", "contains", "Sub Total: $16.50", "subtotal reflects the item");
    await b.assertElementAttribute("body", "text", "contains", "EST-1", "line item id shown");
  },
);
