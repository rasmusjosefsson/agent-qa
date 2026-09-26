#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("tabs-windows", "tc01", "Tabs TC01 — open link in new tab", "https://qaplayground.com/practice/tabs-windows", '[data-testid="tw-open-new-tab"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="tw-open-new-tab"]', "open-tab link in view");
  await b.clickSelector('[data-testid="tw-open-new-tab"]', "click opens a new tab");
  await b.tabAction("t2", "focus the opened tab");
  await b.assertUrlContains("/practice/tabs-windows", "new tab loaded the page");
  await b.tabAction("t1", "switch back to the parent tab");
  await b.assertElementText('[data-testid="result-s01"]', "New tab opened → assert context.pages().length === 2", "parent records the new tab");
});
