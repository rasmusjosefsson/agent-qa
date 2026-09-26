#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc09", "Radio TC09 — arrow keys move selection inside the group", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.focusSelector("#radio-plan-starter", "focus Starter");
  await b.checkSelector("#radio-plan-starter", "select Starter");
  await b.pressKey("ArrowDown", "arrow to next option");
  await b.assertElementAttribute("#radio-plan-pro", "checked", "true", "Pro selected by keyboard");
  await b.assertElementAttribute("#radio-plan-starter", "checked", "false", "Starter deselected");
});
