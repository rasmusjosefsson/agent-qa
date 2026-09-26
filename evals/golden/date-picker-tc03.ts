#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("date-picker", "tc03", "Date Picker TC03 — fill a date range", "https://qaplayground.com/practice/date-picker", '[data-testid="date-picker-page"]', async (b) => {
  await b.openPage();
  await b.fill('[data-testid="dp-range-start"]', "2026-03-01", "range start");
  await b.fill('[data-testid="dp-range-end"]', "2026-03-15", "range end");
  await b.assertElementAttribute('[data-testid="dp-range-start"]', "value", "2026-03-01", "start stored");
  await b.assertElementAttribute('[data-testid="dp-range-end"]', "value", "2026-03-15", "end stored");
});
