import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: en.wikipedia.org — typeahead search navigates straight to
// the article when the term is an exact match (Terraform → /wiki/Terraform).
const BASE = "https://en.wikipedia.org";

await runEdgeGolden(
  "wiki-tc01",
  "search submits to the matching article",
  `${BASE}/wiki/Main_Page`,
  "input[name=search]",
  async (b) => {
    await b.openPage();
    await b.fillSelector(
      "input[name=search]",
      "Terraform",
      "type the search term",
    );
    await b.pressKey("Enter", "submit the search");
    await b.waitSelector("#firstHeading, h1", "article heading rendered");
    await b.assertUrlContains("/wiki/Terraform", "navigated to the article");
    await b.assertElementText(
      "#firstHeading, h1",
      "Terraform",
      "article title matches",
    );
    await b.assertNetworkStatus(
      { urlMatches: "w/load.php", method: "GET" },
      "equals",
      "200",
      "the module loader answered 200",
    );
  },
  { label: "wiki" },
);
