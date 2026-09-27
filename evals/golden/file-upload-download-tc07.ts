#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

await runFileUploadGolden("download-tc07", "Download: anchor carries the download href", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-json"]');
  await golden.assertElementAttribute('[data-testid="dl-json"]', "href", "application/json", "anchor href is a json payload", "contains");
  await golden.assertElementAttribute('[data-testid="dl-json"]', "download", "qa-data.json", "download attribute sets the filename");
});
