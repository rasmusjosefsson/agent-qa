import { runEdgeGolden } from "./edge-pages-lib";

// reactadmin-tc02 — login → Customers nav → the customer card grid → the
// cross-origin demo.api.marmelab.com/customers fetch is claimed.
await runEdgeGolden(
  "ra-tc02",
  "customers grid — SPA nav + cross-origin API claim",
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
      "dashboard renders",
    );
    await g.clickSelector("a[href='#/customers']", "Customers nav");
    await g.waitSelectorText("body", "Customers", "customers page");
    await g.waitSelector(".MuiAvatar-root", "customer avatars render");
    await g.assertNetworkFired(
      { method: "GET", urlMatches: "demo.api.marmelab.com/customers" },
      true,
      "cross-origin customers API fired",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
