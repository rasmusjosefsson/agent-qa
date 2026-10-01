#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// selenium.dev web-form — the rarer controls: a datalist input, a text
// date field, a range slider driven by arrow keys, and a color input.
// The submit url echoes each serialized value.
await runEdgeGolden(
  "tc03", "selenium web-form — datalist, date, range, color echoes",
  "https://www.selenium.dev/selenium/web/web-form.html",
  "#my-text-id",
  async (b) => {
    await b.openPage();
    await b.assertElementCount("#my-options option", 5, "datalist offers five cities");
    await b.fillSelector("input[name=my-datalist]", "San Francisco", "fill the datalist input");
    await b.assertElementAttribute("input[name=my-datalist]", "value", "equals", "San Francisco", "datalist value set");
    // bootstrap-datepicker rewrites the input from its internal state on
    // the next outside mousedown, so a programmatic .value fill is wiped at
    // submit — the value must go in through the widget's own day cells.
    const now = new Date();
    const mm = String(now.getMonth() + 1).padStart(2, "0");
    const dd = String(now.getDate()).padStart(2, "0");
    const yyyy = String(now.getFullYear());
    const picked = `${mm}/${dd}/${yyyy}`;
    await b.clickSelector("input[name=my-date]", "open the date picker");
    await b.waitSelector(".datepicker-dropdown", "calendar opens");
    await b.clickXpath(
      `//td[contains(@class,"day") and not(contains(@class,"old")) and not(contains(@class,"new")) and normalize-space(.)="${now.getDate()}"]`,
      "pick today on the calendar",
    );
    await b.assertElementAttribute("input[name=my-date]", "value", "equals", picked, "date value set");
    await b.assertElementAttribute("input[name=my-range]", "value", "equals", "5", "range starts at 5");
    await b.pressOn("input[name=my-range]", "ArrowRight", "bump the range slider");
    await b.assertElementAttribute("input[name=my-range]", "value", "equals", "6", "range moved to 6");
    await b.assertElementAttribute("input[name=my-date]", "value", "equals", picked, "date survives blur");
    await b.assertElementAttribute("input[name=my-colors]", "value", "equals", "#563d7c", "color input keeps its default");
    await b.clickSelectorForce("button[type=submit]", "submit the form");
    await b.waitSelectorText("body", "Received!", "submit confirmation page");
    await b.assertUrlContains("my-datalist=San+Francisco", "datalist value echoed");
    await b.assertUrlContains(`my-date=${mm}%2F${dd}%2F${yyyy}`, "date value echoed");
    await b.assertUrlContains("my-range=6", "bumped range echoed");
  },
  { label: "webform" },
);
