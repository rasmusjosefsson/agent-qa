#!/usr/bin/env bun
import { runFileUploadGolden } from "./file-upload-lib.ts";

await runFileUploadGolden("upload-tc15", "Upload TC15 file upload page loads without errors", async (golden) => {
  await golden.openPage();
  // URL stayed on /practice/file-upload (recorded navigation) and the single
  // input rendered (recorded wait) — plus the multi + progress inputs prove
  // the whole widget set hydrated.
  await golden.waitSelectorVisible("#fu-multi-input", "multi-file input visible");
  await golden.waitSelectorVisible('[data-testid="fu-upload-btn"]', "upload submit button rendered");
});
