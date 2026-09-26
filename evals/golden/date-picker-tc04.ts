#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("date-picker", "tc04", "Date Picker TC04 — out-of-range date rejected", "https://qaplayground.com/practice/date-picker", '[data-testid="date-picker-page"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="dp-constrained-input"]', "min", "2025-06-01", "min constraint");
  await b.assertElementAttribute('[data-testid="dp-constrained-input"]', "max", "2025-12-31", "max constraint");
  await b.fill('[data-testid="dp-constrained-input"]', "2026-06-01", "enter out-of-range date");
  await b.waitSelectorText('[data-testid="result-s05"]', "out of range", "constraint violation surfaced");
});
