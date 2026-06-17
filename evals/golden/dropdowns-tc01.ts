#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc01", "Dropdowns TC01 select Apple from fruit dropdown", async (golden) => {
  await golden.openPage();
  await golden.selectCustom("#dropdown-fruit", "Apple", "select Apple");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#dropdown-fruit'); if (!el || !el.textContent.includes('Apple')) throw new Error('Apple not selected'); return true; })()`,
    "Apple selected in fruit dropdown",
  );
  await golden.waitSelectorText("#dropdown-fruit", "Apple", "fruit dropdown shows Apple");
});
