#!/usr/bin/env bun
import { runFileUploadGolden } from "./file-upload-lib.ts";

await runFileUploadGolden("upload-tc10", "Upload TC10 multiple files can be selected when allowed", async (golden) => {
  await golden.openPage();
  // fu-multi-input carries the multiple attribute — upload two files together.
  await golden.uploadMulti("#fu-multi-input", ["upload-valid.txt", "upload-unsupported.exe"], "select two files");
  await golden.waitSelectorText('[id="result-s02"]', "2 file(s) selected", "both names listed");
  await golden.waitSelectorText('[id="result-s02"]', "upload-unsupported.exe", "second file name listed");
});
