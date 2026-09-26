#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("date-picker", "tc02", "Date Picker TC02 — birthday date stored", "https://qaplayground.com/practice/date-picker", '[data-testid="date-picker-page"]', async (b) => {
  await b.openPage();
  await b.fill('[data-testid="dp-basic-input"]', "1990-05-20", "enter birthday");
  await b.assertElementAttribute('[data-testid="dp-basic-input"]', "value", "1990-05-20", "birthday stored");
  await b.waitSelectorText('[data-testid="result-s01"]', "1990-05-20", "date echoed");
});
