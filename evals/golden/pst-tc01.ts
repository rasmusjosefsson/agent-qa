import { runEdgeGolden } from "./edge-pages-lib";

// practicesoftwaretesting.com — a real Laravel-API store (search → product
// detail → cart → login). Exercises cross-origin network claims against
// api.practicesoftwaretesting.com plus the SPA's data-test selectors.
// The demo customer account is documented on the site's Testing Guide page.
await runEdgeGolden(
  "tc01",
  "practicesoftwaretesting — search, cart add, and demo login",
  "https://practicesoftwaretesting.com/",
  "a.card[data-test^='product-']",
  async (b) => {
    await b.openPage();
    // The store's catalog endpoints use the QUERY method (safe-method-with-
    // body draft), not GET — match on the URL only.
    await b.assertNetworkStatus(
      { urlMatches: "api\\.practicesoftwaretesting\\.com/products" },
      "equals",
      "200",
      "catalog API call returns 200",
    );

    await b.fillSelector("[data-test=search-query]", "pliers", "type a search term");
    await b.clickSelector("[data-test=search-submit]", "submit the search");
    await b.waitSelector("a.card[data-test^='product-']", "search results render");
    // The SPA filters in place — no router nav; the search hits a
    // QUERY-method endpoint instead of changing the URL.
    await b.assertNetworkFired(
      { urlMatches: "api\\.practicesoftwaretesting\\.com/products/search" },
      true,
      "search API call fired",
    );

    await b.clickSelector("a.card[data-test^='product-']", "open the first result");
    await b.waitSelector("[data-test=product-name]", "product detail renders");
    await b.assertElementPresent("[data-test=unit-price]", "unit price is shown");

    await b.clickSelector("[data-test=add-to-cart]", "add to cart");
    await b.waitSelector("[data-test=cart-quantity]", "cart badge appears");
    await b.assertElementText("[data-test=cart-quantity]", "1", "one item in the cart");
    await b.assertNetworkStatus(
      { urlMatches: "api\\.practicesoftwaretesting\\.com/carts$", method: "POST" },
      "equals",
      "201",
      "cart create returns 201",
    );

    await b.gotoUrl(
      "https://practicesoftwaretesting.com/auth/login",
      "open the login page",
    );
    await b.waitSelector("[data-test=login-form]", "login form renders");
    await b.fillSelector(
      "[data-test=email]",
      "customer@practicesoftwaretesting.com",
      "type the demo email",
    );
    await b.fillSelector("[data-test=password]", "welcome01", "type the demo password");
    await b.clickSelector("[data-test=login-submit]", "submit login");
    await b.waitSelectorText("h1", "My account", "account page renders");
    await b.assertElementPresent(
      "[data-test=nav-sign-out]",
      "sign-out entry means the session is authenticated",
    );
    await b.assertNetworkFired(
      { urlMatches: "api\\.practicesoftwaretesting\\.com/users/login", method: "POST" },
      true,
      "login API was called",
    );

    await b.assertPageError(true, "notExists", undefined, "no uncaught page errors");
  },
  { label: "pst" },
);
