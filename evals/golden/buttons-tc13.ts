#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc13", "Buttons TC13 — reported size stays sane", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="btn-get-size"]', "size button in view");
  await b.clickSelector('[data-testid="btn-get-size"]', "click Do you know my size?");
  await b.assertElementAttributeMatch('[data-testid="result-s04"]', "text", "matches", "^W: \\d+px, H: \\d+px$", "size reported in px");
});
