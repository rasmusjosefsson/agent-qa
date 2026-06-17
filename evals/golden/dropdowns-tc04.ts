#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc04", "Dropdowns TC04 language options are available", async (golden) => {
  await golden.openPage();
  await golden.openCustomDropdownLive("#dropdown-language");
  await golden.assertLiveCondition(
    `(() => { const texts = Array.from(document.querySelectorAll('[role="option"]')).map((el) => el.textContent?.trim()).filter(Boolean); const expected = ['Python', 'Java', 'JavaScript']; if (texts.length !== 3 || !expected.every((item) => texts.includes(item))) throw new Error('language options mismatch: ' + texts.join(',')); return true; })()`,
    "language dropdown has Python Java JavaScript",
  );
  await golden.recordCustomDropdownOpen("#dropdown-language", "open language dropdown");
  await golden.waitSelectorText("body", "Python", "Python option visible");
});
