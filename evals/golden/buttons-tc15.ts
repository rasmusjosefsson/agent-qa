#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc15", "Buttons TC15 — page loads clean", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.waitSelectorVisible('[data-testid="btn-double-click"]', "all widgets rendered");
  await b.assertElementText('[data-testid="result-s01"]', "No navigation yet", "result pane initialised");
  await b.assertElementAttribute('[data-testid="btn-disabled"]', "disabled", "true", "disabled widget present");
});
