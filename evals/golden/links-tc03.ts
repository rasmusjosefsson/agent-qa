#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("links", "tc03", "Links TC03 — external link opens new tab", "https://qaplayground.com/practice/links", '[data-testid="link-internal-home"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="link-external-selenium"]', "target", "_blank", "external link marks new tab");
  await b.scrollToSelector('[data-testid="link-external-selenium"]', "external link in view");
  await b.clickSelector('[data-testid="link-external-selenium"]', "click external link");
  await b.tabAction("t2", "focus the opened tab");
  await b.assertElementAbsent('[data-testid="link-internal-home"]', "child tab is not the links page");
  await b.tabAction("t1", "switch back to the practice tab");
  await b.assertElementAttribute('[data-testid="link-external-course"]', "target", "_blank", "second external link also marked");
});
