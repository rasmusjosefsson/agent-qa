import { runEdgeGolden } from "./edge-pages-lib";

// Edge case: sortable table — clicking the Last Name header triggers
// tablesorter; the first row's last name flips to the alphabetical minimum.
await runEdgeGolden(
  "tc19",
  "clicking the Last Name header sorts table1 ascending",
  "/tables",
  "#table1",
  async (b) => {
    await b.openPage();
    await b.assertElementText("#table1 tbody tr:first-child td:first-child", "Smith", "default first row is Smith");
    await b.clickSelector("#table1 th:has(span)", "sort by Last Name");
    await b.assertElementText("#table1 tbody tr:first-child td:first-child", "Bach", "ascending sort puts Bach first");
  },
);
