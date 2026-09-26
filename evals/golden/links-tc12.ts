#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc12", "Links TC12 — page loads clean", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.waitSelectorVisible('[data-testid="link-external-course"]', "link groups rendered");
  await b.assertElementText('[data-testid="result-s01"]', "Click an internal link", "result pane initialised");
});
