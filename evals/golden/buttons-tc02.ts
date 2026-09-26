#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc02", "Buttons TC02 — button label text", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.assertElementText('[data-testid="btn-navigate-home"]', "Go To Home", "label is Go To Home");
});
