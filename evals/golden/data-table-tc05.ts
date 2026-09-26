#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("data-table", "tc05", "Data Table TC05 — table is not empty after load", "https://qaplayground.com/practice/data-table", '[data-testid="data-table"]', async (b) => {
  await b.openPage();
  await b.waitSelectorVisible('[data-testid="book-row"]', "at least one row rendered");
});
