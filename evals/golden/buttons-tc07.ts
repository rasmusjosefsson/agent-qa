#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc07", "Buttons TC07 — enabled button is usable", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="btn-get-coordinates"]', "disabled", "false", "coordinates button enabled");
  await b.scrollToSelector('[data-testid="btn-get-coordinates"]', "coords button in view");
  await b.clickSelector('[data-testid="btn-get-coordinates"]', "enabled button accepts click");
  await b.assertElementAttributeMatch('[data-testid="result-s02"]', "text", "matches", "^X: \\d+px, Y: \\d+px$", "action fired");
});
