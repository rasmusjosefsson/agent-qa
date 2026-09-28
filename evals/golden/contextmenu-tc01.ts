#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixture = `file://${resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/contextmenu.html")}`;

await runFileUploadGolden(
  "contextmenu-tc01",
  "Context menu: custom menu + native-alert handler via rightclick",
  async (golden) => {
    await golden.openFixture(fixture, "#hot-spot");

    await golden.rightClickSelector("#hot-spot", "right-click the box opens the custom menu");
    // Gate on the class the handler adds — the marker is display:none so a
    // visibility-gated wait would never resolve.
    await golden.waitLiveSelector("#menu.open", "menu open marker");
    await golden.assertElementAttribute(
      "#menu",
      "class",
      "open",
      "menu element carries .open",
      "contains",
    );
    await golden.clickSelector('#menu [data-item="copy"]', "pick Copy from the menu");
    await golden.waitText("#out", "picked: copy", "menu action ran");

    // A contextmenu handler that opens a native alert must not deadlock the
    // verb — the dispatch is deferred past the eval window.
    await golden.rightClickSelector("#alert-btn", "right-click opens a native alert");
    await golden.assertDialogOpen("alert pending");
    await golden.dialogAccept("accept it");
    await golden.assertDialogClosed("alert resolved");
  },
);
