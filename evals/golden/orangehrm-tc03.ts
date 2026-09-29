import { runEdgeGolden } from "./edge-pages-lib";

// orangehrm-tc03 — login → My Info → the personal-details form renders the
// logged-in user's fields (first/last name inputs are prefilled for Admin).
// Covers attribute assertions against server-rendered Vue form state.
await runEdgeGolden(
  "ohrm-tc03",
  "my info — personal details form prefills for the logged-in user",
  "https://opensource-demo.orangehrmlive.com/web/index.php/auth/login",
  "input[name=username]",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input[name=username]", "Admin", "username");
    await g.fillSelector("input[name=password]", "admin123", "password");
    await g.clickSelector("button[type=submit]", "submit login");
    await g.waitSelector(".oxd-topbar", "dashboard topbar renders");
    await g.clickSelector("a[href$='viewMyDetails']", "open My Info");
    await g.waitSelector(
      "input[name=firstName]",
      "personal details form renders",
    );
    await g.waitSelectorText(
      "body",
      "Personal Details",
      "personal details heading",
    );
    await g.assertElementAttribute(
      "input[name=firstName]",
      "value",
      "matches",
      ".+",
      "first name prefilled",
    );
    await g.assertNetworkFired(
      { method: "GET", urlMatches: "api/v2/pim/employees" },
      true,
      "employee-details API fired",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
