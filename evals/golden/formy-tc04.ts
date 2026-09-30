import { runEdgeGolden } from "./edge-pages-lib";

// Fresh site: formy-project.herokuapp.com — dropdown menu routes to a
// widget page; the dropdown itself is JS-only (no native <select>).
const BASE = "https://formy-project.herokuapp.com";

await runEdgeGolden(
  "formy-tc04",
  "dropdown menu routes to the picked widget page",
  `${BASE}/dropdown`,
  "#dropdownMenuButton",
  async (b) => {
    await b.openPage();
    await b.clickSelector("#dropdownMenuButton", "open the dropdown");
    await b.waitSelector(
      ".dropdown-menu.show, .dropdown-menu[style*='block']",
      "menu popped",
    );
    // Bootstrap hides the menu on the synthetic mousedown before the
    // click lands — force-click the item instead.
    await b.clickSelectorForce(
      ".dropdown-menu a[href='/datepicker']",
      "pick the datepicker item",
    );
    await b.waitSelector("#datepicker", "datepicker page rendered");
    await b.assertUrlContains("/datepicker", "landed on the widget page");
  },
  { label: "formy" },
);
