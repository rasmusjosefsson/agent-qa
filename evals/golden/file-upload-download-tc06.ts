#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

// Keyboard activation: focus the download button and press Enter — no mouse.
// The blob handler flipping #dl-status proves the download was triggered.
await runFileUploadGolden("download-tc06", "Download: keyboard activation triggers the download", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-blob"]');
  await golden.pressKeyOn('[data-testid="dl-blob"]', "Enter", "focus the blob button and press Enter");
  await golden.waitText('[data-testid="dl-status"]', "blob download triggered", "download handler ran on Enter");
});
