#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc04", "Links TC04 — internal link same tab", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="link-internal-about"]', "target", "", "no target attribute");
});
