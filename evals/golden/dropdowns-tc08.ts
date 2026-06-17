#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc08", "Dropdowns TC08 fruit placeholder is Select Fruit", async (golden) => {
  await golden.openPage();
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#dropdown-fruit'); if (!el || el.textContent.trim() !== 'Select Fruit') throw new Error('placeholder mismatch: ' + el?.textContent); return true; })()`,
    "fruit placeholder is Select Fruit",
  );
  await golden.waitSelectorText("#dropdown-fruit", "Select Fruit", "fruit dropdown shows placeholder");
});
