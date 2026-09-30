#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// testautomationpractice.blogspot.com — classic blogspot practice page:
// free-text fields, radios, checkboxes, country <select>, multi-select.
await runEdgeGolden(
  "tc01", "TAP — form fields round-trip",
  "https://testautomationpractice.blogspot.com/", "#name",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#name", "Golden User", "fill the name field");
    await b.fillSelector("#email", "golden@example.com", "fill the email field");
    await b.fillSelector("#phone", "555-0134", "fill the phone field");
    await b.fillSelector("#textarea", "Golden comment body", "fill the address textarea");
    await b.assertElementAttribute("#name", "value", "equals", "Golden User", "name field kept its value");
    await b.checkSelector("#male", "check the male radio");
    await b.assertElementAttribute("#male", "checked", "equals", "true", "male radio stays checked");
    await b.checkSelector("#sunday", "check Sunday");
    await b.checkSelector("#monday", "check Monday");
    await b.selectOption("#country", "India", "pick India in the country select");
    await b.assertElementAttribute("#country", "value", "equals", "india", "country select kept the pick");
    await b.selectOption("#colors", "Blue", "pick Blue in the colors list");
    await b.assertElementAttribute("#colors", "value", "equals", "blue", "colors list kept the pick");
    await b.selectOption("#animals", "Fox", "pick Fox in the animals list");
    await b.assertElementAttribute("#animals", "value", "equals", "fox", "animals list kept the pick");
  },
  { label: "tap" },
);
