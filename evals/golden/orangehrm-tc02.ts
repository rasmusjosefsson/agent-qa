import { runEdgeGolden } from "./edge-pages-lib";

// orangehrm-tc02 — login → Admin menu → filter users by username → the
// results table lists the row. Covers SPA nav (client-side route change),
// a form filter POST/GET to /api/v2/admin/users, and a rendered table.
await runEdgeGolden(
  "ohrm-tc02",
  "admin user search — SPA nav to Admin, username filter, results row",
  "https://opensource-demo.orangehrmlive.com/web/index.php/auth/login",
  "input[name=username]",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input[name=username]", "Admin", "username");
    await g.fillSelector("input[name=password]", "admin123", "password");
    await g.clickSelector("button[type=submit]", "submit login");
    await g.waitSelector(".oxd-topbar", "dashboard topbar renders");
    await g.clickSelector("a[href$='viewAdminModule']", "open Admin module");
    await g.waitSelector(".oxd-form", "search form renders");
    await g.fillSelector(
      ".oxd-form .oxd-input",
      "Admin",
      "username filter",
    );
    await g.clickSelector(".oxd-form-actions button[type=submit]", "run search");
    await g.waitSelector(".oxd-table-card", "result rows render");
    await g.waitSelectorText(".oxd-table", "Admin", "result row mentions Admin");
    await g.assertNetworkFired(
      { method: "GET", urlMatches: "api/v2/admin/users" },
      true,
      "user-list API fired",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
