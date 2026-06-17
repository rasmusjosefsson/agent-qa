#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc03", "Dropdowns TC03 selected fruit value is displayed", async (golden) => {
  await golden.openPage();
  await golden.selectCustom("#dropdown-fruit", "Banana", "select Banana");
  await golden.assertLiveCondition(
    `(() => { const fruit = document.querySelector('#dropdown-fruit'); const result = document.querySelector('[data-testid="result-fruit"]'); if (!(fruit?.textContent || result?.textContent || '').includes('Banana')) throw new Error('Banana not displayed'); return true; })()`,
    "Banana displayed after selection",
  );
  await golden.waitSelectorText("#dropdown-fruit", "Banana", "fruit dropdown shows Banana");
});
