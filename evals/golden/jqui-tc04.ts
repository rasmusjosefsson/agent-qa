#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// jqueryui.com/checkboxradio + /selectmenu — synthetic widgets that
// replace native inputs: label-driven radios/checkboxes (IDL checked
// claims, group unselect) and a selectmenu rendered as a detached
// portal-style menu (nth-child item pick, text claim on the button).
await runEdgeGolden(
  "tc04", "jqueryui — checkboxradio labels + selectmenu portal pick",
  "https://jqueryui.com/checkboxradio/",
  "iframe.demo-frame",
  async (b) => {
    await b.openPage();
    await b.enterFrame("iframe.demo-frame", "into the checkboxradio demo frame");
    await b.clickSelector("label[for=radio-2]", "pick radio 2 by its label");
    await b.assertElementAttribute("#radio-2", "checked", "equals", "true", "radio-2 checked via label");
    await b.clickSelector("label[for=checkbox-nested-2]", "check nested checkbox 2");
    await b.assertElementAttribute("#checkbox-nested-2", "checked", "equals", "true", "nested-2 checked");
    await b.clickSelector("label[for=radio-1]", "pick radio 1 — group unselects radio 2");
    await b.assertElementAttribute("#radio-2", "checked", "equals", "false", "radio-2 cleared by group");
    await b.exitFrame("back to the wrapper page");
    await b.assertElementText("h1", "Checkboxradio", "wrapper page heading");

    await b.gotoUrl("https://jqueryui.com/selectmenu/", "open the selectmenu demo");
    await b.enterFrame("iframe.demo-frame", "into the selectmenu demo frame");
    await b.assertElementText("#speed-button .ui-selectmenu-text", "Medium", "selectmenu initial value");
    await b.clickSelector("#speed-button", "open the selectmenu");
    await b.waitSelector("#speed-menu .ui-menu-item-wrapper", "menu items rendered");
    await b.assertElementCount("#speed-menu .ui-menu-item", 5, "five speed options");
    // Items are generated divs without ids — pick "Fast" by position.
    await b.clickSelector("#speed-menu .ui-menu-item:nth-child(4)", "pick Fast");
    await b.waitMs(300, "selectmenu close");
    await b.assertElementText("#speed-button .ui-selectmenu-text", "Fast", "selectmenu value updated");
    await b.exitFrame("back to the wrapper page");
    await b.assertElementText("h1", "Selectmenu", "wrapper page heading");
  },
  { label: "jqui" },
);
