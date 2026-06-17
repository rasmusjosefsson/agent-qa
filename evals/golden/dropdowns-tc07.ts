#!/usr/bin/env bun
import { runDropdownsGolden } from "./dropdowns-lib.ts";

await runDropdownsGolden("tc07", "Dropdowns TC07 deselect Ant-Man and keep Aquaman", async (golden) => {
  await golden.openPage();
  await golden.selectNative("#dropdown-heroes", ["ant-man", "aquaman"], "select Ant-Man and Aquaman heroes");
  await golden.selectNative("#dropdown-heroes", "aquaman", "deselect Ant-Man and keep Aquaman");
  await golden.assertLiveCondition(
    `(() => { const selected = Array.from(document.querySelector('#dropdown-heroes').selectedOptions).map((o) => o.text); if (selected.length !== 1 || selected[0] !== 'Aquaman') throw new Error('heroes mismatch: ' + selected.join(',')); return true; })()`,
    "only Aquaman selected",
  );
  await golden.waitSelector("#dropdown-heroes", "heroes multi-select visible after deselect");
});
