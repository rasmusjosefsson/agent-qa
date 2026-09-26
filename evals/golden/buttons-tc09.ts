#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc09", "Buttons TC09 — keyboard activation", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="btn-get-size"]', "size button in view");
  await b.focusSelector('[data-testid="btn-get-size"]', "focus size button");
  await b.assertElementAttribute('[data-testid="btn-get-size"]', "focused", "true", "button holds focus");
  await b.pressKey("Enter", "activate via keyboard");
  await b.assertElementAttributeMatch('[data-testid="result-s04"]', "text", "matches", "^W: \\d+px, H: \\d+px$", "Enter fired the action");
});
