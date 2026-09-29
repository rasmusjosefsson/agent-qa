import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: en.wikipedia.org — article structure: TOC sidebar links to
// in-page anchors; clicking one updates the URL fragment.
const BASE = "https://en.wikipedia.org";

await runEdgeGolden(
  "wiki-tc02",
  "article TOC navigates to an in-page section",
  `${BASE}/wiki/Terraform`,
  "#vector-toc",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent(
      "#vector-toc",
      "table of contents rendered",
    );
    await b.clickSelector(
      "#vector-toc a",
      "follow the first section link",
    );
    await b.assertUrlContains("#", "fragment navigation happened");
    await b.assertElementPresent(
      "h2",
      "section headings exist",
    );
    await b.assertNetworkStatus(
      { urlMatches: "en.wikipedia.org", method: "GET" },
      "equals",
      "200",
      "article page answered 200",
    );
  },
  { label: "wiki" },
);
