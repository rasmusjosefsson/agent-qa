#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

// Reach the download section by keyboard alone: Tab moves focus through the
// controls; Enter activates — asserting the blob handler ran.
await runFileUploadGolden("download-tc12", "Download: section is keyboard reachable", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-blob"]');
  await golden.pressKey("Tab", "Tab into the first focusable control");
  await golden.pressKey("Tab", "Tab to the json anchor");
  await golden.pressKey("Tab", "Tab to the blob button");
  await golden.pressKey("Enter", "Enter activates the focused button");
  await golden.waitText('[data-testid="dl-status"]', "blob download triggered", "keyboard-only activation triggered the download");
});
