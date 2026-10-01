#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// selenium.dev web-form — checkboxes, radios, and the select all serialize
// into the submit url: unchecking the default box and picking the second
// radio flips which my-check/my-radio params ship.
await runEdgeGolden(
  "tc02", "selenium web-form — checkbox, radio, select state echoes",
  "https://www.selenium.dev/selenium/web/web-form.html",
  "#my-text-id",
  async (b) => {
    await b.openPage();
    await b.assertElementAttribute("#my-check-1", "checked", "equals", "true", "first checkbox checked by default");
    await b.assertElementAttribute("#my-check-2", "checked", "equals", "false", "second checkbox unchecked");
    await b.uncheckSelector("#my-check-1", "uncheck the default box");
    await b.checkSelector("#my-check-2", "check the second box");
    await b.assertElementAttribute("#my-check-1", "checked", "equals", "false", "first box now unchecked");
    await b.assertElementAttribute("#my-check-2", "checked", "equals", "true", "second box now checked");
    await b.checkSelector("#my-radio-2", "pick the second radio");
    await b.assertElementAttribute("#my-radio-2", "checked", "equals", "true", "second radio checked");
    await b.assertElementAttribute("#my-radio-1", "checked", "equals", "false", "first radio released");
    await b.selectOption("select[name=my-select]", "Two", "pick select option Two");
    await b.assertElementAttribute("select[name=my-select]", "value", "equals", "2", "select carries value 2");
    await b.clickSelectorForce("button[type=submit]", "submit the form");
    await b.waitSelectorText("body", "Received!", "submit confirmation page");
    await b.assertUrlContains("my-select=2", "selected option value echoed");
    await b.assertUrlContains("my-radio=on", "picked radio echoed");
    await b.assertUrlContains("my-check=on", "checked box echoed");
  },
  { label: "webform" },
);
