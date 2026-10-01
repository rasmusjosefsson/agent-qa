#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// blazedemo.com — two <select>s drive a classic server-rendered flight
// search: static fixture table, row-level cell claims (fixture flights
// are identical for every route pair).
//
// Cell claims use nth-of-type: the markup is malformed (<tr><form></form>
// <td>…) so nth-child counts the empty form and lands one cell early.
// --no-auto-errors: every page throws "jQuery is not defined" etc.
// (site ships broken JS); the quirk is asserted positively in tc04.
await runEdgeGolden(
  "tc01", "blazedemo — choose route, search flights",
  "https://blazedemo.com/",
  "select[name='fromPort']",
  async (b) => {
    await b.openPage();
    await b.assertElementText("h1", "Welcome to the Simple Travel Agency!", "welcome heading");
    await b.assertElementCount("select[name='fromPort'] option", 7, "seven departure cities");
    await b.assertElementCount("select[name='toPort'] option", 7, "seven destination cities");

    await b.selectOption("select[name='fromPort']", "Boston", "depart Boston");
    await b.selectOption("select[name='toPort']", "London", "arrive London");
    await b.clickSelector("input[value='Find Flights']", "find flights");
    await b.waitSelector("h3", "results heading rendered");
    await b.assertElementText("h3", "Flights from Boston to London:", "route echoed in heading");
    await b.assertElementCount("table tbody tr", 5, "five fixture flights listed");

    await b.assertElementText("table tbody tr:nth-child(1) td:nth-of-type(2)", "43", "row 1 flight number");
    await b.assertElementText("table tbody tr:nth-child(1) td:nth-of-type(3)", "Virgin America", "row 1 airline");
    await b.assertElementText("table tbody tr:nth-child(1) td:nth-of-type(6)", "$472.56", "row 1 price");
    await b.assertElementText("table tbody tr:nth-child(3) td:nth-of-type(2)", "9696", "row 3 flight number");
    await b.assertElementText("table tbody tr:nth-child(3) td:nth-of-type(3)", "Aer Lingus", "row 3 airline");
  },
  { label: "blaze", flushArgs: ["--no-auto-errors"] },
);
