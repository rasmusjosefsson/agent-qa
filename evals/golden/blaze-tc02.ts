#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// blazedemo.com — purchase form: text fills by id, card-type select,
// remember-me checkbox, submit → confirmation. The confirmation is a
// CANNED receipt — it echoes "xxxxxxxxxxxx1111", "11 /2018", "555 USD"
// regardless of submitted data; claims assert the literal echo.
// --no-auto-errors: site ships broken JS on every page (see tc04).
await runEdgeGolden(
  "tc02", "blazedemo — pick flight, purchase ticket",
  "https://blazedemo.com/",
  "select[name='fromPort']",
  async (b) => {
    await b.openPage();
    await b.selectOption("select[name='fromPort']", "San Diego", "depart San Diego");
    await b.selectOption("select[name='toPort']", "Dublin", "arrive Dublin");
    await b.clickSelector("input[value='Find Flights']", "find flights");
    await b.waitSelector("table tbody tr:nth-child(3) input[type='submit']", "results rendered");

    await b.clickSelector("table tbody tr:nth-child(3) input[type='submit']", "choose the third flight");
    await b.waitSelector("h2", "purchase heading rendered");
    await b.assertElementText("h2", "Your flight from TLV to SFO has been reserved.", "purchase heading (site echoes fixed TLV→SFO route)");
    await b.assertElementCount("input[type='text']", 9, "nine text fields on the form");

    await b.fillSelector("#inputName", "Rasmus J", "cardholder name");
    await b.fillSelector("#address", "1 Test Way", "street address");
    await b.fillSelector("#city", "Stockholm", "city");
    await b.fillSelector("#state", "ST", "state");
    await b.fillSelector("#zipCode", "11122", "zip code");
    await b.selectOption("#cardType", "amex", "card type Amex");
    await b.fillSelector("#creditCardNumber", "4111111111111111", "card number");
    await b.fillSelector("#nameOnCard", "Rasmus Josefsson", "name on card");
    await b.checkSelector("#rememberMe", "remember me");

    await b.clickSelector("input[value='Purchase Flight']", "purchase flight");
    await b.waitSelector("h1", "confirmation rendered");
    await b.assertElementText("h1", "Thank you for your purchase today!", "thank-you heading");
    await b.assertElementText("table tbody tr:nth-child(2) td:nth-child(1)", "Status", "status label");
    await b.assertElementText("table tbody tr:nth-child(2) td:nth-child(2)", "PendingCapture", "status PendingCapture");
    await b.assertElementAttribute("table tbody tr:nth-child(3) td:nth-child(2)", "text", "matches", "^\\d{3} USD$", "amount is N USD");
    await b.assertElementText("table tbody tr:nth-child(4) td:nth-child(2)", "xxxxxxxxxxxx1111", "card masked to last four (canned)");
    await b.assertElementAttribute("table tbody tr:nth-child(1) td:nth-child(2)", "text", "matches", "^\\d+$", "order id is numeric");
  },
  { label: "blaze", flushArgs: ["--no-auto-errors"] },
);
