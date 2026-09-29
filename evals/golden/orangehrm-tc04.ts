import { runEdgeGolden } from "./edge-pages-lib";

// orangehrm-tc04 — login → user dropdown → Logout → back at the login
// form (the demo's creds banner returns) and the logout request fired.
await runEdgeGolden(
  "ohrm-tc04",
  "logout — user dropdown, logout link, back at login form",
  "https://opensource-demo.orangehrmlive.com/web/index.php/auth/login",
  "input[name=username]",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input[name=username]", "Admin", "username");
    await g.fillSelector("input[name=password]", "admin123", "password");
    await g.clickSelector("button[type=submit]", "submit login");
    await g.waitSelector(".oxd-topbar", "dashboard topbar renders");
    await g.assertCookie("orangehrm", true, "session cookie set");
    await g.clickSelector(".oxd-userdropdown-tab", "open user dropdown");
    await g.clickSelector("a[href$='/auth/logout']", "logout");
    await g.waitSelector("input[name=username]", "back at login form");
    await g.waitSelectorText(
      "body",
      "Username : Admin",
      "demo creds banner visible again",
    );
    await g.assertNetworkFired(
      { method: "GET", urlMatches: "auth/logout" },
      true,
      "logout request fired",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
