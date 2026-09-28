import { runEdgeGolden } from "./edge-pages-lib";

// New site: saucedemo.com negative auth — locked_out_user gets an inline
// error and never reaches the inventory.
await runEdgeGolden(
  "s03",
  "saucedemo locked-out user sees the error banner",
  "https://www.saucedemo.com/",
  "input#login-button",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#user-name", "locked_out_user", "username");
    await b.fillSelector("#password", "secret_sauce", "password");
    await b.clickSelector("#login-button", "log in");
    await b.waitSelector("[data-test='error']", "error banner rendered");
    await b.assertElementAttribute(
      "[data-test='error']",
      "text",
      "contains",
      "locked out",
      "locked-out message shown",
    );
    await b.assertElementAbsent(".inventory_list", "no inventory behind the error");
  },
  { label: "sauce" },
);
