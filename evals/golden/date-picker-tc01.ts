#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

const today = new Date().toISOString().slice(0, 10);
runPracticeGolden("date-picker", "tc01", "Date Picker TC01 — fill today's date", "https://qaplayground.com/practice/date-picker", '[data-testid="date-picker-page"]', async (b) => {
  await b.openPage();
  await b.fill('[data-testid="dp-basic-input"]', today, "enter today's date");
  await b.assertElementAttribute('[data-testid="dp-basic-input"]', "value", today, "date value stored");
});
