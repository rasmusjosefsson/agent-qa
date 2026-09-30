#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// XYZ Bank — transactions ledger lists the deposit just made.
await runEdgeGolden(
  "tc03", "XYZ Bank — deposit lands in the transactions table",
  "https://www.globalsqa.com/angularJs-protractor/BankingProject/#/login", "button[ng-click*='customer']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("button[ng-click*='customer']", "choose customer login");
    await b.waitSelector("#userSelect", "customer list rendered");
    await b.selectOption("#userSelect", "Harry Potter", "choose Harry Potter");
    await b.waitSelector("button[type='submit']:not(.ng-hide)", "login button un-hidden");
    await b.waitMs(400, "let AngularJS digest settle");
    await b.clickSelectorForce("button[type='submit']", "log in");
    await b.clickSelector("button[ng-click='deposit()']", "open the deposit tab");
    await b.waitSelector("input[placeholder='amount']", "deposit form rendered");
    await b.fillSelector("input[placeholder='amount']", "150", "deposit 150");
    await b.waitMs(300, "let the amount field settle");
    await b.clickSelectorForce("button[type='submit']", "submit the deposit");
    await b.waitSelectorText("body", "Deposit Successful", "deposit confirmed");
    await b.clickSelector("button[ng-click='transactions()']", "open the transactions tab");
    await b.assertUrlContains("listTx", "transactions route loaded");
    await b.waitSelectorText("body", "Credit", "deposit row rendered as credit");
    await b.assertElementAttribute("body", "text", "contains", "150", "deposit amount listed");
  },
);
