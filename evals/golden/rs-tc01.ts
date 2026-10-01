#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// rahulshettyacademy.com/AutomationPractice — the classic practice page:
// radios, a jQuery UI suggestion autocomplete, a select dropdown,
// checkboxes, and a course table. All state claims read live IDL props.
await runEdgeGolden(
  "tc01", "rahulshetty practice — radios, autocomplete, select, checkboxes, table",
  "https://rahulshettyacademy.com/AutomationPractice/",
  "#autocomplete",
  async (b) => {
    await b.openPage();
    // Radios: only the picked one holds state.
    await b.checkSelector("input[value=radio2]", "pick radio2");
    await b.assertElementAttribute("input[value=radio2]", "checked", "equals", "true", "radio2 checked");
    await b.assertElementAttribute("input[value=radio1]", "checked", "equals", "false", "radio1 stays off");
    // jQuery UI suggestion autocomplete: menu items have no stable css
    // text handle — pick through xpath like the calendar cells did.
    await b.fillSelector("#autocomplete", "ind", "type into the country autocomplete");
    await b.waitSelector(".ui-menu-item", "suggestions open");
    await b.clickXpath(
      '//li[contains(@class,"ui-menu-item") and normalize-space(.)="India"]',
      "pick India from the suggestions",
    );
    await b.assertElementAttribute("#autocomplete", "value", "equals", "India", "autocomplete holds the pick");
    // Native select.
    await b.selectOption("#dropdown-class-example", "Option2", "pick dropdown Option2");
    await b.assertElementAttribute("#dropdown-class-example", "value", "equals", "option2", "select carries option2");
    // Checkboxes check and uncheck independently.
    await b.checkSelector("#checkBoxOption1", "check option1");
    await b.checkSelector("#checkBoxOption3", "check option3");
    await b.uncheckSelector("#checkBoxOption1", "uncheck option1 again");
    await b.assertElementAttribute("#checkBoxOption1", "checked", "equals", "false", "option1 back off");
    await b.assertElementAttribute("#checkBoxOption3", "checked", "equals", "true", "option3 still on");
    // Course table: row count + first price cell. The page reuses
    // id="product" on both tables — scope to the .table-display one;
    // its header tr lives inside tbody, so :has(td) picks data rows.
    await b.assertElementCount("table.table-display tbody tr:has(td)", 10, "course table holds ten data rows");
    await b.assertElementAttribute("table.table-display tbody tr:has(td) td:last-child", "text", "equals", "30", "first course priced 30");
  },
  { label: "rs" },
);
