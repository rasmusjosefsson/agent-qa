import { runEdgeGolden } from "./edge-pages-lib";

// books-tc02 — category browse + pagination: Travel category filters the
// catalogue; the Next pager advances to page 2 of the filtered list.
await runEdgeGolden(
  "books-tc02",
  "Fiction category → next page — filtered catalogue + pagination",
  "https://books.toscrape.com/",
  "article.product_pod",
  async (g) => {
    await g.openPage();
    await g.clickSelector(
      ".nav-list ul li a[href$='fiction_10/index.html']",
      "open Fiction category",
    );
    await g.waitSelectorText(
      ".page-header h1, h1",
      "Fiction",
      "Fiction category heading",
    );
    await g.waitSelectorText(
      "article.product_pod",
      "Soumission",
      "fiction title listed",
    );
    await g.clickSelector("li.next a", "next page");
    await g.assertUrlContains("page-2.html", "page 2 in the url");
    await g.waitSelectorText(
      ".page-header h1, h1",
      "Fiction",
      "still Fiction on page 2",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
