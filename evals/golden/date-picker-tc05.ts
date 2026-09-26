#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("date-picker", "tc05", "Date Picker TC05 — cleared date becomes empty", "https://qaplayground.com/practice/date-picker", '[data-testid="date-picker-page"]', async (b) => {
  await b.openPage();
  await b.fill('[data-testid="dp-basic-input"]', "2026-04-10", "enter a date");
  await b.assertElementAttribute('[data-testid="dp-basic-input"]', "value", "2026-04-10", "date stored");
  await b.clearSelector('[data-testid="dp-basic-input"]', "clear the date");
  await b.assertElementAttribute('[data-testid="dp-basic-input"]', "value", "", "date empty after clear");
});
