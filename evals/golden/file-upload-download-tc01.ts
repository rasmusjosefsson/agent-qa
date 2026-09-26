#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

// The live File Upload page ships no download widget, so the download contract
// is proven against the bundled fixture: click a download anchor, the file
// lands in the scenario dir, and file claims verify it.
await runFileUploadGolden("download-tc01", "Download: file lands and matches name/size", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-txt"]');
  await golden.downloadBySelector('[data-testid="dl-txt"]', "downloads/qa-note.txt", "download the txt anchor");
  await golden.assertFileExists("downloads/qa-note.txt", "downloaded file exists");
  await golden.assertFileName("downloads/qa-note.txt", "qa-note.txt", "downloaded file name matches");
  await golden.assertFileSizeGt("downloads/qa-note.txt", 0, "downloaded file is not empty");
});
