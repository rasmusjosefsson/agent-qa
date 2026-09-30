#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// globalsqa XYZ Bank (AngularJS) — customer login, deposit, balance.
// The customer select and the submit take a couple of AngularJS digests
// to wire up: wait for the select itself, pick the seeded customer, then
// wait for the Login button to un-hide before clicking it.
await runEdgeGolden(
  "tc01", "XYZ Bank — customer login and deposit",
  "https://www.globalsqa.com/angularJs-protractor/BankingProject/#/login", "button[ng-click*='customer']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("button[ng-click*='customer']", "choose customer login");
    await b.waitSelector("#userSelect", "customer list rendered");
    await b.selectOption("#userSelect", "Harry Potter", "choose Harry Potter");
    await b.waitSelector("button[type='submit']:not(.ng-hide)", "login button un-hidden");
    await b.waitMs(400, "let AngularJS digest settle");
    await b.clickSelectorForce("button[type='submit']", "log in");
    await b.assertElementAttribute("body", "text", "contains", "Welcome Harry Potter", "greeting on the account page");
    await b.clickSelector("button[ng-click='deposit()']", "open the deposit tab");
    await b.waitSelector("input[placeholder='amount']", "deposit form rendered");
    await b.fillSelector("input[placeholder='amount']", "100", "deposit 100");
    await b.waitMs(300, "let the amount field settle");
    await b.clickSelectorForce("button[type='submit']", "submit the deposit");
    await b.waitSelectorText("body", "Deposit Successful", "deposit confirmed");
    await b.assertElementAttribute("body", "text", "contains", "Balance : 100", "balance reflects the deposit");
  },
);
