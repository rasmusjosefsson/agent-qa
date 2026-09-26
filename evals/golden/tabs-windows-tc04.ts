#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("tabs-windows", "tc04", "Tabs TC04 — close child tab", "https://qaplayground.com/practice/tabs-windows", '[data-testid="tw-open-new-tab"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="tw-close-tab-btn"]', "close-tab widget in view");
  await b.clickSelector('[data-testid="tw-close-tab-btn"]', "open child tab");
  await b.tabAction("close t2", "close the child tab");
  await b.tabAction("t1", "return to the parent tab");
  await b.assertElementText('[data-testid="result-s05"]', "New tab opened → call newPage.close()", "parent records the cycle");
});
