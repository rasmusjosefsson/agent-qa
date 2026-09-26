#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc11", "Links TC11 — anchor link resolves fragment", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="link-text-anchor"]', "anchor link in view");
  await b.clickSelector('[data-testid="link-text-anchor"]', "click anchor link");
  await b.assertUrlContains("#anchor-target", "fragment appended");
});
