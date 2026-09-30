import { runEdgeGolden } from "./edge-pages-lib";

// books-tc03 — detail page product-info table: UPC + star rating class
// + availability count on a known book (A Light in the Attic is the
// catalogue's canonical first title with stable metadata).
await runEdgeGolden(
  "books-tc03",
  "book detail — UPC row, star-rating class, price + stock claims",
  "https://books.toscrape.com/catalogue/a-light-in-the-attic_1000/index.html",
  "div.product_main",
  async (g) => {
    await g.openPage();
    await g.waitSelectorText("h1", "A Light in the Attic", "book title");
    await g.waitSelectorText(
      ".product_page .table",
      "Product Type",
      "product info table renders",
    );
    await g.assertElementAttribute(
      "article.product_page > div p.star-rating, .product_main p.star-rating",
      "class",
      "matches",
      "star-rating\\s+\\w+",
      "star rating present",
    );
    await g.assertElementAttribute(
      "div.product_main p.price_color",
      "text",
      "equals",
      "£51.77",
      "exact price",
    );
    await g.waitSelectorText("p.instock", "22 available", "stock count");
    await g.assertPageError(true, "countEquals", 0, "no page errors");
  },
);
