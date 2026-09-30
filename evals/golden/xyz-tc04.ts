#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// XYZ Bank — bank manager side: add a customer, find them in the
// customers table via the live search box.
await runEdgeGolden(
  "tc04", "XYZ Bank — manager adds a customer",
  "https://www.globalsqa.com/angularJs-protractor/BankingProject/#/login", "button[ng-click*='manager']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("button[ng-click*='manager']", "choose bank manager login");
    await b.waitSelector("button[ng-click*='addCust']", "manager console rendered");
    await b.clickSelector("button[ng-click*='addCust']", "open the add-customer tab");
    await b.waitSelector("input[placeholder='First Name']", "add-customer form rendered");
    await b.fillSelector("input[placeholder='First Name']", "Devin", "enter first name");
    await b.fillSelector("input[placeholder='Last Name']", "QA-{{vars._unique}}", "enter last name");
    await b.fillSelector("input[placeholder='Post Code']", "E14", "enter post code");
    await b.waitMs(300, "let AngularJS digest settle");
    await b.clickSelectorForce("button[type='submit']", "add the customer");
    await b.clickSelector("button[ng-click*='showCust']", "open the customers tab");
    await b.waitSelector("input[placeholder='Search Customer']", "search box rendered");
    await b.fillSelector("input[placeholder='Search Customer']", "Devin", "filter customers by name");
    await b.assertElementAttribute("body", "text", "contains", "Devin", "new customer found in the table");
  },
);
