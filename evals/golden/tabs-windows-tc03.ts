#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("tabs-windows", "tc03", "Tabs TC03 — switch back to parent", "https://qaplayground.com/practice/tabs-windows", '[data-testid="tw-open-new-tab"]', async (b) => {
  await b.openPage();
  await b.scrollToSelector('[data-testid="tw-open-and-return"]', "switch-back widget in view");
  await b.clickSelector('[data-testid="tw-open-and-return"]', "open child tab");
  await b.tabAction("t2", "focus the child tab");
  await b.assertUrlContains("/practice/tabs-windows", "child tab loaded");
  await b.tabAction("t1", "return to the parent tab");
  await b.scrollToSelector('[data-testid="scenario-tw-switch-back"]', "widget back in view");
  await b.clickSelector('[data-testid="scenario-tw-switch-back"] button:first-of-type', "mark as returned");
  await b.assertElementText('[data-testid="result-s03"]', "Switched back to original tab ✓", "return acknowledged");
});
