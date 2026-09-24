#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc07", "Dropdowns TC07 deselect a superhero", async (golden) => {
  await golden.openPage();
  await golden.selectNative("#heroSelect", ["ant-man", "aquaman"], "select Ant-Man and Aquaman heroes");
  await golden.selectNative("#heroSelect", "aquaman", "deselect Ant-Man and keep Aquaman");
  await golden.assertLiveCondition(
    `(() => { const selected = Array.from(document.querySelector('#heroSelect').selectedOptions).map((o) => o.text); if (selected.length !== 1 || selected[0] !== 'Aquaman') throw new Error('heroes mismatch: ' + selected.join(',')); return true; })()`,
    "only Aquaman remains selected",
  );
  await golden.waitSelector("#heroSelect", "heroes multi-select visible after deselect");
});
