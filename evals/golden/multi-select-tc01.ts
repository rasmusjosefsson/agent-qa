#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("multi-select", "tc01", "Multi Select TC01 — pick several options", "https://qaplayground.com/practice/multi-select", '[data-testid="scenario-ms-single"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="scenario-ms-multi"]', "multi widget in view");
  await b.selectOption('[data-testid="scenario-ms-multi"] [data-testid="ms-native-select"]', ["playwright", "selenium"], "select two frameworks");
  await b.assertElementText('[data-testid="result-s02"]', "Playwright, Selenium selected", "both selections echoed");
});
