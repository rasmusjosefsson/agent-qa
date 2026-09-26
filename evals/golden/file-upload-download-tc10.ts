#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

await runFileUploadGolden("download-tc10", "Download: several files in sequence", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-txt"]');
  await golden.downloadBySelector('[data-testid="dl-txt"]', "downloads/qa-note.txt", "download the txt anchor");
  await golden.downloadBySelector('[data-testid="dl-json"]', "downloads/qa-data.json", "download the json anchor");
  await golden.assertFileExists("downloads/qa-note.txt", "first file exists");
  await golden.assertFileExists("downloads/qa-data.json", "second file exists");
  await golden.assertFileName("downloads/qa-data.json", "qa-data.json", "second file name matches");
});
