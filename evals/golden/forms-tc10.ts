#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc10", "Forms TC10 gender radio button selection", async (golden) => {
  await golden.openPage();
  await golden.clickSelector("#gender-female", "select female");
  // `checked` is a live-property read on the radio input.
  await golden.assertElementAttribute("#gender-female", "checked", "true", "female radio checked");
  await golden.assertElementAttribute("#gender-male", "checked", "false", "male radio unchecked");
});
