#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// letcode.in /forms — a 22-field registration form: text inputs, two
// id-less <select>s (country code + country), a date input, gender radios,
// a required checkbox, and a submit that reloads the page in place.
// Field values are asserted BEFORE submit since the post is a reload.
await runEdgeGolden(
  "tc01",
  "letcode forms — full registration form, value claims pre-submit",
  "https://letcode.in/forms",
  "#firstname",
  async (b) => {
    await b.openPage();
    // letcode mounts Google's fundingchoices consent wall lazily — dismiss
    // it so a late-mounting overlay can't intercept a later click.
    await b.dismissBySelector(".fc-monetization-dialog-container, .fc-dialog-overlay, .fc-message-root", "dismiss the consent wall if mounted");
    await b.fillSelector("#firstname", "Ada", "enter the first name");
    await b.fillSelector("#lasttname", "Lovelace", "enter the last name");
    await b.fillSelector("#email", "ada@example.com", "enter the email");
    await b.fillSelector("#Phno", "5551234567", "enter the phone number");
    await b.fillSelector("#Addl1", "12 Analytical Way", "enter address line 1");
    await b.fillSelector("#Addl2", "Suite 5", "enter address line 2");
    await b.fillSelector("#state", "London", "enter the state");
    await b.fillSelector("#postalcode", "N1 9GU", "enter the postal code");
    // Neither select has an id — the labelled ids on their wrappers are the anchor.
    await b.selectOption("#countrycode + * select", "44", "choose the UK country code");
    await b.selectOption("#country + * select", "India", "choose the country");
    await b.fillSelector("#Date", "1990-12-10", "enter the date of birth");
    await b.clickSelector("#female", "choose the female gender radio");
    await b.checkSelector("input[type=checkbox]", "agree to the terms");
    await b.assertElementAttribute("#firstname", "value", "equals", "Ada", "first name kept its value");
    await b.assertElementAttribute("#email", "value", "equals", "ada@example.com", "email kept its value");
    await b.assertElementAttribute("#country + * select", "value", "equals", "India", "country select kept its value");
    await b.assertElementAttribute("#Date", "value", "equals", "1990-12-10", "date kept its value");
    await b.assertElementAttribute("#female", "checked", "equals", "true", "female radio is checked");
    await b.clickSelector("input[type=submit]", "submit the registration form");
    await b.assertUrlContains("/forms", "submit stays on the same page (reload)");
    await b.assertPageError(true, "notExists", undefined, "page raised no uncaught exceptions");
  },
);
