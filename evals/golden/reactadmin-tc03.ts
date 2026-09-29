import { runEdgeGolden } from "./edge-pages-lib";

// reactadmin-tc03 — Posters catalog: SPA nav to a media-card grid; the
// poster list fetches from the cross-origin API. (Orders/Reviews lists
// are excluded — the demo's rotating seed data can leave them empty.)
await runEdgeGolden(
  "ra-tc03",
  "posters catalog — card grid + cross-origin posters fetch",
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
    await g.clickSelector("a[href='#/products']", "Posters nav");
    await g.waitSelectorText("body", "Posters", "posters page");
    await g.waitSelector(".MuiCard-root", "poster cards render");
    await g.assertNetworkFired(
      { method: "GET", urlMatches: "demo.api.marmelab.com/products" },
      true,
      "cross-origin products API fired",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
