#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc10", "Dropdowns TC10 country dropdown option count", async (golden) => {
  await golden.openPage();
  await golden.assertLiveCondition(
    `(() => { const count = document.querySelector('#countrySelect').options.length; if (count !== 5) throw new Error('option count mismatch: ' + count); return true; })()`,
    "country dropdown has 5 options",
  );
  await golden.recordCustomDropdownOpen("#countrySelect", "open country dropdown");
  await golden.waitSelectorText("body", "Argentina", "Argentina option visible");
});
