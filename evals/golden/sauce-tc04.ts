import { runEdgeGolden } from "./edge-pages-lib";

// New site: saucedemo.com logout through the burger menu — a slide-in
// drawer + a link that returns to the login screen.
await runEdgeGolden(
  "s04",
  "saucedemo logout via the burger menu drawer",
  "https://www.saucedemo.com/",
  "input#login-button",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#user-name", "standard_user", "username");
    await b.fillSelector("#password", "secret_sauce", "password");
    await b.clickSelector("#login-button", "log in");
    await b.waitSelector(".inventory_list", "inventory rendered");
    await b.clickSelector("#react-burger-menu-btn", "open the menu");
    await b.waitSelector("#logout_sidebar_link", "drawer rendered");
    await b.waitMs(800, "drawer animation settles — link click point clears the header");
    await b.clickSelectorForce("#logout_sidebar_link", "log out");
    await b.assertElementPresent(
      "input#login-button",
      "back on the login screen",
    );
    await b.assertElementAbsent(".inventory_list", "inventory gone after logout");
  },
  { label: "sauce" },
);
