#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc02", "Links TC02 — link label text", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.assertElementText('[data-testid="link-internal-home"]', "Home", "label is Home");
  await b.assertElementText('[data-testid="link-internal-about"]', "About Us", "label is About Us");
});
