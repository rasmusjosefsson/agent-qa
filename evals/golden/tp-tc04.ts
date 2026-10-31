import { runEdgeGolden } from "./edge-pages-lib.ts";

// testpages.eviltester.com client-side validation: onblur validators
// write error spans, and the form's onsubmit gate blocks submission.
await runEdgeGolden(
  "tp-tc04",
  "onblur validators flag bad values and the submit gate enforces them",
  "https://testpages.eviltester.com/styled/basic-javascript-validation-test.html",
  "#lteq30a",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#lteq30a", "99", "enter a value over 30");
    await b.pressKey("Tab", "blur to trigger the validator");
    // Site clears the error span after 2s — wait (do-step) rather than a
    // durable check claim so verify/replay aren't racing the timeout.
    await b.waitSelectorText("#lteq30aError", "30", "error span flags the first field");
    await b.fillSelector("#lteq30b", "99", "over-range in the number input");
    await b.pressKey("Tab", "blur the second field");
    await b.waitSelectorText("#lteq30bError", "30", "error span flags the second field");
    await b.fillSelector("#lteq30a", "10", "fix the first value");
    await b.fillSelector("#lteq30b", "20", "fix the second value");
    await b.clickSelector(
      "input[type='submit'][name='submitbutton']",
      "submit once valid",
    );
    await b.assertUrlContains(
      "javascript-validation",
      "gate passed and the form posted",
    );
  },
  { label: "tp" },
);
