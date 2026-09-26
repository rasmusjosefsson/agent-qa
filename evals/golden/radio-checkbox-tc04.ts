#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc04", "Radio TC04 — option label text is correct", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.assertElementText('label[for="radio-plan-starter"]', "Starter", "Starter label");
  await b.assertElementText('label[for="radio-plan-pro"]', "Pro", "Pro label");
  await b.assertElementText('label[for="radio-plan-business"]', "Business", "Business label");
});
