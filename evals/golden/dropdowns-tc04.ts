#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc04", "Dropdowns TC04 language options are available", async (golden) => {
  await golden.openPage();
  await golden.assertLiveCondition(
    `(() => { const texts = Array.from(document.querySelector('#languageSelect').options).map((o) => o.text.trim()); const expected = ['Python', 'Java', 'JavaScript', 'TypeScript']; if (texts.length !== expected.length || !expected.every((item) => texts.includes(item))) throw new Error('language options mismatch: ' + texts.join(',')); return true; })()`,
    "language select has all four options",
  );
  await golden.recordCustomDropdownOpen("#languageSelect", "open language dropdown");
  await golden.waitSelectorText("#languageSelect", "Python", "Python option visible");
});
