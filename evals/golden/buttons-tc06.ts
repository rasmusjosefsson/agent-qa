#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc06", "Buttons TC06 — disabled button cannot fire", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="btn-disabled"]', "disabled", "true", "disabled property set");
  await b.assertElementText('[data-testid="result-s05"]', "Button is disabled — no action fires", "page notes the inert state");
});
