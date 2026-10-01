#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// material.angular.dev/components/select — mat-select renders its
// options in a CDK overlay panel detached at document.body (portal),
// not inside the select element: presence/absence claims on the panel
// plus a value-text claim prove the pick round-trips.
await runEdgeGolden(
  "tc01", "angular material — mat-select portal pick",
  "https://material.angular.dev/components/select/overview",
  "select-overview-example mat-select",
  async (b) => {
    await b.openPage();
    await b.clickSelector("select-overview-example mat-select", "open the Favorite food select");
    await b.waitSelector(".mat-mdc-select-panel", "option panel overlay opened");
    await b.assertElementCount(".mat-mdc-select-panel mat-option", 3, "three food options in the portal");
    await b.clickSelector(".mat-mdc-select-panel mat-option:nth-child(2)", "pick Pizza");
    await b.waitMs(300, "panel close");
    await b.assertElementAbsent(".mat-mdc-select-panel", "portal panel closed");
    await b.assertElementText("select-overview-example .mat-mdc-select-value-text", "Pizza", "select shows the pick");
    await b.assertElementPresent("select-overview-example mat-select.mat-mdc-select-value-text, select-overview-example .mat-mdc-select", "select element intact");
  },
  { label: "mat" },
);
