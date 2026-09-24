#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc08", "Dropdowns TC08 fruit placeholder before selection", async (golden) => {
  await golden.openPage();
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#fruitSelect'); const first = el?.options[0]?.text?.trim(); if (!el || el.value !== '' || first !== 'Select Fruit') throw new Error('placeholder mismatch: ' + first + ' value=' + el?.value); return true; })()`,
    "fruit dropdown shows placeholder",
  );
  await golden.waitSelectorText("#fruitSelect", "Select Fruit", "fruit select shows placeholder option");
});
