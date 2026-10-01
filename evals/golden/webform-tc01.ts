#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// selenium.dev/selenium/web/web-form.html — the canonical every-control
// page. Submit GETs to submitted-form.html with every field serialized
// into the URL, so the url claims pin exactly what the controls carried.
await runEdgeGolden(
  "tc01", "selenium web-form — text controls submit into the url echo",
  "https://www.selenium.dev/selenium/web/web-form.html",
  "#my-text-id",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#my-text-id", "qa-sweep", "fill the text input");
    await b.fillSelector("input[name=my-password]", "s3cret!", "fill the password");
    await b.fillSelector("textarea[name=my-textarea]", "line one", "fill the textarea");
    await b.clickSelectorForce("button[type=submit]", "submit the form");
    await b.waitSelectorText("body", "Received!", "submit confirmation page");
    await b.assertUrlContains("submitted-form", "submit navigates to the echo page");
    await b.assertUrlContains("my-text=qa-sweep", "text value echoed in the url");
    await b.assertUrlContains("my-password=s3cret%21", "password echoed url-encoded");
    await b.assertUrlContains("my-textarea=line+one", "textarea echoed url-encoded");
    await b.assertElementText("h1.display-6", "Form submitted", "confirmation heading");
    await b.assertNetworkFired({ method: "GET", urlMatches: "submitted-form.html" }, true, "echo page fetched");
  },
  { label: "webform" },
);
