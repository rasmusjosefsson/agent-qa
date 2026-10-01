#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// blazedemo.com — same purchase path with explicit month/year + Diners
// Club card — exercising the remaining form fields. The confirmation
// ignores submitted values entirely (canned "11 /2018" / xxxx1111),
// so these claims assert the site's fixed echo, not the inputs.
// --no-auto-errors: site ships broken JS on every page (see tc04).
await runEdgeGolden(
  "tc03", "blazedemo — explicit expiry + card echo",
  "https://blazedemo.com/",
  "select[name='fromPort']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("input[value='Find Flights']", "find flights (default route)");
    await b.waitSelector("table tbody tr:nth-child(5) input[type='submit']", "results rendered");

    await b.clickSelector("table tbody tr:nth-child(5) input[type='submit']", "choose the last flight");
    await b.waitSelector("#inputName", "purchase form rendered");

    await b.fillSelector("#inputName", "Marta Test", "cardholder name");
    await b.fillSelector("#address", "7 Demo Road", "street address");
    await b.fillSelector("#city", "Lisbon", "city");
    await b.fillSelector("#state", "LS", "state");
    await b.fillSelector("#zipCode", "1100", "zip code");
    await b.selectOption("#cardType", "dinersclub", "card type Diners Club");
    await b.fillSelector("#creditCardNumber", "5555555555554444", "card number");
    await b.fillSelector("#creditCardMonth", "3", "expiry month");
    await b.fillSelector("#creditCardYear", "2030", "expiry year");
    await b.fillSelector("#nameOnCard", "Marta Test", "name on card");

    await b.clickSelector("input[value='Purchase Flight']", "purchase flight");
    await b.waitSelector("h1", "confirmation rendered");
    await b.assertElementText("h1", "Thank you for your purchase today!", "thank-you heading");
    await b.assertElementText("table tbody tr:nth-child(4) td:nth-child(2)", "xxxxxxxxxxxx1111", "card echo is canned, not our input");
    await b.assertElementText("table tbody tr:nth-child(5) td:nth-child(2)", "11 /2018", "expiration echo is canned, not our input");
    await b.assertElementText("table tbody tr:nth-child(6) td:nth-child(2)", "888888", "auth code");
    await b.assertElementAttribute("table tbody tr:nth-child(7) td:nth-child(2)", "text", "matches", "\\d{4}", "date line carries a year");
  },
  { label: "blaze", flushArgs: ["--no-auto-errors"] },
);
