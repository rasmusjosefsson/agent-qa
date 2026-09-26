#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc11", "Forms TC11 country dropdown selection", async (golden) => {
  await golden.openPage();
  await golden.selectOption("#country", "IN", "select India by value");
  // `value` is a live-property read — the option is selected.
  await golden.assertElementAttribute("#country", "value", "IN", "country select value is IN");
});
