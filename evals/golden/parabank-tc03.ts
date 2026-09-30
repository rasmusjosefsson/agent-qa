import { runEdgeGolden } from "./edge-pages-lib";

// parabank-tc03 — login/logout round-trip with the seeded demo account
// (john/demo is the canonical public credential the site documents).
await runEdgeGolden(
  "pb-tc03",
  "login + logout round-trip on the seeded demo account",
  "https://parabank.parasoft.com/parabank/index.htm",
  "input[name='username']",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input[name='username']", "john", "username");
    await g.fillSelector("input[name='password']", "demo", "password");
    await g.clickSelector("input[value='Log In']", "submit login");
    await g.waitSelector("a[href*='logout']", "logged in — Log Out link appears");
    await g.assertCookie("JSESSIONID", true, "session cookie set");
    await g.assertNetworkFired({ method: "POST", urlMatches: "login" }, true, "login POST fired");
    await g.clickSelector("a[href*='logout']", "log out");
    await g.waitSelector("input[name='username']", "login form returns after logout");
    await g.assertElementPresent("#leftPanel a[href*='register.htm']", "register link back");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
