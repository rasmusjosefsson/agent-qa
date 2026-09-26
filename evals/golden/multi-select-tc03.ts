#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("multi-select", "tc03", "Multi Select TC03 — Select All button", "https://qaplayground.com/practice/multi-select", '[data-testid="scenario-ms-single"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="scenario-ms-select-all"]', "select-all widget in view");
  await b.clickSelector('[data-testid="scenario-ms-select-all"] [data-testid="ms-custom-trigger"]', "open the panel");
  await b.waitSelectorVisible('[data-testid="ms-select-all-btn"]', "bulk actions visible");
  await b.clickSelector('[data-testid="ms-select-all-btn"]', "Select All");
  await b.assertElementText('[data-testid="result-s05"]', "All selected", "all options echoed");
});
