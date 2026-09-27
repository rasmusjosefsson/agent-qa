#!/usr/bin/env bun
// Download TC09 — mobile viewport.
// do/viewport resizes the browser to a phone-sized viewport, then the same
// download trigger must still be visible and capture a file.
import { dirname, resolve } from "path";
import { fileURLToPath, pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

await runFileUploadGolden("download-tc09", "download at mobile viewport", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-txt"]');
  await golden.setViewport(375, 812, "resize to mobile viewport");
  await golden.waitSelectorVisible('[data-testid="dl-txt"]', "download trigger still visible at mobile width");
  await golden.downloadBySelector('[data-testid="dl-txt"]', "downloads/qa-note.txt", "download at mobile viewport");
  await golden.assertFileExists("downloads/qa-note.txt", "captured file exists");
  await golden.assertFileName("downloads/qa-note.txt", "qa-note.txt", "captured filename");
});
