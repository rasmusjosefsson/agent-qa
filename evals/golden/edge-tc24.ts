import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: HTTP basic auth — the site accepts the well-known public test
// credentials embedded in the URL (admin/admin). Covers navigations whose URL
// carries userinfo.
await runEdgeGolden(
  "tc24",
  "credentialed URL clears the basic-auth gate",
  "https://admin:admin@the-internet.herokuapp.com/basic_auth",
  ".example p",
  async (b) => {
    await b.openPage();
    await b.assertElementText(
      ".example p",
      "Congratulations! You must have the proper credentials.",
      "auth gate cleared",
    );
  },
);
