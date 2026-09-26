#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc12", "Buttons TC12 — state resets on refresh", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="btn-get-coordinates"]', "coords button in view");
  await b.clickSelector('[data-testid="btn-get-coordinates"]', "fire coordinates action");
  await b.assertElementAttributeMatch('[data-testid="result-s02"]', "text", "matches", "^X: \\d+px, Y: \\d+px$", "coordinates echoed");
  await b.reloadPage("refresh the page");
  await b.assertElementText('[data-testid="result-s02"]', "Coordinates: —", "echo reset to placeholder");
});
