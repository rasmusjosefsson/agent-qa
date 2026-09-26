#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("buttons", "tc04", "Buttons TC04 — double-click action", "https://qaplayground.com/practice/buttons", '[data-testid="btn-navigate-home"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="btn-double-click"]', "dblclick button in view");
  await b.dblclickSelector('[data-testid="btn-double-click"]', "double-click the button");
  await b.assertElementText('[data-testid="result-s07"]', "Double clicked!", "double-click acknowledged");
});
