#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// material.angular.dev/components/dialog + /menu — both render into
// .cdk-overlay portal containers at document.body. Presence/absence
// claims on the portal prove open/pick/close round-trips.
await runEdgeGolden(
  "tc04", "angular material — dialog open/close + menu portal pick",
  "https://material.angular.dev/components/dialog/overview",
  "dialog-overview-example button",
  async (b) => {
    await b.openPage();
    await b.clickSelector("dialog-overview-example button", "open the dialog");
    await b.waitSelector("mat-dialog-container", "dialog portal rendered");
    await b.assertElementText("mat-dialog-container .mat-mdc-dialog-title", "Hi", "dialog title");
    await b.clickSelector("mat-dialog-container .mat-mdc-dialog-actions button:last-child", "click Ok");
    await b.assertElementAbsent("mat-dialog-container", "dialog portal closed");

    await b.gotoUrl("https://material.angular.dev/components/menu/overview", "open the menu demo");
    await b.clickSelector("menu-overview-example button", "open the menu");
    await b.waitSelector(".mat-mdc-menu-panel", "menu portal rendered");
    await b.assertElementCount(".mat-mdc-menu-panel .mat-mdc-menu-item", 2, "two menu items");
    await b.clickSelector(".mat-mdc-menu-panel .mat-mdc-menu-item:nth-child(2)", "pick Item 2");
    await b.waitMs(300, "menu close");
    await b.assertElementAbsent(".mat-mdc-menu-panel", "menu portal closed");
  },
  { label: "mat" },
);
