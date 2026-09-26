#!/usr/bin/env bun
import { runFileUploadGolden } from "./file-upload-lib.ts";

await runFileUploadGolden("upload-tc11", "Upload TC11 file input accepts only allowed extensions", async (golden) => {
  await golden.openPage();
  // fu-type-input declares accept="image/*" — assert the attribute directly.
  await golden.assertElementAttribute("#fu-type-input", "accept", "image/*", "accept attribute restricts to images");
});
