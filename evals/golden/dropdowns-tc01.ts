#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc01", "Dropdowns TC01 select Apple from fruit dropdown", async (golden) => {
  await golden.openPage();
  await golden.selectNative("#fruitSelect", "apple", "select Apple");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#result-s01'); if (!el || !el.textContent.includes('Apple')) throw new Error('Apple not selected'); return true; })()`,
    "Apple selected in fruit dropdown",
  );
  await golden.waitSelectorText("#result-s01", "Apple", "fruit result shows Apple");
});
