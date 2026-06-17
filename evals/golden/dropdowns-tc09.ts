#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc09", "Dropdowns TC09 country dropdown is enabled and interactable", async (golden) => {
  await golden.openPage();
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#dropdown-country'); if (!el || el.disabled || el.getAttribute('aria-disabled') === 'true') throw new Error('country dropdown disabled'); return true; })()`,
    "country dropdown enabled",
  );
  await golden.selectCustom("#dropdown-country", "USA", "select USA");
  await golden.waitSelectorText("#dropdown-country", "USA", "country dropdown shows USA");
});
