#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc07", "Links TC07 — href attribute value", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="link-internal-about"]', "href", "/about-us", "href is /about-us");
});
