#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc05", "Dropdowns TC05 select JavaScript as last language", async (golden) => {
  await golden.openPage();
  await golden.selectCustom("#dropdown-language", "JavaScript", "select JavaScript");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#dropdown-language'); if (!el || !el.textContent.includes('JavaScript')) throw new Error('JavaScript not selected'); return true; })()`,
    "JavaScript selected in language dropdown",
  );
  await golden.waitSelectorText("#dropdown-language", "JavaScript", "language dropdown shows JavaScript");
});
