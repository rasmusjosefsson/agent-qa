#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// material.angular.dev/components/autocomplete — typing filters an
// autocomplete overlay panel; the pick writes back into the input's
// IDL value (element attribute claim).
await runEdgeGolden(
  "tc02", "angular material — mat-autocomplete filter + pick",
  "https://material.angular.dev/components/autocomplete/overview",
  "autocomplete-filter-example input",
  async (b) => {
    await b.openPage();
    await b.fillSelector("autocomplete-filter-example input", "tw", "type tw into the autocomplete");
    await b.waitSelector(".mat-mdc-autocomplete-panel mat-option", "filtered option rendered");
    await b.assertElementCount(".mat-mdc-autocomplete-panel mat-option", 1, "only Two matches tw");
    await b.clickSelector(".mat-mdc-autocomplete-panel mat-option", "pick Two");
    await b.waitMs(300, "panel close");
    await b.assertElementAbsent(".mat-mdc-autocomplete-panel", "autocomplete panel closed");
    await b.assertElementAttribute("autocomplete-filter-example input", "value", "equals", "Two", "input value is Two");
  },
  { label: "mat" },
);
