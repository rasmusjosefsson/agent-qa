#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("data-table", "tc04", "Data Table TC04 — search filters to the matching row", "https://qaplayground.com/practice/data-table", '[data-testid="data-table"]', async (b) => {
  await b.openPage();
  await b.fill('[data-testid="table-search"]', "Rowling", "filter by author");
  await b.waitSelectorText('[data-testid="row-count"]', "1 book", "one row remains");
  await b.assertElementAttributeMatch('[data-testid="table-body"]', "text", "contains", "Harry Potter", "Rowling book shown");
});
