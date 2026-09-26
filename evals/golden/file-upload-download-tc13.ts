#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

await runFileUploadGolden("download-tc13", "Download: page loads without errors", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-txt"]');
  await golden.assertElementVisible("h1", "heading rendered");
  await golden.assertElementAttribute('[data-testid="dl-status"]', "text", "idle", "status starts idle");
});
