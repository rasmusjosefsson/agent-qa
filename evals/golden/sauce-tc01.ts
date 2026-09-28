import { runEdgeGolden } from "./edge-pages-lib";

// New site: saucedemo.com login + inventory + client-side sort.
await runEdgeGolden(
  "s01",
  "saucedemo login, inventory grid, and price sort",
  "https://www.saucedemo.com/",
  "input#login-button",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#user-name", "standard_user", "username");
    await b.fillSelector("#password", "secret_sauce", "password");
    await b.clickSelector("#login-button", "log in");
    await b.waitSelector(".inventory_list", "inventory rendered");
    await b.assertElementPresent(
      ".inventory_item:nth-of-type(6)",
      "all six products rendered",
    );
    await b.assertElementAbsent(
      ".inventory_item:nth-of-type(7)",
      "exactly six products — no extras",
    );
    await b.selectOption("select.product_sort_container", "lohi", "sort low→high");
    await b.assertElementText(
      ".inventory_item:nth-of-type(1) .inventory_item_price",
      "$7.99",
      "cheapest item sorts first",
    );
  },
  { label: "sauce" },
);
