#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc01", "Buttons TC01 — click triggers action", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="btn-get-coordinates"]', "coords button in view");
  await b.clickSelector('[data-testid="btn-get-coordinates"]', "click Find Location");
  await b.assertElementAttributeMatch('[data-testid="result-s02"]', "text", "matches", "^X: \\d+px, Y: \\d+px$", "coordinates echoed");
});
