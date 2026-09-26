#!/usr/bin/env bun
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { pathToFileURL } from "url";
import { runFileUploadGolden } from "./file-upload-lib.ts";

const fixtureUrl = pathToFileURL(
  resolve(dirname(fileURLToPath(import.meta.url)), "../fixtures/downloads.html"),
).href;

// Download triggers are keyboard- and screen-reader-operable controls — the
// blob button must surface an accessible name.
await runFileUploadGolden("download-tc05", "Download: trigger has an accessible label", async (golden) => {
  await golden.openFixture(fixtureUrl, '[data-testid="dl-blob"]');
  await golden.assertRolePresent("button", "Download blob report", "blob button exposes its accessible name");
  await golden.assertRoleAbsent("button", "definitely-not-the-label", "wrong label is absent");
});
