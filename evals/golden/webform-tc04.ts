#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// selenium.dev web-form — form-serialization semantics: a disabled input
// never reaches the url, readonly and hidden inputs do, and a file input
// submits just the file name.
await runEdgeGolden(
  "tc04", "selenium web-form — disabled/readonly/hidden/file serialization",
  "https://www.selenium.dev/selenium/web/web-form.html",
  "#my-text-id",
  async (b) => {
    await b.openPage();
    await b.assertElementAttribute("input[name=my-disabled]", "disabled", "equals", "true", "disabled input marked");
    await b.assertElementAttribute("input[name=my-readonly]", "readonly", "equals", "true", "readonly input marked");
    await b.fillSelector("input[name=my-readonly]", "still reads", "fill the readonly input");
    await b.assertElementAttribute("input[name=my-readonly]", "value", "equals", "still reads", "readonly value set");
    await b.fillSelector("input[name=my-hidden]", "peekaboo", "fill the hidden input");
    await b.upload("input[name=my-file]", "evals/fixtures/upload-valid.txt", "attach the upload fixture");
    await b.clickSelectorForce("button[type=submit]", "submit the form");
    await b.waitSelectorText("body", "Received!", "submit confirmation page");
    await b.assertUrlContains("my-readonly=still+reads", "readonly value echoed");
    await b.assertUrlContains("my-hidden=peekaboo", "hidden value echoed");
    await b.assertUrlContains("my-file=upload-valid.txt", "file name echoed");
  },
  { label: "webform" },
);
