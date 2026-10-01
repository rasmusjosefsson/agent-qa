#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// blazedemo.com — navigation round-trip: promo link out to the static
// vacation page, then the "home" breadcrumb back to the index.
//
// Every blazedemo page ships broken JS — "jQuery is not defined" and
// TypeErrors fire on load. Instead of the blanket no-errors gate we
// assert the quirk exists, so a real change in the site's error surface
// (new error text, or the bug being fixed) makes the case fail.
await runEdgeGolden(
  "tc04", "blazedemo — promo link + home round-trip",
  "https://blazedemo.com/",
  "select[name='fromPort']",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("a[href='vacation.html']", "promo link rendered");

    await b.clickSelector("a[href='vacation.html']", "open destination of the week");
    await b.waitSelector("img", "vacation image rendered");
    await b.assertUrlContains("vacation.html", "landed on vacation page");
    await b.assertElementCount("img", 1, "single promo image");

    await b.clickSelector("a[href='index.php']", "follow the home link");
    await b.waitSelector("h1", "index heading rendered");
    await b.assertElementText("h1", "Welcome to the Simple Travel Agency!", "back on the index");
    await b.assertElementPresent("select[name='toPort']", "route selects rendered");

    // Documented site quirk: blazedemo throws "jQuery is not defined"
    // on every page (its own bug, not a scenario failure).
    await b.assertPageError({ text: "jQuery is not defined" }, "exists", undefined, "site's known broken JS");
  },
  { label: "blaze", flushArgs: ["--no-auto-errors"] },
);
