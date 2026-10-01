#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// demowebshop.tricentis.com/register — nopCommerce registration with
// `{{vars._unique}}` credentials (each replay mints its own account, so
// the flow is safe to run repeatedly against the live site). Claims the
// success message and the logged-in header state after Continue.
await runEdgeGolden(
  "tc02", "demo web shop — registration round-trip",
  "https://demowebshop.tricentis.com/register",
  "#register-button",
  async (b) => {
    await b.openPage();
    await b.checkSelector("#gender-male", "pick the Male radio");
    await b.fillSelector("#FirstName", "Rasmus", "first name");
    await b.fillSelector("#LastName", "Testsson", "last name");
    await b.fillUnique("#Email", "dw-{{vars._unique}}@qa.test", "unique email");
    await b.fillUnique("#Password", "dw-{{vars._unique}}", "unique password");
    await b.fillUnique("#ConfirmPassword", "dw-{{vars._unique}}", "confirm password");
    await b.clickSelector("#register-button", "submit registration");
    await b.waitSelector(".result", "registration result rendered");
    await b.assertElementText(".result", "Your registration completed", "registration succeeded");
    await b.clickSelector(".register-continue-button", "continue to the shop");
    await b.assertElementPresent(".header-links a.ico-logout", "logged-in header shows Log out");
    await b.assertElementAttribute(".header-links a.account", "href", "contains", "customer/info", "account link points at profile");
  },
  { label: "dwshop" },
);
