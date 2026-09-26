#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

// A download must not navigate away from the page.
await runFileUploadGolden("download-tc11", "Download: page does not navigate", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-txt"]');
  await golden.downloadBySelector('[data-testid="dl-txt"]', "downloads/qa-note.txt", "download the txt anchor");
  await golden.assertUrlEquals(fixtureUrl, "still on the fixture page");
});
