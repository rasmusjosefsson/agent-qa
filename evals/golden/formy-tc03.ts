import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: formy-project.herokuapp.com — jQuery-UI datepicker popup:
// click the input → day grid → pick a day → input's live value set.
const BASE = "https://formy-project.herokuapp.com";

await runEdgeGolden(
  "formy-tc03",
  "datepicker popup writes the chosen day into the input",
  `${BASE}/datepicker`,
  "#datepicker",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#datepicker", "open the datepicker");
    await b.waitSelector(
      ".datepicker-days, .datepicker.datepicker-dropdown",
      "day grid popped",
    );
    await b.clickSelector(
      ".datepicker-days td.day:not(.old):not(.new)",
      "pick the first in-month day",
    );
    await b.assertElementAttribute(
      "#datepicker",
      "value",
      "matches",
      "\\d{2}/\\d{2}/\\d{4}",
      "input carries the picked date",
    );
  },
  { label: "formy" },
);
