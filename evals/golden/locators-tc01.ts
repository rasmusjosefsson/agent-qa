#!/usr/bin/env bun
import { writeFileSync } from "fs";
import { resolve } from "path";
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden(
  "locators",
  "tc01",
  "Locators — scoped role locator + i18nKey name",
  "https://qaplayground.com/practice/multi-select",
  '[data-testid="scenario-ms-single"]',
  async (b) => {
    await b.openPage();
    await b.scrollToSelector('[data-testid="scenario-ms-select-all"]', "select-all widget in view");
    await b.clickSelector('[data-testid="scenario-ms-select-all"] [data-testid="ms-custom-trigger"]', "open the panel");
    // Presence probe resolves role+name strictly inside the card scope;
    // the name itself resolves through i18n.json beside the scenario.
    await b.waitScopedRoleVisible(
      '[data-testid="scenario-ms-select-all"]',
      "button",
      { i18nKey: "ms.selectAll" },
      "bulk actions visible",
    );
    await b.clickScopedRole(
      '[data-testid="scenario-ms-select-all"]',
      '[data-testid="ms-select-all-btn"]',
      "button",
      { i18nKey: "ms.selectAll" },
      "Select All",
    );
    await b.assertElementText('[data-testid="result-s05"]', "All selected", "all options echoed");
  },
  {
    beforeReplay: (scenarioDir) => {
      writeFileSync(resolve(scenarioDir, "i18n.json"), JSON.stringify({ "ms.selectAll": "Select All" }, null, 2));
    },
  },
);
