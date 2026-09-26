#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc03", "Radio TC03 — only one group member stays selected", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.checkSelector("#radio-plan-starter", "select Starter");
  await b.checkSelector("#radio-plan-pro", "select Pro");
  await b.checkSelector("#radio-plan-business", "select Business");
  await b.assertElementAttribute("#radio-plan-starter", "checked", "false", "Starter off");
  await b.assertElementAttribute("#radio-plan-pro", "checked", "false", "Pro off");
  await b.assertElementAttribute("#radio-plan-business", "checked", "true", "Business on");
});
