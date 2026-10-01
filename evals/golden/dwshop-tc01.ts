#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// demowebshop.tricentis.com (nopCommerce) — catalog browse + ajax
// add-to-cart + search. The add is asynchronous: `.cart-qty` bumps and
// a `.bar-notification.success` banner lands without a navigation.
await runEdgeGolden(
  "tc01", "demo web shop — browse, search, ajax add-to-cart",
  "https://demowebshop.tricentis.com/",
  ".top-menu a[href='/books']",
  async (b) => {
    await b.openPage();
    await b.clickSelector(".top-menu a[href='/books']", "open the Books category");
    await b.waitSelector(".product-item", "books catalog rendered");
    await b.assertElementCount(".item-box", 6, "six books listed");

    await b.fillSelector("#small-searchterms", "book", "search for 'book'");
    await b.clickSelector(".search-box input[value='Search']", "run the search");
    await b.assertUrlContains("search", "search results page");
    await b.waitSelector(".search-results .item-box", "results rendered");
    await b.assertElementPresent(".search-results .item-box", "search returned a product");

    await b.clickSelector(".search-results .product-box-add-to-cart-button", "add first result to cart");
    await b.waitSelector(".bar-notification.success", "ajax add-to-cart banner");
    await b.assertElementText(".cart-qty", "(1)", "cart badge bumped to one item");
  },
  { label: "dwshop" },
);
