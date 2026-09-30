import { runEdgeGolden } from "./edge-pages-lib";

// parabank-tc04 — register a user, then update contact info: exercises the
// services_proxy POST the profile page drives (the account-list API on this
// shared instance is intermittently 500, so account-dependent flows are
// intentionally not covered here).
await runEdgeGolden(
  "pb-tc04",
  "register, then update contact info — customer API round-trip",
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
    await g.gotoUrl(
      "https://parabank.parasoft.com/parabank/updateprofile.htm",
      "open update contact info",
    );
    await g.waitSelector("input[name='customer.phoneNumber']", "profile form renders");
    await g.fillSelector("input[name='customer.phoneNumber']", "5550009999", "change phone number");
    await g.clickSelector("input[value='Update Profile']", "submit profile update");
    await g.waitSelectorText("body", "Profile Updated", "update confirmation renders");
    await g.assertNetworkFired(
      { method: "POST", urlMatches: "services_proxy/bank/customers/update" },
      true,
      "customer update POST fired",
    );
    await g.assertNetworkStatus(
      { method: "POST", urlMatches: "services_proxy/bank/customers/update" },
      "equals",
      "200",
      "customer update returned 200",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
