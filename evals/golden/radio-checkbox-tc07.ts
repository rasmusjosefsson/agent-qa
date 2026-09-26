#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc07", "Radio TC07 — checkbox can be unchecked", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.checkSelector("#chk-accept-terms", "check terms");
  await b.clickSelector("#chk-accept-terms", "uncheck terms");
  await b.assertElementAttribute("#chk-accept-terms", "checked", "false", "terms unchecked");
});
