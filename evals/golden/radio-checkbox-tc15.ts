#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc15", "Radio TC15 — radio and checkbox widgets load", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.waitSelectorVisible("#radio-plan-starter", "Starter radio rendered");
  await b.waitSelectorVisible("#chk-accept-terms", "terms checkbox rendered");
  await b.waitSelectorVisible("#skill-playwright", "skills checkbox rendered");
  await b.waitSelectorVisible('[data-testid="chk-disabled"]', "disabled checkbox rendered");
});
