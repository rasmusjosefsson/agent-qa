#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// XYZ Bank — withdraw flow: deposit first (balance resets per session),
// then withdraw a smaller amount and read the success banner.
await runEdgeGolden(
  "tc02", "XYZ Bank — withdraw after a deposit",
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
    await b.clickSelector("button[ng-click='withdrawl()']", "open the withdraw tab");
    await b.waitSelector("input[placeholder='amount']", "withdraw form rendered");
    await b.fillSelector("input[placeholder='amount']", "40", "withdraw 40");
    await b.waitMs(300, "let the amount field settle");
    await b.clickSelectorForce("button[type='submit']", "submit the withdrawal");
    await b.waitSelectorText("body", "Transaction successful", "withdrawal confirmed");
    await b.assertElementAttribute("body", "text", "contains", "Balance : 60", "balance reflects the withdrawal");
  },
);
