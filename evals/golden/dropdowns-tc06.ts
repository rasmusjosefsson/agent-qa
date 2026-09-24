#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc06", "Dropdowns TC06 multi-select heroes", async (golden) => {
  await golden.openPage();
  await golden.selectNative("#heroSelect", ["ant-man", "batman"], "select Ant-Man and Batman heroes");
  await golden.assertLiveCondition(
    `(() => { const selected = Array.from(document.querySelector('#heroSelect').selectedOptions).map((o) => o.text); if (selected.length !== 2 || !selected.includes('Ant-Man') || !selected.includes('Batman')) throw new Error('heroes mismatch: ' + selected.join(',')); return true; })()`,
    "Ant-Man and Batman selected",
  );
  await golden.waitSelector("#heroSelect", "heroes multi-select visible");
});
