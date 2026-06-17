#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc02", "Dropdowns TC02 select India from country dropdown", async (golden) => {
  await golden.openPage();
  await golden.selectCustom("#dropdown-country", "India", "select India");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#dropdown-country'); if (!el || !el.textContent.includes('India')) throw new Error('India not selected'); return true; })()`,
    "India selected in country dropdown",
  );
  await golden.waitSelectorText("#dropdown-country", "India", "country dropdown shows India");
});
