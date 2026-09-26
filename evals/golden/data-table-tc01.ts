#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("data-table", "tc01", "Data Table TC01 — all column headers present", "https://qaplayground.com/practice/data-table", '[data-testid="data-table"]', async (b) => {
  await b.openPage();
  await b.assertElementAttributeMatch('[data-testid="col-book-name"]', "text", "contains", "Book Name", "name header");
  await b.assertElementAttributeMatch('[data-testid="col-book-genre"]', "text", "contains", "Genre", "genre header");
  await b.assertElementAttributeMatch('[data-testid="col-book-author"]', "text", "contains", "Author", "author header");
  await b.assertElementAttributeMatch('[data-testid="col-book-isbn"]', "text", "contains", "ISBN", "isbn header");
});
