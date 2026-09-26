#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc12", "Radio TC12 — disabled checkbox cannot be toggled", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="chk-disabled"]', "disabled", "true", "checkbox disabled");
  await b.assertElementAttribute('[data-testid="chk-disabled"]', "checked", "false", "disabled checkbox unchecked");
});
