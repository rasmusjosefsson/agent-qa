import { runEdgeGolden } from "./edge-pages-lib";

// parabank-tc02 — register a brand-new user. The username and password go
// through `{{vars._unique}}` templates so every replay mints fresh values
// instead of colliding on the already-taken username; password and confirm
// share the same minted token so they still match.
await runEdgeGolden(
  "pb-tc02",
  "register a unique user — success page, session cookie, logged-in nav",
  "https://parabank.parasoft.com/parabank/register.htm",
  "input[name='customer.username']",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input[name='customer.firstName']", "Qa", "first name");
    await g.fillSelector("input[name='customer.lastName']", "Golden", "last name");
    await g.fillSelector("input[name='customer.address.street']", "1 Golden Ave", "street");
    await g.fillSelector("input[name='customer.address.city']", "Golden City", "city");
    await g.fillSelector("input[name='customer.address.state']", "GA", "state");
    await g.fillSelector("input[name='customer.address.zipCode']", "12345", "zip");
    await g.fillSelector("input[name='customer.phoneNumber']", "5551234567", "phone");
    await g.fillSelector("input[name='customer.ssn']", "123456789", "ssn");
    await g.fillUnique("input[name='customer.username']", "qa-{{vars._unique}}", "unique username");
    await g.fillUnique("input[name='customer.password']", "pb-{{vars._unique}}", "unique password");
    await g.fillUnique("input[name='repeatedPassword']", "pb-{{vars._unique}}", "confirm password");
    await g.clickSelector("input[value='Register']", "submit registration");
    await g.waitSelectorText("body", "created successfully", "registration succeeds");
    await g.assertCookie("JSESSIONID", true, "session cookie set");
    await g.assertElementPresent("a[href*='logout']", "logged-in nav (Log Out) present");
    await g.assertNetworkFired({ method: "POST", urlMatches: "register" }, true, "registration POST fired");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
