#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc05", "Dropdowns TC05 select the last programming language option", async (golden) => {
  await golden.openPage();
  await golden.selectNative("#languageSelect", "typescript", "select the last language option");
  await golden.assertLiveCondition(
    `(() => { const el = document.querySelector('#result-s03'); if (!el || !el.textContent.includes('TypeScript')) throw new Error('TypeScript not selected'); return true; })()`,
    "TypeScript selected in language dropdown",
  );
  await golden.waitSelectorText("#result-s03", "TypeScript", "language result shows TypeScript");
});
