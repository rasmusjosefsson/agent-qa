import { runEdgeGolden } from "./edge-pages-lib";

// saucedemo session seeding — do/state plants the session-username cookie
// before navigation so the run starts already logged in, no form round-trip.
// Exercises the state verb + cookie claim end to end.
await runEdgeGolden(
  "s06",
  "saucedemo do/state cookie seed lands on inventory",
  "https://www.saucedemo.com/",
  "input#login-button",
  async (b) => {
    await b.openPage();
    await b.assertCookie("session-username", false, "no session cookie yet");
    await b.seedCookie("session-username", "standard_user", "plant the session cookie");
    await b.assertCookie("session-username", true, "cookie readable after seed");
    await b.gotoUrl("https://www.saucedemo.com/inventory.html", "straight to inventory");
    await b.waitSelector(".inventory_list", "logged-in view without the form");
    await b.assertElementPresent(".inventory_item", "inventory rendered for the seeded session");
    await b.assertElementAbsent("input#login-button", "login form bypassed");
  },
  { label: "sauce" },
);
