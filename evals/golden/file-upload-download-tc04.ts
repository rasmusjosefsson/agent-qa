#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

await runFileUploadGolden("download-tc04", "Download: file content matches expected text", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-json"]');
  await golden.downloadBySelector('[data-testid="dl-json"]', "downloads/qa-data.json", "download the json anchor");
  await golden.assertFileContent("downloads/qa-data.json", '"fixture":true', "downloaded content matches");
});
