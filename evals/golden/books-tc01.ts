import { runEdgeGolden } from "./edge-pages-lib";

// books-tc01 — the books.toscrape.com catalogue: first page lists 20 titles,
// a book link lands on its detail page with title, price, and stock count.
// Static catalog site — deterministic content, real pagination structure.
await runEdgeGolden(
  "books-tc01",
  "catalog page 1 → book detail — title, price, availability",
  "https://books.toscrape.com/",
  "article.product_pod",
  async (g) => {
    await g.openPage();
    await g.waitSelectorText("body", "Travel", "fiction categories render");
    await g.clickSelector(
      "article.product_pod h3 a",
      "open the first book",
    );
    await g.waitSelector("div.product_main", "detail panel renders");
    await g.waitSelectorText("h1", "A Light in the Attic", "book title");
    await g.assertElementAttribute(
      "div.product_main p.price_color",
      "text",
      "matches",
      "£\\d+\\.\\d+",
      "price shown",
    );
    await g.waitSelectorText(
      "p.instock",
      "In stock",
      "stock banner visible",
    );
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
