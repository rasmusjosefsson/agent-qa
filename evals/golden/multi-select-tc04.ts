#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("multi-select", "tc04", "Multi Select TC04 — checkbox-style picks", "https://qaplayground.com/practice/multi-select", '[data-testid="scenario-ms-single"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="scenario-ms-custom"]', "custom widget in view");
  await b.clickSelector('[data-testid="scenario-ms-custom"] [data-testid="ms-custom-trigger"]', "open the panel");
  await b.clickNthOption('[data-testid="scenario-ms-custom"] [data-testid="ms-custom-panel"] > div', 1, "pick React");
  await b.clickNthOption('[data-testid="scenario-ms-custom"] [data-testid="ms-custom-panel"] > div', 3, "pick Angular");
  await b.assertElementText('[data-testid="result-s04"]', "React, Angular selected", "two options echoed");
});
