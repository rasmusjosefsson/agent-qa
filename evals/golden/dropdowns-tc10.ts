#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc10", "Dropdowns TC10 country dropdown has four options", async (golden) => {
  await golden.openPage();
  await golden.openCustomDropdownLive("#dropdown-country");
  await golden.assertLiveCondition(
    `(() => { const texts = Array.from(document.querySelectorAll('[role="option"]')).map((el) => el.textContent?.trim()).filter(Boolean); const expected = ['India', 'USA', 'UK', 'Argentina']; if (texts.length !== 4 || !expected.every((item) => texts.includes(item))) throw new Error('country options mismatch: ' + texts.join(',')); return true; })()`,
    "country dropdown has India USA UK Argentina",
  );
  await golden.recordCustomDropdownOpen("#dropdown-country", "open country dropdown");
  await golden.waitSelectorText("body", "Argentina", "Argentina option visible");
});
