import { runEdgeGolden } from "./edge-pages-lib";

// parabank-tc01 — landing page smoke + register-link navigation.
// Server-rendered JSP site: no XHR to render, so a services_proxy silence
// claim covers the negative side.
await runEdgeGolden(
  "pb-tc01",
  "landing renders the login form and the register link navigates",
  "https://parabank.parasoft.com/parabank/index.htm",
  "input[name='username']",
  async (g) => {
    await g.openPage();
    await g.assertElementPresent("input[name='username']", "username field present");
    await g.assertElementPresent("input[name='password']", "password field present");
    await g.assertElementPresent("#leftPanel a[href*='register.htm']", "register link present");
    await g.clickSelector("#leftPanel a[href*='register.htm']", "open the registration page");
    await g.waitSelector("input[name='customer.username']", "registration form renders");
    await g.assertElementPresent("input[name='customer.firstName']", "first name field present");
    await g.assertElementPresent("input[name='repeatedPassword']", "confirm password field present");
    await g.assertNetworkSilent({ urlMatches: "services_proxy" }, "register page made no API calls");
  },
);
