#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc09", "Dropdowns TC09 country dropdown enabled and interactable", async (golden) => {
  await golden.openPage();
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#countrySelect'); if (!el || el.disabled || el.getAttribute('aria-disabled') === 'true') throw new Error('country dropdown disabled'); return true; })()`,
    "country dropdown enabled",
  );
  await golden.selectNative("#countrySelect", "united-states", "select United States");
  await golden.waitSelectorText("#result-s02", "United States", "country result shows United States");
});
