import { runEdgeGolden } from "./edge-pages-lib";

// reactadmin-tc01 — login with the page's own demo/demo creds → dashboard
// renders the welcome card + monthly revenue chart title.
await runEdgeGolden(
  "ra-tc01",
  "login → dashboard — welcome card + revenue heading",
  "https://marmelab.com/react-admin-demo/",
  "input[name=username]",
  async (g) => {
    await g.openPage();
    await g.fillSelector("input[name=username]", "demo", "username");
    await g.fillSelector("input[name=password]", "demo", "password");
    await g.clickSelector("button[type=submit]", "sign in");
    await g.waitSelectorText(
      "body",
      "Welcome to the react-admin e-commerce demo",
      "dashboard welcome card",
    );
    await g.waitSelectorText("body", "Monthly Revenue", "revenue chart card");
    await g.assertUrlContains("#/", "on the SPA route");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
