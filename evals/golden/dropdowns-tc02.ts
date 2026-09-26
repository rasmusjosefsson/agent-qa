#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc02", "Dropdowns TC02 select India from country dropdown by value", async (golden) => {
  await golden.openPage();
  await golden.selectNative("#countrySelect", "india", "select India");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#result-s02'); if (!el || !el.textContent.includes('India')) throw new Error('India not selected'); return true; })()`,
    "India selected in country dropdown",
  );
  await golden.waitSelectorText("#result-s02", "India", "country result shows India");
});
