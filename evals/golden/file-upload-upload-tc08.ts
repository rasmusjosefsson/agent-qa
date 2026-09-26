#!/usr/bin/env bun
import { runFileUploadGolden } from "./file-upload-lib.ts";

await runFileUploadGolden("upload-tc08", "Upload TC08 uploaded file appears in the file list", async (golden) => {
  await golden.openPage();
  await golden.upload("#fu-filename-input", "upload-valid.txt", "select upload-valid.txt");
  // The filename scenario shows the selected name in a dedicated display.
  await golden.waitSelectorText('[data-testid="fu-filename-display"]', "upload-valid.txt", "filename display lists the file");
});
