import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: en.wikipedia.org — View history tab shows the revision list.
const BASE = "https://en.wikipedia.org";

await runEdgeGolden(
  "wiki-tc03",
  "view history lists page revisions",
  `${BASE}/wiki/Terraform`,
  "#vector-toc",
  async (b) => {
    await b.openPage();
    await b.clickSelector(
      "a[href*='action=history']",
      "open the View history tab",
    );
    await b.waitSelector("#pagehistory", "history list rendered");
    await b.assertUrlContains(
      "action=history",
      "landed on the history view",
    );
    await b.assertElementPresent(
      "#pagehistory li",
      "revision rows listed",
    );
    await b.assertElementPresent(
      "#firstHeading",
      "history heading rendered",
    );
  },
  { label: "wiki" },
);
