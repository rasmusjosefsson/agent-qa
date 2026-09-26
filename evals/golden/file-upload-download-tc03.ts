#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

await runFileUploadGolden("download-tc03", "Download: downloaded file is not empty", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-json"]');
  await golden.downloadBySelector('[data-testid="dl-json"]', "downloads/qa-data.json", "download the json anchor");
  await golden.assertFileSizeGt("downloads/qa-data.json", 5, "downloaded file has content");
});
