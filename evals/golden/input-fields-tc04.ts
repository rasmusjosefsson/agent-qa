#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc04", "Input Fields TC04 — appended text is retained", "https://qaplayground.com/practice/input-fields", '[data-testid="input-append"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="input-append"]', "value", "Avengers", "prefilled value present");
  await b.fill('[data-testid="input-append"]', "Avengers Endgame", "append text to the prefilled value");
  await b.assertElementAttribute('[data-testid="input-append"]', "value", "Avengers Endgame", "appended value retained");
});
