#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("data-table", "tc06", "Data Table TC06 — ISBN column holds string values", "https://qaplayground.com/practice/data-table", '[data-testid="data-table"]', async (b) => {
  await b.openPage();
  await b.assertElementAttributeMatch('[data-testid="table-body"] tr:first-child td:nth-child(5)', "text", "matches", "^ISBN-[0-9]+$", "isbn is a labeled digit string");
});
