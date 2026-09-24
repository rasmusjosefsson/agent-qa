#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc03", "Dropdowns TC03 selected value displayed after selection", async (golden) => {
  await golden.openPage();
  await golden.selectNative("#fruitSelect", "banana", "select Banana");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#result-s01'); if (!el || !el.textContent.includes('Banana')) throw new Error('Banana not displayed'); return true; })()`,
    "Banana displayed after selection",
  );
  await golden.waitSelectorText("#result-s01", "Banana", "fruit result shows Banana");
});
