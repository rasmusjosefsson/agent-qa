import { runEdgeGolden } from "./edge-pages-lib";

// orangehrm-tc01 — login with the demo creds shown on the login page, land
// on the dashboard shell. The site is a Vue SPA backed by /api/v2/* calls,
// so the suite asserts the SPA nav, session cookie, and auth POST.
await runEdgeGolden(
  "ohrm-tc01",
  "login — dashboard shell renders, orangehrm cookie set, auth POST fired",
  "https://opensource-demo.orangehrmlive.com/web/index.php/auth/login",
  "input[name=username]",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input[name=username]", "Admin", "username");
    await g.fillSelector("input[name=password]", "admin123", "password");
    await g.clickSelector("button[type=submit]", "submit login");
    await g.waitSelector(".oxd-topbar", "dashboard topbar renders");
    await g.waitSelectorText("body", "Dashboard", "dashboard heading visible");
    await g.assertCookie("orangehrm", true, "session cookie set");
    await g.assertNetworkFired(
      { method: "POST", urlMatches: "auth/validate" },
      true,
      "login POST fired",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
