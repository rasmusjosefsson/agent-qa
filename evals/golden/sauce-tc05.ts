import { runEdgeGolden } from "./edge-pages-lib";

// New site: saucedemo.com cookie lifecycle — the session-username cookie
// exists after login and is gone after logout. First golden exercising the
// cookie claim subject (and storage's notExists on a site that sets none).
await runEdgeGolden(
  "s05",
  "saucedemo login sets session-username, logout clears it",
  "https://www.saucedemo.com/",
  "input#login-button",
  async (b) => {
    await b.openPage();
    await b.assertCookie("session-username", false, "no session cookie pre-login");
    await b.fillSelector("#user-name", "standard_user", "username");
    await b.fillSelector("#password", "secret_sauce", "password");
    await b.clickSelector("#login-button", "log in");
    await b.waitSelector(".inventory_list", "inventory rendered");
    await b.assertCookie("session-username", true, "session cookie set");
    await b.assertStorage(
      { key: "cart-contents", scope: "local" },
      false,
      "cart is empty before adding",
    );
    await b.clickSelectorForce("#add-to-cart-sauce-labs-backpack", "add backpack");
    await b.waitSelector(".shopping_cart_badge", "badge shows 1");
    await b.assertStorage(
      { key: "cart-contents", scope: "local" },
      true,
      "cart state tracked in localStorage",
    );
    await b.clickSelector("#react-burger-menu-btn", "open the menu");
    await b.waitSelector("#logout_sidebar_link", "drawer rendered");
    await b.waitMs(800, "drawer animation settles — link click point clears the header");
    // the link's click point sits under the header while the drawer
    // animates — agent-browser's live coverage check refuses it, so drive
    // the live click via el.click() (what replay does natively anyway).
    await b.clickSelectorForce("#logout_sidebar_link", "log out");
    await b.waitSelector("input#login-button", "back on the login screen");
    await b.assertCookie("session-username", false, "session cookie cleared");
  },
  { label: "sauce" },
);
