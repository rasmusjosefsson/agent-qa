#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("multi-select", "tc05", "Multi Select TC05 — remove a tag", "https://qaplayground.com/practice/multi-select", '[data-testid="scenario-ms-single"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="scenario-ms-tag-remove"]', "tag widget in view");
  await b.clickSelector('[data-testid="scenario-ms-tag-remove"] [data-testid="ms-tag"]:nth-child(3) button', "remove Python");
  await b.assertElementText('[data-testid="result-s06"]', "\"Python\" tag removed — 3 remaining", "removal echoed");
});
