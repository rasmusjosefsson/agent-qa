#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc10", "Radio TC10 — checkbox toggles with Space", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute("#chk-newsletter", "checked", "true", "newsletter pre-checked");
  await b.focusSelector("#chk-newsletter", "focus newsletter");
  await b.pressKey(" ", "toggle with Space");
  await b.assertElementAttribute("#chk-newsletter", "checked", "false", "newsletter unchecked via keyboard");
});
