#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("data-table", "tc03", "Data Table TC03 — read a cell from a specific row", "https://qaplayground.com/practice/data-table", '[data-testid="data-table"]', async (b) => {
  await b.openPage();
  await b.assertElementText('[data-testid="table-body"] tr:first-child td:nth-child(2)', "The Pragmatic Programmer", "first row book name");
});
