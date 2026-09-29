import { runEdgeGolden } from "./edge-pages-lib";

// reactadmin-tc04 — logout via the user-menu → back at the Sign in form.
await runEdgeGolden(
  "ra-tc04",
  "logout — user menu → logout → login form returns",
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
    await g.waitMs(500, "app bar settles after login");
    await g.clickSelector("button[aria-label='Profile']", "open user menu");
    await g.waitSelector(
      ".MuiPopover-root:not(.MuiModal-hidden)",
      "menu popover open",
    );
    await g.clickSelector(
      ".MuiPopover-root:not(.MuiModal-hidden) li:last-child",
      "Logout menu item",
    );
    await g.waitSelector(
      "input[name=username]",
      "back at the login form",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
