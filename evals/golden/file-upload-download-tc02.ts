#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

await runFileUploadGolden("download-tc02", "Download: file name matches downloaded file", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-json"]');
  await golden.downloadBySelector('[data-testid="dl-json"]', "downloads/qa-data.json", "download the json anchor");
  await golden.assertFileName("downloads/qa-data.json", "qa-data.json", "downloaded file name matches");
});
