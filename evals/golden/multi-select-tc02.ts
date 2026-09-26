#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("multi-select", "tc02", "Multi Select TC02 — deselect one option", "https://qaplayground.com/practice/multi-select", '[data-testid="scenario-ms-single"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="scenario-ms-deselect"]', "deselect widget in view");
  await b.clickSelector('[data-testid="ms-deselect-trigger"]', "pre-select every option");
  await b.selectOption('[data-testid="scenario-ms-deselect"] [data-testid="ms-native-select"]', ["playwright", "cypress", "webdriverio"], "drop selenium");
  await b.assertElementText('[data-testid="result-s03"]', "Remaining selected: Playwright, Cypress, WebdriverIO", "deselection echoed");
});
