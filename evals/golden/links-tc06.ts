#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc06", "Links TC06 — keyboard activation", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="link-internal-about"]', "about link in view");
  await b.focusSelector('[data-testid="link-internal-about"]', "focus About Us");
  await b.assertElementAttribute('[data-testid="link-internal-about"]', "focused", "true", "link holds focus");
  await b.pressKey("Enter", "activate via keyboard");
  await b.assertUrlContains("/about-us", "navigated to /about-us");
});
