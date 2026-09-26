#!/usr/bin/env bun
import { runFileUploadGolden } from "./file-upload-lib.ts";

await runFileUploadGolden("upload-tc06", "Upload TC06 error message for unsupported file type", async (golden) => {
  await golden.openPage();
  // fu-type-input is constrained to accept="image/*"; an .exe must be rejected.
  await golden.upload("#fu-type-input", "upload-unsupported.exe", "select unsupported .exe");
  await golden.waitSelectorText('[id="result-s05"]', "Type error", "rejection error shown");
});
