#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc02", "Input Fields TC02 — placeholder attribute and typed value", "https://qaplayground.com/practice/input-fields", '[data-testid="input-movie-name"]', async (b) => {
  await b.openPage();
  await b.assertElementAttributeMatch('[data-testid="input-movie-name"]', "placeholder", "contains", "movie name", "placeholder visible before typing");
  await b.fill('[data-testid="input-movie-name"]', "Interstellar", "type over placeholder");
  await b.assertElementAttribute('[data-testid="input-movie-name"]', "value", "Interstellar", "typed value retained");
});
