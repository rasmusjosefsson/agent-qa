#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("data-table", "tc02", "Data Table TC02 — total row count shown", "https://qaplayground.com/practice/data-table", '[data-testid="data-table"]', async (b) => {
  await b.openPage();
  await b.assertElementAttributeMatch('[data-testid="row-count"]', "text", "contains", "25 books", "25 books cataloged");
});
