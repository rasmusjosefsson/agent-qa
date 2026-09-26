#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc11", "Input Fields TC11 — readonly input does not accept typing", "https://qaplayground.com/practice/input-fields", '[data-testid="input-readonly"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="input-readonly"]', "readOnly", "true", "readonly property true");
  await b.assertElementAttribute('[data-testid="input-readonly"]', "value", "Read-only content", "readonly value readable");
});
