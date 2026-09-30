import { runEdgeGolden } from "./edge-pages-lib.ts";

// testpages.eviltester.com HTML form: real POST that echoes every
// submitted field on the result page.
await runEdgeGolden(
  "tp-tc02",
  "html form POST echoes the submitted field values",
  "https://testpages.eviltester.com/styled/basic-html-form-test.html",
  "input[name='username']",
  async (b) => {
    await b.openPage();
    await b.fillSelector(
      "input[name='username']",
      "devin-tp",
      "type a username",
    );
    await b.selectOption(
      "select[name='dropdown']",
      "dd2",
      "pick dropdown item 2",
    );
    await b.checkSelector(
      "input[type='checkbox'][name='checkboxes[]']",
      "tick the first checkbox",
    );
    await b.clickSelector(
      "input[type='submit'][name='submitbutton']",
      "submit the form",
    );
    await b.assertUrlContains(
      "/html-form/submit",
      "landed on the echo endpoint",
    );
    await b.waitSelector("#_valueusername", "echo table rendered");
    await b.assertElementAttribute(
      "#_valueusername",
      "text",
      "equals",
      "devin-tp",
      "username echoed back",
    );
    await b.assertNetworkFired(
      { urlMatches: "html-form/submit", method: "POST" },
      "the form POST fired",
    );
  },
  { label: "tp" },
);
