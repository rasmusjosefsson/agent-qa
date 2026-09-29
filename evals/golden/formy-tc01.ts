import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: formy-project.herokuapp.com — the complete web form:
// text inputs + radio + checkbox + datepicker + submit → thanks page.
const BASE = "https://formy-project.herokuapp.com";

await runEdgeGolden(
  "formy-tc01",
  "complete web form submits to the thanks page",
  `${BASE}/form`,
  "#first-name",
  async (b) => {
    await b.openPage();
    await b.fillSelector("#first-name", "Ada", "first name");
    await b.fillSelector("#last-name", "Lovelace", "last name");
    await b.fillSelector("#job-title", "Engineer", "job title");
    await b.clickSelector("#radio-button-2", "pick college education");
    await b.clickSelector("#checkbox-1", "pick male gender");
    await b.fillSelector("#datepicker", "09/29/2026", "pick a date");
    // The jQuery-UI popup stays open over Submit — Enter confirms+closes it.
    await b.pressKey("Enter", "confirm the date and close the popup");
    // Submit sits below the fold — a bare click reports Done but lands
    // on the navbar area; bring it into view first.
    await b.scrollToSelector("a[role='button']", "scroll submit into view");
    await b.clickSelector("a[role='button']", "submit the form");
    await b.waitSelector(".alert", "thanks banner rendered");
    await b.assertUrlContains("/thanks", "landed on the thanks page");
    await b.assertElementAttribute(
      ".alert",
      "text",
      "contains",
      "successfully submitted",
      "thanks message shown",
    );
  },
  { label: "formy" },
);
