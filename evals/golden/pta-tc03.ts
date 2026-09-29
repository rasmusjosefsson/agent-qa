import { runEdgeGolden } from "./edge-pages-lib";

// pta-tc03 — login → Log out → back at the login form with the username
// field ready again.
await runEdgeGolden(
  "pta-tc03",
  "logout — Log out link returns to the login form",
  "https://practicetestautomation.com/practice-test-login/",
  "#username",
  async (g) => {
    await g.openPage();
    await g.fillSelector("#username", "student", "username");
    await g.fillSelector("#password", "Password123", "password");
    await g.clickSelector("#submit", "submit login");
    await g.waitSelectorText("h1", "Logged In Successfully", "logged in");
    await g.clickSelector(
      "a[href*='practice-test-login'], .wp-block-button a",
      "log out",
    );
    await g.waitSelector("#username", "back at login form");
    await g.assertUrlContains("practice-test-login", "login url again");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
