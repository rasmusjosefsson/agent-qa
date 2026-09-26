#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc01", "Input Fields TC01 — submit a movie name", "https://qaplayground.com/practice/input-fields", '[data-testid="input-movie-name"]', async (b) => {
  await b.openPage();
  await b.fill('[data-testid="input-movie-name"]', "Inception", "type movie name");
  await b.clickSelector('[data-testid="btn-submit-movie"]', "submit movie");
  await b.waitSelectorText('[data-testid="result-s01"]', "You entered: Inception", "echo shows submitted movie");
});
