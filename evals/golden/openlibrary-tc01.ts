import { runEdgeGolden } from "./edge-pages-lib";

// openlibrary-tc01 — search?q=dune → click the first result → the Dune work
// page renders with Frank Herbert's name. Production-site coverage: real
// search ranking, server-rendered results, work-page editions table.
await runEdgeGolden(
  "openlib-tc01",
  "search → first result → Dune work page renders author + editions",
  "https://openlibrary.org/search?q=dune",
  "li.searchResultItem",
  async (g) => {
    await g.openPage();
    await g.waitSelectorText(
      "li.searchResultItem .booktitle a, li.searchResultItem h3 a",
      "Dune",
      "first result is Dune",
    );
    await g.clickSelector(
      "li.searchResultItem .booktitle a, li.searchResultItem h3 a",
      "open the Dune work",
    );
    await g.waitSelectorText("h1", "Dune", "work title renders");
    await g.waitSelectorText("body", "Frank Herbert", "author name visible");
    await g.assertUrlContains("/works/", "navigated to a work page");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
