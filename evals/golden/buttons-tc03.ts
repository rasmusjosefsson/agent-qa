#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc03", "Buttons TC03 — click runs the bound action", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="btn-get-color"]', "color button in view");
  await b.clickSelector('[data-testid="btn-get-color"]', "click Find my color?");
  await b.assertElementAttributeMatch('[data-testid="result-s03"]', "text", "matches", "^Background: rgb\\(\\d+, \\d+, \\d+\\)$", "background color echoed");
});
