import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: broken images — the page embeds two <img> pointing at 404s.
// Covers network claims: assert the requests fired AND returned 404, while
// the real avatar loaded fine.
await runEdgeGolden(
  "tc28",
  "broken images 404 in the request log while the avatar loads",
  "/broken_images",
  ".example img",
  async (b) => {
    await b.openPage();
    await b.assertNetworkStatus(
      { urlMatches: "asdf.jpg" },
      "equals",
      "404",
      "first broken image 404'd",
    );
    await b.assertNetworkStatus(
      { urlMatches: "hjkl.jpg" },
      "equals",
      "404",
      "second broken image 404'd",
    );
    await b.assertNetworkStatus(
      { urlMatches: "avatar-blank.jpg" },
      "equals",
      "200",
      "the real avatar loaded",
    );
    await b.assertNetworkStatus(
      { urlMatches: "the-internet.herokuapp.com/broken" },
      "equals",
      "200",
      "the document itself loaded",
    );
  },
);
