#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc05", "Links TC05 — broken link points at 500", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="link-broken-same"]', "href", "https://the-internet.herokuapp.com/status_codes/500", "broken link targets status_codes/500");
});
