#!/usr/bin/env bun
import { mkdtempSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { resolve } from "path";
import { runFileUploadGolden } from "./file-upload-lib.ts";

await runFileUploadGolden("upload-tc07", "Upload TC07 error for files exceeding size limit", async (golden) => {
  await golden.openPage();
  // The size panel documents Max size: 2 MB — generate a 2.5 MB oversize file
  // under the OS temp dir (not committed).
  const big = resolve(mkdtempSync(`${tmpdir()}/aq-oversize-`), "oversize.bin");
  writeFileSync(big, Buffer.alloc(2_600_000, 7));
  await golden.uploadAbs("#fu-size-input", big, big, "select oversize file");
  await golden.waitSelectorText('[id="result-s06"]', "Size error", "size limit error shown");
});
