import { runEdgeGolden } from "./edge-pages-lib";

// New site: saucedemo.com end-to-end purchase — cart badge, cart page,
// checkout form, order confirmation.
await runEdgeGolden(
  "s02",
  "saucedemo add-to-cart through order confirmation",
  "https://www.saucedemo.com/",
  "input#login-button",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#user-name", "standard_user", "username");
    await b.fillSelector("#password", "secret_sauce", "password");
    await b.clickSelector("#login-button", "log in");
    await b.waitSelector(".inventory_list", "inventory rendered");

    await b.clickSelector(
      "#add-to-cart-sauce-labs-backpack",
      "add the backpack",
    );
    await b.assertElementText(".shopping_cart_badge", "1", "cart badge = 1");
    await b.clickSelector(".shopping_cart_link", "open the cart");
    await b.waitSelector(".cart_item", "cart page rendered");
    await b.assertElementText(
      ".inventory_item_name",
      "Sauce Labs Backpack",
      "cart holds the backpack",
    );

    await b.clickSelector("#checkout", "checkout");
    await b.waitSelector("#first-name", "checkout form rendered");
    await b.fillSelector("#first-name", "QA", "first name");
    await b.fillSelector("#last-name", "Bot", "last name");
    await b.fillSelector("#postal-code", "12345", "postal code");
    await b.clickSelector("#continue", "continue to overview");
    await b.waitSelector(".summary_total_label", "order summary rendered");
    await b.assertElementAttribute(
      ".summary_total_label",
      "text",
      "contains",
      "Total: $",
      "total line renders",
    );
    await b.clickSelector("#finish", "finish the order");
    await b.assertElementText(
      ".complete-header",
      "Thank you for your order!",
      "order confirmation",
    );
  },
  { label: "sauce" },
);
