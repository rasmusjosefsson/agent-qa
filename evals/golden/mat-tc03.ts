#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// material.angular.dev/components/datepicker — the calendar portal is
// the real input path (mat-datepicker ignores programmatic text entry).
// Two `.mat-calendar` nodes exist on the page (a hidden prerendered
// copy ~10k px offscreen), so every selector scopes to the live
// `.cdk-overlay-pane`. The picked value varies by run date — the claim
// is a locale-date pattern via the matches predicate.
await runEdgeGolden(
  "tc03", "angular material — datepicker calendar pick",
  "https://material.angular.dev/components/datepicker/overview",
  "datepicker-overview-example input",
  async (b) => {
    await b.openPage();
    await b.clickSelector("datepicker-overview-example mat-datepicker-toggle", "open the calendar");
    await b.waitSelector(".cdk-overlay-pane", "calendar portal rendered");
    await b.assertElementPresent(".cdk-overlay-pane .mat-calendar-body-active", "calendar marks an active cell");
    // Today is the only always-enabled deterministic cell.
    await b.clickSelector(".cdk-overlay-pane button.mat-calendar-body-cell:has(.mat-calendar-body-today)", "pick today");
    await b.assertElementAbsent(".cdk-overlay-pane", "calendar closed on pick");
    await b.assertElementAttribute("datepicker-overview-example input", "value", "matches", "\\d{1,2}/\\d{1,2}/\\d{4}", "input holds the picked date");
  },
  { label: "mat" },
);
