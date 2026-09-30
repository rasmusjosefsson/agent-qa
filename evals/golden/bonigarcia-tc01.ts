// bonigarcia-tc01 — web form: fill text/password/textarea, select an option,
// toggle a checkbox + radio, submit. The form GETs to submitted-form.html
// with every field as a query param, so the URL itself asserts the payload
// was bound and sent.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc01",
  "Bonigarcia — fill and submit the web form",
  "https://bonigarcia.dev/selenium-webdriver-java/web-form.html",
  "#my-text-id",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#my-text-id", "boni-text", "fill the text input");
    await b.fillSelector("input[name=my-password]", "s3cret", "fill the password");
    await b.fillSelector("[name=my-textarea]", "line one\nline two", "fill the textarea");
    await b.selectOption("[name=my-select]", "1", "pick option One");
    await b.checkSelector("#my-check-1", "check the first checkbox");
    await b.clickSelector("#my-radio-1", "select the first radio");
    await b.clickSelector("button[type=submit]", "submit the form");
    await b.waitSelector("h1.display-6", "submitted page rendered");
    await b.assertUrlContains("submitted-form.html", "GET landed on the submit target");
    await b.assertUrlContains("my-text=boni-text", "text input travelled in the query");
    await b.assertElementText("h1.display-6", "Form submitted", "confirmation heading rendered");
    await b.assertElementText("p.lead", "Received!", "server acknowledged the submission");
  },
);
