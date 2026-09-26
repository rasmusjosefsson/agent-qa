#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc01", "Links TC01 — internal link navigates", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="link-internal-about"]', "about link in view");
  await b.clickSelector('[data-testid="link-internal-about"]', "click About Us");
  await b.assertUrlContains("/about-us", "navigated to /about-us");
});
