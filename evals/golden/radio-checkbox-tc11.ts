#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc11", "Radio TC11 — disabled radio cannot be selected", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="radio-disabled"]', "disabled", "true", "radio disabled");
  await b.assertElementAttribute('[data-testid="radio-disabled"]', "checked", "false", "disabled radio unchecked");
});
