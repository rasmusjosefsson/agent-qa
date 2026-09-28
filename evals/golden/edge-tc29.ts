import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: the site's own auth flow — login with flash message, secure
// area, logout with a second flash.
await runEdgeGolden(
  "tc29",
  "form authentication: login flash, secure area, logout flash",
  "/login",
  "input#username",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#username", "tomsmith", "username");
    await b.fillSelector("#password", "SuperSecretPassword!", "password");
    await b.clickSelector("button.radius", "log in");
    await b.waitSelector("#flash", "post-login flash");
    await b.assertElementAttribute(
      "#flash",
      "text",
      "contains",
      "You logged into a secure area",
      "success flash text",
    );
    await b.assertUrlContains("/secure", "on the secure area");
    await b.clickSelector("a.button.secondary", "log out");
    await b.waitSelector("#flash", "post-logout flash");
    await b.assertElementAttribute(
      "#flash",
      "text",
      "contains",
      "You logged out",
      "logout flash text",
    );
    await b.assertUrlContains("/login", "back on the login page");
  },
);
