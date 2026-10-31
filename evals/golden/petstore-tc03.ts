// petstore-tc03 — sign in with the shipped demo account (j2ee/j2ee) and
// claim the signed-in chrome: the Sign Out link replaces Sign In.
// Spring path: GET /account, POST /account/signon.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc03",
  "JPetStore — sign in as j2ee",
  "https://petstore.octoperf.com/account",
  "input[name=username]",
  async (b) => {
    await b.openPage();
    await b.fillSelector("input[name=username]", "j2ee", "fill username");
    await b.fillSelector("input[name=password]", "j2ee", "fill password");
    await b.clickSelector("input[value=Login]", "submit the signon form");
    await b.waitSelector('a[href*="signoff"]', "Sign Out link appeared");
    await b.assertElementAttribute("body", "text", "contains", "Welcome", "welcome banner shown");
    await b.assertNetworkFired({ urlMatches: "/account/signon", method: "POST" }, true, "signon POST sent");
  },
);
